export const STORAGE_CONTRACT_SCHEMA_VERSION = 1;
export const SCAN_SUMMARY_SCHEMA_VERSION = 3;
export const CANDIDATE_PAGE_SCHEMA_VERSION = 2;
export const CANDIDATE_QUERY_SCHEMA_VERSION = 1;

const UNSIGNED_DECIMAL = /^(0|[1-9][0-9]*)$/;
const OPAQUE_ID = /^[A-Za-z0-9_-]{1,128}$/;
const MOUNT_LABEL = /^[A-Za-z]:$/;
const SHA256_HEX = /^[0-9a-f]{64}$/;
const MAX_U64 = 18_446_744_073_709_551_615n;
const MAX_DISKS = 128;
const MAX_VOLUMES_PER_DISK = 128;
const MAX_CANDIDATES_PER_PAGE = 200;
const MAX_ACTIONABLE_CANDIDATES_PER_PAGE = 100;
export const MAX_RETAINED_CANDIDATES_PER_SCAN = 110_000;
const MAX_WARNINGS = 128;
const MAX_TEXT_CODE_POINTS = 512;

export type BusType =
  | "unknown"
  | "ata"
  | "sata"
  | "scsi"
  | "usb"
  | "nvme"
  | "virtual";
export type SupportedFileSystem =
  | "ntfs"
  | "fat12"
  | "fat16"
  | "fat32"
  | "unrecognized";
export type ScanStatus = "complete" | "partial" | "unrecognized";
export type ScanMode = "metadata" | "deepJpeg";
export type CandidateKind = "file" | "directory";
export type DiscoveryMethod =
  | "ntfsMetadata"
  | "fatMetadata"
  | "exfatMetadata"
  | "carving"
  | "recycleBin";
export type CandidateState =
  | "exactEvidence"
  | "likelyComplete"
  | "completeUnvalidated"
  | "structurallyValid"
  | "partial"
  | "conflicted"
  | "readError"
  | "zeroedOrTrimmed"
  | "overwritten"
  | "metadataOnly"
  | "unknown";
export type MetadataConfidence = "high" | "medium" | "low";
export type PathState =
  | "exact"
  | "reconstructed"
  | "incomplete"
  | "orphaned"
  | "ambiguous";
export type RecoveryEligibility = "complete" | "bestEffort" | "ineligible";
export type CandidateSortField =
  | "path"
  | "extension"
  | "size"
  | "state"
  | "confidence"
  | "recoverabilityScore"
  | "method";
export type CandidateSortDirection = "ascending" | "descending";

export interface CandidateQuery {
  revision: string;
  search: string;
  extensions: readonly string[];
  kinds: readonly CandidateKind[];
  metadataConfidences: readonly MetadataConfidence[];
  methods: readonly DiscoveryMethod[];
  states: readonly CandidateState[];
  minRecoverabilityScore: number | null;
  maxRecoverabilityScore: number | null;
  eligibilities: readonly RecoveryEligibility[];
  selectedOnly: boolean;
}

export interface CandidateSort {
  field: CandidateSortField;
  direction: CandidateSortDirection;
}

export interface ActionableCandidateRow {
  id: string;
  displayPath: string;
  extension: string;
  kind: CandidateKind;
  state: CandidateState;
  sizeBytes: string;
  metadataConfidence: MetadataConfidence;
  recoverabilityScore: number | null;
  pathState: PathState;
  method: DiscoveryMethod;
  eligibility: RecoveryEligibility;
  selected: boolean;
  warnings: string[];
}

export interface CandidateSelectionSummary {
  selectionRevision: string;
  selectedCandidates: string;
  selectedFiles: string;
  selectedDirectories: string;
  selectedLogicalBytes: string;
  bestEffortCandidates: string;
  conflictedCandidates: string;
  ineligibleCandidates: string;
  matchingSelectedCandidates: string;
}

export interface CandidateExtensionFacet {
  extension: string;
  count: string;
}

