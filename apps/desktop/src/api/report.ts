export const DESKTOP_REPORT_SCHEMA_VERSION = 2;

const UNSIGNED_DECIMAL = /^(0|[1-9][0-9]*)$/;
const MAX_VOLUMES = 1_024;
const MAX_U32 = 4_294_967_295;
const MAX_U64 = 18_446_744_073_709_551_615n;
const MAX_WARNINGS = 512;
const MAX_WARNING_CODE_POINTS = 512;

export type PartitionTable = "mbr" | "gpt" | "none";
export type FileSystem =
  | "ntfs"
  | "fat12"
  | "fat16"
  | "fat32"
  | "unrecognized";
export type VolumeScanStatus = "complete" | "partial" | "unrecognized";

export interface DesktopSource {
  label: string;
  sizeBytes: string;
}

export interface DesktopVolume {
  index: number;
  offsetBytes: string;
  lengthBytes: string;
  fileSystem: FileSystem;
  scanStatus: VolumeScanStatus;
  candidateCount: string;
  warnings: string[];
}

export interface DesktopScanReport {
  schemaVersion: number;
  source: DesktopSource;
  partitionTable: PartitionTable;
  volumes: DesktopVolume[];
  warnings: string[];
  warningCount: string;
  warningsOmitted: string;
}

export class ReportContractError extends Error {
  constructor() {
    super("The desktop scanner returned an incompatible report.");
    this.name = "ReportContractError";
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasControlCharacter(value: string): boolean {
  return [...value].some((character) => {
    const codePoint = character.codePointAt(0);
    return (
      codePoint !== undefined &&
      (codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f))
    );
  });
}

function decimal(value: unknown): string {
  if (
    typeof value !== "string" ||
    value.length > 20 ||
    !UNSIGNED_DECIMAL.test(value) ||
    BigInt(value) > MAX_U64
  ) {
    throw new ReportContractError();
  }
  return value;
}

function finiteIndex(value: unknown): number {
  if (
    typeof value !== "number" ||
    !Number.isSafeInteger(value) ||
    value < 0 ||
    value > MAX_U32
  ) {
    throw new ReportContractError();
  }
  return value;
}

function warning(value: unknown): string {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    [...value].length > MAX_WARNING_CODE_POINTS ||
    hasControlCharacter(value)
  ) {
    throw new ReportContractError();
  }
  return value;
}

function warningList(value: unknown): string[] {
  if (!Array.isArray(value)) {
    throw new ReportContractError();
  }
  return value.map(warning);
}

function source(value: unknown): DesktopSource {
  if (!isRecord(value)) {
    throw new ReportContractError();
  }
  const { label } = value;
  if (
    typeof label !== "string" ||
    label.length === 0 ||
    [...label].length > 512 ||
    label.includes("/") ||
    label.includes("\\") ||
    hasControlCharacter(label)
  ) {
    throw new ReportContractError();
  }
  return { label, sizeBytes: decimal(value.sizeBytes) };
}

function partitionTable(value: unknown): PartitionTable {
  if (value === "mbr" || value === "gpt" || value === "none") {
    return value;
  }
  throw new ReportContractError();
}

function fileSystem(value: unknown): FileSystem {
  if (
    value === "ntfs" ||
    value === "fat12" ||
    value === "fat16" ||
    value === "fat32" ||
    value === "unrecognized"
  ) {
    return value;
  }
  throw new ReportContractError();
}

function scanStatus(
  value: unknown,
  parsedFileSystem: FileSystem,
): VolumeScanStatus {
  const valid =
    (value === "complete" && parsedFileSystem !== "unrecognized") ||
    (value === "partial" && parsedFileSystem !== "unrecognized") ||
    (value === "unrecognized" && parsedFileSystem === "unrecognized");
  if (valid) {
    return value;
  }
  throw new ReportContractError();
}

function volume(value: unknown): DesktopVolume {
  if (!isRecord(value)) {
    throw new ReportContractError();
  }
  const parsedFileSystem = fileSystem(value.fileSystem);
  return {
    index: finiteIndex(value.index),
    offsetBytes: decimal(value.offsetBytes),
    lengthBytes: decimal(value.lengthBytes),
    fileSystem: parsedFileSystem,
    scanStatus: scanStatus(value.scanStatus, parsedFileSystem),
    candidateCount: decimal(value.candidateCount),
    warnings: warningList(value.warnings),
  };
}

export function parseDesktopScanReport(value: unknown): DesktopScanReport {
  if (!isRecord(value) || value.schemaVersion !== DESKTOP_REPORT_SCHEMA_VERSION) {
    throw new ReportContractError();
  }

  if (!Array.isArray(value.volumes) || value.volumes.length > MAX_VOLUMES) {
    throw new ReportContractError();
  }

  const volumes = value.volumes.map(volume);
  const indexes = new Set(volumes.map((item) => item.index));
  if (indexes.size !== volumes.length) {
    throw new ReportContractError();
  }

  const warnings = warningList(value.warnings);
  const returnedWarningCount =
    warnings.length +
    volumes.reduce((total, item) => total + item.warnings.length, 0);
  if (returnedWarningCount > MAX_WARNINGS) {
    throw new ReportContractError();
  }

  const warningCount = decimal(value.warningCount);
  const warningsOmitted = decimal(value.warningsOmitted);
  if (
    BigInt(warningCount) !==
    BigInt(returnedWarningCount) + BigInt(warningsOmitted)
  ) {
    throw new ReportContractError();
  }

  return {
    schemaVersion: value.schemaVersion,
    source: source(value.source),
    partitionTable: partitionTable(value.partitionTable),
    volumes,
    warnings,
    warningCount,
    warningsOmitted,
  };
}

export function sumCandidateCounts(report: DesktopScanReport): bigint {
  return report.volumes.reduce(
    (total, item) => total + BigInt(item.candidateCount),
    0n,
  );
}
