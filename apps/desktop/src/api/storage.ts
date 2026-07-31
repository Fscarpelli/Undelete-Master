export const STORAGE_CONTRACT_SCHEMA_VERSION = 1;
export const SCAN_SUMMARY_SCHEMA_VERSION = 3;
export const CANDIDATE_PAGE_SCHEMA_VERSION = 2;
export const CANDIDATE_QUERY_SCHEMA_VERSION = 1;
export const RESTORE_CONTRACT_SCHEMA_VERSION = 1;

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

export type RestoreCollisionPolicy = "rename";
export type PartialFilePolicy = "completeOnly" | "zeroFillAndMap";
export type RestoreJobStatus =
  | "queued"
  | "running"
  | "cancelling"
  | "completed"
  | "failed"
  | "cancelled";

export interface RestoreDestinationSummary {
  schemaVersion: 1;
  destinationId: string;
  label: string;
  volumeLabel: string;
  fileSystem: "NTFS";
  freeBytes: string;
  relation: "different";
}

export interface RestorePlanSummary {
  schemaVersion: 1;
  planId: string;
  planDigest: string;
  scanId: string;
  destinationId: string;
  selectionRevision: string;
  collisionPolicy: RestoreCollisionPolicy;
  partialFilePolicy: PartialFilePolicy;
  itemsTotal: string;
  filesTotal: string;
  directoriesTotal: string;
  logicalBytes: string;
  bestEffortItems: string;
}

export interface RestoreCurrentItem {
  ordinal: string;
  candidateId: string;
  kind: CandidateKind;
}

export type RestoreCompletionStatus =
  | "completedDurable"
  | "needsReconciliation";

export interface RestoreManifestSummary {
  manifestSha256: string;
  completionStatus: RestoreCompletionStatus;
  publishedItems: string;
  partialItems: string;
}

export interface RestoreJobSnapshot {
  schemaVersion: 1;
  jobId: string;
  planId: string;
  status: RestoreJobStatus;
  itemsTotal: string;
  itemsCompleted: string;
  itemsFailed: string;
  itemsCancelled: string;
  bytesTotal: string;
  bytesCompleted: string;
  currentItem: RestoreCurrentItem | null;
  warnings: string[];
  manifest: RestoreManifestSummary | null;
}