export interface CandidateQueryPage {
  schemaVersion: 1;
  scanId: string;
  queryId: string;
  queryRevision: string;
  cursor: string | null;
  nextCursor: string | null;
  filteredTotal: string;
  extensionFacets: CandidateExtensionFacet[];
  selection: CandidateSelectionSummary;
  candidates: ActionableCandidateRow[];
}

export type CandidateSelectionOperation =
  | {
      type: "setIds";
      candidateIds: readonly string[];
      selected: boolean;
    }
  | { type: "selectAllMatching" }
  | { type: "clearMatching" }
  | { type: "clearAll" };

export interface CandidateSelectionUpdate {
  schemaVersion: 1;
  scanId: string;
  queryId: string;
  selectionRevision: string;
  selection: CandidateSelectionSummary;
}

export interface StorageVolume {
  id: string;
  mountLabel: string;
  label: string;
  fileSystem: string;
  sizeBytes: string;
  freeBytes: string;
  isSystem: boolean;
  scanSupported: boolean;
  folderScopeSupported: boolean;
  warnings: string[];
}

export interface StorageDisk {
  id: string;
  displayName: string;
  busType: BusType;
  sizeBytes: string;
  volumes: StorageVolume[];
}

export interface StorageInventory {
  schemaVersion: 1;
  generation: string;
  disks: StorageDisk[];
}

export interface ScanScope {
  kind: "volume" | "folder";
  label: string;
}

export interface MftScanCoverage {
  recordsDeclared: string;
  recordsAvailable: string;
  recordsExamined: string;
  bytesDeclared: string;
  bytesAvailable: string;
  bytesExamined: string;
}

export interface JpegCarveCoverage {
  bytesRequested: string;
  bytesScanned: string;
  signaturesAttempted: string;
  validationBytesRead: string;
  partial: boolean;
  readErrorCount: string;
  candidateLimitReached: boolean;
  candidateByteLimitHits: string;
  signatureAttemptLimitReached: boolean;
  validationByteLimitReached: boolean;
  rejectedSignatures: string;
  truncatedSignatures: string;
  regionsSubmitted: string;
  regionLimitReached: boolean;
}

export interface FolderSelection {
  schemaVersion: 1;
  scopeId: string;
  volumeId: string;
  label: string;
}

export interface ScanSummary {
  schemaVersion: 3;
  scanId: string;
  sourceLabel: string;
  scope: ScanScope;
  scanMode: ScanMode;
  fileSystem: SupportedFileSystem;
  scanStatus: ScanStatus;
  totalCandidates: string;
  matchedCandidates: string;
  unknownCandidates: string;
  mftCoverage: MftScanCoverage | null;
  jpegCarveCoverage: JpegCarveCoverage | null;
  warnings: string[];
}

export interface CandidateRow {
  id: string;
  displayPath: string;
  kind: CandidateKind;
  state: CandidateState;
  sizeBytes: string;
  metadataConfidence: MetadataConfidence;
  recoverabilityScore: number | null;
  pathState: PathState;
  method: DiscoveryMethod;
  contentSha256: string | null;
  validator: string | null;
  warnings: string[];
}

export interface CandidatePage {
  schemaVersion: 2;
  scanId: string;
  cursor: string | null;
  nextCursor: string | null;
  candidates: CandidateRow[];
}

export class StorageContractError extends Error {
  constructor() {
    super("The desktop storage service returned an incompatible response.");
    this.name = "StorageContractError";
  }
}

function record(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new StorageContractError();
  }
  return value as Record<string, unknown>;
}

function exactKeys(value: Record<string, unknown>, keys: readonly string[]) {
  if (
    Object.keys(value).length !== keys.length ||
    !keys.every((key) => Object.hasOwn(value, key))
  ) {
    throw new StorageContractError();
  }
}

function text(value: unknown, allowEmpty = false): string {
  if (
    typeof value !== "string" ||
    (!allowEmpty && value.length === 0) ||
    [...value].length > MAX_TEXT_CODE_POINTS ||
    [...value].some((character) => {
      const code = character.codePointAt(0);
      return (
        code !== undefined &&
        (code <= 0x1f ||
          (code >= 0x7f && code <= 0x9f) ||
          (code >= 0x202a && code <= 0x202e) ||
          (code >= 0x2066 && code <= 0x2069))
      );
    })
  ) {
    throw new StorageContractError();
  }
  return value;
}

function opaqueId(value: unknown): string {
  if (typeof value !== "string" || !OPAQUE_ID.test(value)) {
    throw new StorageContractError();
  }
  return value;
}

function decimal(value: unknown): string {
  if (
    typeof value !== "string" ||
    value.length > 20 ||
    !UNSIGNED_DECIMAL.test(value) ||
    BigInt(value) > MAX_U64
  ) {
    throw new StorageContractError();
  }
  return value;
}

function boolean(value: unknown): boolean {
  if (typeof value !== "boolean") {
    throw new StorageContractError();
  }
  return value;
}

function warningList(value: unknown): string[] {
  if (!Array.isArray(value) || value.length > MAX_WARNINGS) {
    throw new StorageContractError();
  }
  return value.map((item) => text(item));
}

function oneOf<T extends string>(
  value: unknown,
  allowed: readonly T[],
): T {
  if (typeof value !== "string" || !allowed.includes(value as T)) {
    throw new StorageContractError();
  }
  return value as T;
}

function volume(value: unknown): StorageVolume {
  const item = record(value);
  exactKeys(item, [
    "id",
    "mountLabel",
    "label",
    "fileSystem",
    "sizeBytes",
    "freeBytes",
    "isSystem",
    "scanSupported",
    "folderScopeSupported",
    "warnings",
  ]);
  if (typeof item.mountLabel !== "string" || !MOUNT_LABEL.test(item.mountLabel)) {
    throw new StorageContractError();
  }
  return {
    id: opaqueId(item.id),
    mountLabel: item.mountLabel,
    label: text(item.label, true),
    fileSystem: text(item.fileSystem),
    sizeBytes: decimal(item.sizeBytes),
    freeBytes: decimal(item.freeBytes),
    isSystem: boolean(item.isSystem),
    scanSupported: boolean(item.scanSupported),
    folderScopeSupported: boolean(item.folderScopeSupported),
    warnings: warningList(item.warnings),
  };
}

function disk(value: unknown): StorageDisk {
  const item = record(value);
  exactKeys(item, [
    "id",
    "displayName",
    "busType",
    "sizeBytes",
    "volumes",
  ]);
  if (
    !Array.isArray(item.volumes) ||
    item.volumes.length > MAX_VOLUMES_PER_DISK
  ) {
    throw new StorageContractError();
  }
  const volumes = item.volumes.map(volume);
  if (new Set(volumes.map((entry) => entry.id)).size !== volumes.length) {
    throw new StorageContractError();
  }
  return {
    id: opaqueId(item.id),
    displayName: text(item.displayName),
    busType: oneOf(item.busType, [
      "unknown",
      "ata",
      "sata",
      "scsi",
      "usb",
      "nvme",
      "virtual",
    ]),
    sizeBytes: decimal(item.sizeBytes),
    volumes,
  };
}

export function parseStorageInventory(value: unknown): StorageInventory {
  const item = record(value);
  exactKeys(item, ["schemaVersion", "generation", "disks"]);
  if (
    item.schemaVersion !== STORAGE_CONTRACT_SCHEMA_VERSION ||
    !Array.isArray(item.disks) ||
    item.disks.length > MAX_DISKS
  ) {
    throw new StorageContractError();
  }
  const disks = item.disks.map(disk);
  if (new Set(disks.map((entry) => entry.id)).size !== disks.length) {
    throw new StorageContractError();
  }
  return {
    schemaVersion: 1,
    generation: opaqueId(item.generation),
    disks,
  };
}