export interface OpenRestoreDestinationResponse {
  schemaVersion: 1;
  opened: true;
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

export function parseRestoreOpaqueId(value: unknown): string {
  return opaqueId(value);
}

export function parseRestoreSelectionRevision(value: unknown): string {
  return decimal(value);
}

export function parseRestoreCollisionPolicy(
  value: unknown,
): RestoreCollisionPolicy {
  return oneOf(value, ["rename"]);
}

export function parsePartialFilePolicy(value: unknown): PartialFilePolicy {
  return oneOf(value, ["completeOnly", "zeroFillAndMap"]);
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

function restoreSummaryLabel(value: unknown): string {
  const label = text(value);
  if (label.includes("/") || label.includes("\\")) {
    throw new StorageContractError();
  }
  return label;
}

function restoreWarningList(value: unknown): string[] {
  const warnings = warningList(value);
  if (
    warnings.some(
      (warning) =>
        warning.includes("/") ||
        warning.includes("\\") ||
        /(?:^|[^A-Za-z])[A-Za-z]:/u.test(warning),
    )
  ) {
    throw new StorageContractError();
  }
  return warnings;
}

function sha256(value: unknown): string {
  if (typeof value !== "string" || !SHA256_HEX.test(value)) {
    throw new StorageContractError();
  }
  return value;
}

export function parseRestoreDestinationSummary(
  value: unknown,
): RestoreDestinationSummary {
  const item = record(value);
  exactKeys(item, [
    "schemaVersion",
    "destinationId",
    "label",
    "volumeLabel",
    "fileSystem",
    "freeBytes",
    "relation",
  ]);
  if (
    item.schemaVersion !== RESTORE_CONTRACT_SCHEMA_VERSION ||
    item.fileSystem !== "NTFS" ||
    item.relation !== "different"
  ) {
    throw new StorageContractError();
  }
  return {
    schemaVersion: 1,
    destinationId: opaqueId(item.destinationId),
    label: restoreSummaryLabel(item.label),
    volumeLabel: restoreSummaryLabel(item.volumeLabel),
    fileSystem: "NTFS",
    freeBytes: decimal(item.freeBytes),
    relation: "different",
  };
}

export function parseRestorePlanSummary(value: unknown): RestorePlanSummary {
  const item = record(value);
  exactKeys(item, [
    "schemaVersion",
    "planId",
    "planDigest",
    "scanId",
    "destinationId",
    "selectionRevision",
    "collisionPolicy",
    "partialFilePolicy",
    "itemsTotal",
    "filesTotal",
    "directoriesTotal",
    "logicalBytes",
    "bestEffortItems",
  ]);
  if (item.schemaVersion !== RESTORE_CONTRACT_SCHEMA_VERSION) {
    throw new StorageContractError();
  }
  const parsed: RestorePlanSummary = {
    schemaVersion: 1,
    planId: opaqueId(item.planId),
    planDigest: sha256(item.planDigest),
    scanId: opaqueId(item.scanId),
    destinationId: opaqueId(item.destinationId),
    selectionRevision: decimal(item.selectionRevision),
    collisionPolicy: parseRestoreCollisionPolicy(item.collisionPolicy),
    partialFilePolicy: parsePartialFilePolicy(item.partialFilePolicy),
    itemsTotal: decimal(item.itemsTotal),
    filesTotal: decimal(item.filesTotal),
    directoriesTotal: decimal(item.directoriesTotal),
    logicalBytes: decimal(item.logicalBytes),
    bestEffortItems: decimal(item.bestEffortItems),
  };
  const itemsTotal = BigInt(parsed.itemsTotal);
  const filesTotal = BigInt(parsed.filesTotal);
  const directoriesTotal = BigInt(parsed.directoriesTotal);
  if (
    itemsTotal === 0n ||
    itemsTotal > BigInt(MAX_RETAINED_CANDIDATES_PER_SCAN) ||
    filesTotal + directoriesTotal !== itemsTotal ||
    BigInt(parsed.bestEffortItems) > filesTotal ||
    (parsed.partialFilePolicy === "completeOnly" &&
      parsed.bestEffortItems !== "0")
  ) {
    throw new StorageContractError();
  }
  return parsed;
}

function restoreCurrentItem(
  value: unknown,
  itemsTotal: bigint,
): RestoreCurrentItem | null {
  if (value === null) {
    return null;
  }
  const item = record(value);
  exactKeys(item, ["ordinal", "candidateId", "kind"]);
  const current = {
    ordinal: decimal(item.ordinal),
    candidateId: decimal(item.candidateId),
    kind: oneOf(item.kind, ["file", "directory"]),
  };
  if (BigInt(current.ordinal) >= itemsTotal) {
    throw new StorageContractError();
  }
  return current;
}

function restoreManifestSummary(
  value: unknown,
  itemsCompleted: bigint,
): RestoreManifestSummary | null {
  if (value === null) {
    return null;
  }
  const item = record(value);
  exactKeys(item, [
    "manifestSha256",
    "completionStatus",
    "publishedItems",
    "partialItems",
  ]);
  const manifest = {
    manifestSha256: sha256(item.manifestSha256),
    completionStatus: oneOf(item.completionStatus, [
      "completedDurable",
      "needsReconciliation",
    ]),
    publishedItems: decimal(item.publishedItems),
    partialItems: decimal(item.partialItems),
  };
  if (
    BigInt(manifest.publishedItems) > itemsCompleted ||
    BigInt(manifest.partialItems) > BigInt(manifest.publishedItems)
  ) {
    throw new StorageContractError();
  }
  return manifest;
}

function isTerminalRestoreStatus(status: RestoreJobStatus): boolean {
  return (
    status === "completed" || status === "failed" || status === "cancelled"
  );
}

const LEGAL_RESTORE_JOB_TRANSITIONS: Record<
  RestoreJobStatus,
  readonly RestoreJobStatus[]
> = {
  queued: [
    "queued",
    "running",
    "cancelling",
    "completed",
    "failed",
    "cancelled",
  ],
  running: ["running", "cancelling", "completed", "failed", "cancelled"],
  cancelling: ["cancelling", "completed", "failed", "cancelled"],
  completed: ["completed"],
  failed: ["failed"],
  cancelled: ["cancelled"],
};

function sameRestoreManifest(
  left: RestoreManifestSummary | null,
  right: RestoreManifestSummary | null,
): boolean {
  return (
    (left === null && right === null) ||
    (left !== null &&
      right !== null &&
      left.manifestSha256 === right.manifestSha256 &&
      left.completionStatus === right.completionStatus &&
      left.publishedItems === right.publishedItems &&
      left.partialItems === right.partialItems)
  );
}

function sameTerminalRestoreSnapshot(
  previous: RestoreJobSnapshot,
  current: RestoreJobSnapshot,
): boolean {
  return (
    previous.status === current.status &&
    previous.itemsCompleted === current.itemsCompleted &&
    previous.itemsFailed === current.itemsFailed &&
    previous.itemsCancelled === current.itemsCancelled &&
    previous.bytesCompleted === current.bytesCompleted &&
    previous.warnings.length === current.warnings.length &&
    previous.warnings.every(
      (warning, index) => warning === current.warnings[index],
    ) &&
    sameRestoreManifest(previous.manifest, current.manifest)
  );
}

export function parseRestoreJobSnapshot(
  value: unknown,
  previous?: RestoreJobSnapshot,
): RestoreJobSnapshot {
  const item = record(value);
  exactKeys(item, [
    "schemaVersion",
    "jobId",
    "planId",
    "status",
    "itemsTotal",
    "itemsCompleted",
    "itemsFailed",
    "itemsCancelled",
    "bytesTotal",
    "bytesCompleted",
    "currentItem",
    "warnings",
    "manifest",
  ]);
  if (item.schemaVersion !== RESTORE_CONTRACT_SCHEMA_VERSION) {
    throw new StorageContractError();
  }
  const status = oneOf(item.status, [
    "queued",
    "running",
    "cancelling",
    "completed",
    "failed",
    "cancelled",
  ]);
  const itemsTotal = decimal(item.itemsTotal);
  const itemsCompleted = decimal(item.itemsCompleted);
  const itemsFailed = decimal(item.itemsFailed);
  const itemsCancelled = decimal(item.itemsCancelled);
  const bytesTotal = decimal(item.bytesTotal);
  const bytesCompleted = decimal(item.bytesCompleted);
  const total = BigInt(itemsTotal);
  const completed = BigInt(itemsCompleted);
  const failed = BigInt(itemsFailed);
  const cancelled = BigInt(itemsCancelled);
  const dispositions = completed + failed + cancelled;
  const completedBytes = BigInt(bytesCompleted);
  if (
    total === 0n ||
    total > BigInt(MAX_RETAINED_CANDIDATES_PER_SCAN) ||
    dispositions > total ||
    completedBytes > BigInt(bytesTotal)
  ) {
    throw new StorageContractError();
  }
  const currentItem = restoreCurrentItem(item.currentItem, total);
  const manifest = restoreManifestSummary(item.manifest, completed);
  if (
    (!isTerminalRestoreStatus(status) && manifest !== null) ||
    ((status === "queued" || isTerminalRestoreStatus(status)) &&
      currentItem !== null) ||
    (currentItem !== null && BigInt(currentItem.ordinal) !== dispositions) ||
    (status === "queued" &&
      (completed !== 0n ||
        failed !== 0n ||
        cancelled !== 0n ||
        completedBytes !== 0n)) ||
    (status === "completed" &&
      (completed !== total ||
        failed !== 0n ||
        cancelled !== 0n ||
        completedBytes !== BigInt(bytesTotal) ||
        manifest === null ||
        BigInt(manifest.publishedItems) !== total))
  ) {
    throw new StorageContractError();
  }
  const parsed: RestoreJobSnapshot = {
    schemaVersion: 1,
    jobId: opaqueId(item.jobId),
    planId: opaqueId(item.planId),
    status,
    itemsTotal,
    itemsCompleted,
    itemsFailed,
    itemsCancelled,
    bytesTotal,
    bytesCompleted,
    currentItem,
    warnings: restoreWarningList(item.warnings),
    manifest,
  };
  if (
    previous !== undefined &&
    (previous.jobId !== parsed.jobId ||
      previous.planId !== parsed.planId ||
      previous.itemsTotal !== parsed.itemsTotal ||
      previous.bytesTotal !== parsed.bytesTotal ||
      !LEGAL_RESTORE_JOB_TRANSITIONS[previous.status].includes(parsed.status) ||
      BigInt(previous.itemsCompleted) > completed ||
      BigInt(previous.itemsFailed) > failed ||
      BigInt(previous.itemsCancelled) > cancelled ||
      BigInt(previous.bytesCompleted) > completedBytes ||
      (previous.currentItem !== null &&
        parsed.currentItem !== null &&
        (BigInt(parsed.currentItem.ordinal) <
          BigInt(previous.currentItem.ordinal) ||
          (parsed.currentItem.ordinal === previous.currentItem.ordinal &&
            (parsed.currentItem.candidateId !==
              previous.currentItem.candidateId ||
              parsed.currentItem.kind !== previous.currentItem.kind)))) ||
      (previous.currentItem !== null &&
        parsed.currentItem === null &&
        !isTerminalRestoreStatus(parsed.status) &&
        dispositions <=
          BigInt(previous.itemsCompleted) +
            BigInt(previous.itemsFailed) +
            BigInt(previous.itemsCancelled)) ||
      (isTerminalRestoreStatus(previous.status) &&
        !sameTerminalRestoreSnapshot(previous, parsed)))
  ) {
    throw new StorageContractError();
  }
  return parsed;
}

export function parseOpenRestoreDestinationResponse(
  value: unknown,
): OpenRestoreDestinationResponse {
  const item = record(value);
  exactKeys(item, ["schemaVersion", "opened"]);
  if (
    item.schemaVersion !== RESTORE_CONTRACT_SCHEMA_VERSION ||
    item.opened !== true
  ) {
    throw new StorageContractError();
  }
  return { schemaVersion: 1, opened: true };
}