export function parseFolderSelection(value: unknown): FolderSelection {
  const item = record(value);
  exactKeys(item, ["schemaVersion", "scopeId", "volumeId", "label"]);
  if (item.schemaVersion !== STORAGE_CONTRACT_SCHEMA_VERSION) {
    throw new StorageContractError();
  }
  return {
    schemaVersion: 1,
    scopeId: opaqueId(item.scopeId),
    volumeId: opaqueId(item.volumeId),
    label: text(item.label),
  };
}

function scope(value: unknown): ScanScope {
  const item = record(value);
  exactKeys(item, ["kind", "label"]);
  return {
    kind: oneOf(item.kind, ["volume", "folder"]),
    label: text(item.label),
  };
}

function mftCoverage(value: unknown): MftScanCoverage | null {
  if (value === null) {
    return null;
  }
  const item = record(value);
  exactKeys(item, [
    "recordsDeclared",
    "recordsAvailable",
    "recordsExamined",
    "bytesDeclared",
    "bytesAvailable",
    "bytesExamined",
  ]);
  return {
    recordsDeclared: decimal(item.recordsDeclared),
    recordsAvailable: decimal(item.recordsAvailable),
    recordsExamined: decimal(item.recordsExamined),
    bytesDeclared: decimal(item.bytesDeclared),
    bytesAvailable: decimal(item.bytesAvailable),
    bytesExamined: decimal(item.bytesExamined),
  };
}

function jpegCarveCoverage(value: unknown): JpegCarveCoverage | null {
  if (value === null) {
    return null;
  }
  const item = record(value);
  exactKeys(item, [
    "bytesRequested",
    "bytesScanned",
    "signaturesAttempted",
    "validationBytesRead",
    "partial",
    "readErrorCount",
    "candidateLimitReached",
    "candidateByteLimitHits",
    "signatureAttemptLimitReached",
    "validationByteLimitReached",
    "rejectedSignatures",
    "truncatedSignatures",
    "regionsSubmitted",
    "regionLimitReached",
  ]);
  const coverage = {
    bytesRequested: decimal(item.bytesRequested),
    bytesScanned: decimal(item.bytesScanned),
    signaturesAttempted: decimal(item.signaturesAttempted),
    validationBytesRead: decimal(item.validationBytesRead),
    partial: boolean(item.partial),
    readErrorCount: decimal(item.readErrorCount),
    candidateLimitReached: boolean(item.candidateLimitReached),
    candidateByteLimitHits: decimal(item.candidateByteLimitHits),
    signatureAttemptLimitReached: boolean(
      item.signatureAttemptLimitReached,
    ),
    validationByteLimitReached: boolean(
      item.validationByteLimitReached,
    ),
    rejectedSignatures: decimal(item.rejectedSignatures),
    truncatedSignatures: decimal(item.truncatedSignatures),
    regionsSubmitted: decimal(item.regionsSubmitted),
    regionLimitReached: boolean(item.regionLimitReached),
  };
  if (BigInt(coverage.bytesScanned) > BigInt(coverage.bytesRequested)) {
    throw new StorageContractError();
  }
  if (
    !coverage.partial &&
    (coverage.signatureAttemptLimitReached ||
      coverage.validationByteLimitReached)
  ) {
    throw new StorageContractError();
  }
  return coverage;
}

export function parseScanSummary(value: unknown): ScanSummary {
  const item = record(value);
  exactKeys(item, [
    "schemaVersion",
    "scanId",
    "sourceLabel",
    "scope",
    "scanMode",
    "fileSystem",
    "scanStatus",
    "totalCandidates",
    "matchedCandidates",
    "unknownCandidates",
    "mftCoverage",
    "jpegCarveCoverage",
    "warnings",
  ]);
  if (item.schemaVersion !== SCAN_SUMMARY_SCHEMA_VERSION) {
    throw new StorageContractError();
  }
  const fileSystem = oneOf(item.fileSystem, [
    "ntfs",
    "fat12",
    "fat16",
    "fat32",
    "unrecognized",
  ]);
  const scanStatus = oneOf(item.scanStatus, [
    "complete",
    "partial",
    "unrecognized",
  ]);
  if (
    (fileSystem === "unrecognized") !== (scanStatus === "unrecognized")
  ) {
    throw new StorageContractError();
  }
  const parsedScope = scope(item.scope);
  const scanMode = oneOf(item.scanMode, ["metadata", "deepJpeg"]);
  const parsedJpegCoverage = jpegCarveCoverage(item.jpegCarveCoverage);
  const validMetadataScan =
    scanMode === "metadata" && parsedJpegCoverage === null;
  const validDeepScan =
    scanMode === "deepJpeg" &&
    parsedScope.kind === "volume" &&
    fileSystem === "ntfs" &&
    parsedJpegCoverage !== null &&
    (!parsedJpegCoverage.partial || scanStatus === "partial");
  if (!validMetadataScan && !validDeepScan) {
    throw new StorageContractError();
  }
  return {
    schemaVersion: 3,
    scanId: opaqueId(item.scanId),
    sourceLabel: text(item.sourceLabel),
    scope: parsedScope,
    scanMode,
    fileSystem,
    scanStatus,
    totalCandidates: decimal(item.totalCandidates),
    matchedCandidates: decimal(item.matchedCandidates),
    unknownCandidates: decimal(item.unknownCandidates),
    mftCoverage: mftCoverage(item.mftCoverage),
    jpegCarveCoverage: parsedJpegCoverage,
    warnings: warningList(item.warnings),
  };
}

function nullableOpaqueId(value: unknown): string | null {
  return value === null ? null : opaqueId(value);
}

function candidate(value: unknown): CandidateRow {
  const item = record(value);
  exactKeys(item, [
    "id",
    "displayPath",
    "kind",
    "state",
    "sizeBytes",
    "metadataConfidence",
    "recoverabilityScore",
    "pathState",
    "method",
    "contentSha256",
    "validator",
    "warnings",
  ]);
  const kind = oneOf(item.kind, ["file", "directory"]);
  const score = item.recoverabilityScore;
  const validFileScore =
    kind === "file" &&
    typeof score === "number" &&
    Number.isInteger(score) &&
    score >= 0 &&
    score <= 100;
  const validDirectoryScore = kind === "directory" && score === null;
  if (!validFileScore && !validDirectoryScore) {
    throw new StorageContractError();
  }
  const method = oneOf(item.method, [
    "ntfsMetadata",
    "fatMetadata",
    "exfatMetadata",
    "carving",
    "recycleBin",
  ]);
  const contentSha256 =
    item.contentSha256 === null ? null : item.contentSha256;
  if (
    contentSha256 !== null &&
    (typeof contentSha256 !== "string" || !SHA256_HEX.test(contentSha256))
  ) {
    throw new StorageContractError();
  }
  const validator =
    item.validator === null ? null : text(item.validator);
  const hasJpegEvidence =
    contentSha256 !== null && validator === "jpeg-structural-v1";
  const hasNoContentEvidence =
    contentSha256 === null && validator === null;
  if (
    (!hasJpegEvidence && !hasNoContentEvidence) ||
    (method === "carving" && (kind !== "file" || !hasJpegEvidence))
  ) {
    throw new StorageContractError();
  }
  return {
    id: opaqueId(item.id),
    displayPath: text(item.displayPath),
    kind,
    state: oneOf(item.state, [
      "exactEvidence",
      "likelyComplete",
      "completeUnvalidated",
      "structurallyValid",
      "partial",
      "conflicted",
      "readError",
      "zeroedOrTrimmed",
      "overwritten",
      "metadataOnly",
      "unknown",
    ]),
    sizeBytes: decimal(item.sizeBytes),
    metadataConfidence: oneOf(item.metadataConfidence, [
      "high",
      "medium",
      "low",
    ]),
    recoverabilityScore: score as number | null,
    pathState: oneOf(item.pathState, [
      "exact",
      "reconstructed",
      "incomplete",
      "orphaned",
      "ambiguous",
    ]),
    method,
    contentSha256,
    validator,
    warnings: warningList(item.warnings),
  };
}

export function parseCandidatePage(value: unknown): CandidatePage {
  const item = record(value);
  exactKeys(item, [
    "schemaVersion",
    "scanId",
    "cursor",
    "nextCursor",
    "candidates",
  ]);
  if (
    item.schemaVersion !== CANDIDATE_PAGE_SCHEMA_VERSION ||
    !Array.isArray(item.candidates) ||
    item.candidates.length > MAX_CANDIDATES_PER_PAGE
  ) {
    throw new StorageContractError();
  }
  const candidates = item.candidates.map(candidate);
  if (new Set(candidates.map((entry) => entry.id)).size !== candidates.length) {
    throw new StorageContractError();
  }
  return {
    schemaVersion: 2,
    scanId: opaqueId(item.scanId),
    cursor: nullableOpaqueId(item.cursor),
    nextCursor: nullableOpaqueId(item.nextCursor),
    candidates,
  };
}

function score(value: unknown, kind: CandidateKind): number | null {
  const validFileScore =
    kind === "file" &&
    typeof value === "number" &&
    Number.isInteger(value) &&
    value >= 0 &&
    value <= 100;
  const validDirectoryScore = kind === "directory" && value === null;
  if (!validFileScore && !validDirectoryScore) {
    throw new StorageContractError();
  }
  return value as number | null;
}

function normalizedExtension(value: unknown): string {
  const extension = text(value, true);
  if (
    [...extension].length > 255 ||
    extension !== extension.toLowerCase() ||
    extension.includes(".") ||
    extension.includes("/") ||
    extension.includes("\\") ||
    extension.includes("\uFEFF") ||
    extension.trim() !== extension
  ) {
    throw new StorageContractError();
  }
  return extension;
}

function actionableCandidate(value: unknown): ActionableCandidateRow {
  const item = record(value);
  exactKeys(item, [
    "id",
    "displayPath",
    "extension",
    "kind",
    "state",
    "sizeBytes",
    "metadataConfidence",
    "recoverabilityScore",
    "pathState",
    "method",
    "eligibility",
    "selected",
    "warnings",
  ]);
  const kind = oneOf(item.kind, ["file", "directory"]);
  return {
    id: decimal(item.id),
    displayPath: text(item.displayPath),
    extension: normalizedExtension(item.extension),
    kind,
    state: oneOf(item.state, [
      "exactEvidence",
      "likelyComplete",
      "completeUnvalidated",
      "structurallyValid",
      "partial",
      "conflicted",
      "readError",
      "zeroedOrTrimmed",
      "overwritten",
      "metadataOnly",
      "unknown",
    ]),
    sizeBytes: decimal(item.sizeBytes),
    metadataConfidence: oneOf(item.metadataConfidence, [
      "high",
      "medium",
      "low",
    ]),
    recoverabilityScore: score(item.recoverabilityScore, kind),
    pathState: oneOf(item.pathState, [
      "exact",
      "reconstructed",
      "incomplete",
      "orphaned",
      "ambiguous",
    ]),
    method: oneOf(item.method, [
      "ntfsMetadata",
      "fatMetadata",
      "exfatMetadata",
      "carving",
      "recycleBin",
    ]),
    eligibility: oneOf(item.eligibility, [
      "complete",
      "bestEffort",
      "ineligible",
    ]),
    selected: boolean(item.selected),
    warnings: warningList(item.warnings),
  };
}

function extensionFacet(value: unknown): CandidateExtensionFacet {
  const item = record(value);
  exactKeys(item, ["extension", "count"]);
  return {
    extension: normalizedExtension(item.extension),
    count: decimal(item.count),
  };
}

function selectionSummary(value: unknown): CandidateSelectionSummary {
  const item = record(value);
  exactKeys(item, [
    "selectionRevision",
    "selectedCandidates",
    "selectedFiles",
    "selectedDirectories",
    "selectedLogicalBytes",
    "bestEffortCandidates",
    "conflictedCandidates",
    "ineligibleCandidates",
    "matchingSelectedCandidates",
  ]);
  const parsed = {
    selectionRevision: decimal(item.selectionRevision),
    selectedCandidates: decimal(item.selectedCandidates),
    selectedFiles: decimal(item.selectedFiles),
    selectedDirectories: decimal(item.selectedDirectories),
    selectedLogicalBytes: decimal(item.selectedLogicalBytes),
    bestEffortCandidates: decimal(item.bestEffortCandidates),
    conflictedCandidates: decimal(item.conflictedCandidates),
    ineligibleCandidates: decimal(item.ineligibleCandidates),
    matchingSelectedCandidates: decimal(item.matchingSelectedCandidates),
  };
  const selected = BigInt(parsed.selectedCandidates);
  if (
    BigInt(parsed.selectedFiles) + BigInt(parsed.selectedDirectories) !==
      selected ||
    BigInt(parsed.bestEffortCandidates) > selected ||
    BigInt(parsed.conflictedCandidates) > selected ||
    BigInt(parsed.ineligibleCandidates) > selected ||
    BigInt(parsed.matchingSelectedCandidates) > selected
  ) {
    throw new StorageContractError();
  }
  return parsed;
}

export function parseCandidateQueryPage(value: unknown): CandidateQueryPage {
  const item = record(value);
  exactKeys(item, [
    "schemaVersion",
    "scanId",
    "queryId",
    "queryRevision",
    "cursor",
    "nextCursor",
    "filteredTotal",
    "extensionFacets",
    "selection",
    "candidates",
  ]);
  if (
    item.schemaVersion !== CANDIDATE_QUERY_SCHEMA_VERSION ||
    !Array.isArray(item.extensionFacets) ||
    item.extensionFacets.length > MAX_RETAINED_CANDIDATES_PER_SCAN ||
    !Array.isArray(item.candidates) ||
    item.candidates.length > MAX_ACTIONABLE_CANDIDATES_PER_PAGE
  ) {
    throw new StorageContractError();
  }
  const extensionFacets = item.extensionFacets.map(extensionFacet);
  if (
    new Set(extensionFacets.map((facet) => facet.extension)).size !==
    extensionFacets.length
  ) {
    throw new StorageContractError();
  }
  const candidates = item.candidates.map(actionableCandidate);
  if (new Set(candidates.map((entry) => entry.id)).size !== candidates.length) {
    throw new StorageContractError();
  }
  const filteredTotal = decimal(item.filteredTotal);
  const selection = selectionSummary(item.selection);
  if (
    BigInt(filteredTotal) < BigInt(candidates.length) ||
    BigInt(selection.matchingSelectedCandidates) > BigInt(filteredTotal)
  ) {
    throw new StorageContractError();
  }
  return {
    schemaVersion: 1,
    scanId: opaqueId(item.scanId),
    queryId: opaqueId(item.queryId),
    queryRevision: decimal(item.queryRevision),
    cursor: nullableOpaqueId(item.cursor),
    nextCursor: nullableOpaqueId(item.nextCursor),
    filteredTotal,
    extensionFacets,
    selection,
    candidates,
  };
}

export function parseCandidateSelectionUpdate(
  value: unknown,
): CandidateSelectionUpdate {
  const item = record(value);
  exactKeys(item, [
    "schemaVersion",
    "scanId",
    "queryId",
    "selectionRevision",
    "selection",
  ]);
  if (item.schemaVersion !== CANDIDATE_QUERY_SCHEMA_VERSION) {
    throw new StorageContractError();
  }
  const selectionRevision = decimal(item.selectionRevision);
  const selection = selectionSummary(item.selection);
  if (selection.selectionRevision !== selectionRevision) {
    throw new StorageContractError();
  }
  return {
    schemaVersion: 1,
    scanId: opaqueId(item.scanId),
    queryId: opaqueId(item.queryId),
    selectionRevision,
    selection,
  };
}
