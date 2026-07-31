import { invoke, isTauri } from "@tauri-apps/api/core";
import {
  parseCandidatePage,
  parseCandidateQueryPage,
  parseCandidateSelectionUpdate,
  parseFolderSelection,
  parseOpenRestoreDestinationResponse,
  parsePartialFilePolicy,
  parseRestoreCollisionPolicy,
  parseRestoreDestinationSummary,
  parseRestoreJobSnapshot,
  parseRestoreOpaqueId,
  parseRestorePlanSummary,
  parseRestoreSelectionRevision,
  parseScanSummary,
  parseStorageInventory,
  StorageContractError,
  type CandidatePage,
  type CandidateQuery,
  type CandidateQueryPage,
  type CandidateSelectionOperation,
  type CandidateSelectionUpdate,
  type CandidateSort,
  type FolderSelection,
  type OpenRestoreDestinationResponse,
  type PartialFilePolicy,
  type RestoreCollisionPolicy,
  type RestoreDestinationSummary,
  type RestoreJobSnapshot,
  type RestorePlanSummary,
  type ScanMode,
  type ScanSummary,
  type StorageInventory,
} from "./storage";

export type StorageErrorCode =
  | "DESKTOP_RUNTIME_UNAVAILABLE"
  | "INVENTORY_UNAVAILABLE"
  | "SOURCE_UNSUPPORTED"
  | "SOURCE_GONE"
  | "SOURCE_IDENTITY_CHANGED"
  | "FOLDER_SCOPE_UNSUPPORTED"
  | "FOLDER_SCOPE_MISMATCH"
  | "SCAN_MODE_UNSUPPORTED"
  | "UAC_CANCELLED"
  | "BROKER_UNAVAILABLE"
  | "BROKER_PROTOCOL"
  | "SOURCE_IO"
  | "SCAN_CORRUPT"
  | "SCAN_INTERNAL"
  | "REPORT_INCOMPATIBLE";

export type RestoreErrorCode =
  | "REPORT_INCOMPATIBLE"
  | "RESTORE_DESTINATION_INVALID"
  | "RESTORE_DESTINATION_LIMIT"
  | "RESTORE_DESTINATION_EXPIRED"
  | "RESTORE_DIFFERENT_DISK_REQUIRED"
  | "RESTORE_SOURCE_CHANGED"
  | "RESTORE_SELECTION_STALE"
  | "RESTORE_SELECTION_EMPTY"
  | "RESTORE_ITEM_INELIGIBLE"
  | "RESTORE_PARTIAL_POLICY_REQUIRED"
  | "RESTORE_PLAN_INVALID"
  | "RESTORE_PLAN_EXPIRED"
  | "RESTORE_JOB_LIMIT"
  | "RESTORE_JOB_NOT_FOUND"
  | "RESTORE_JOB_NOT_COMPLETE"
  | "RESTORE_INTERNAL";

const KNOWN_ERROR_CODES = new Set<StorageErrorCode>([
  "DESKTOP_RUNTIME_UNAVAILABLE",
  "INVENTORY_UNAVAILABLE",
  "SOURCE_UNSUPPORTED",
  "SOURCE_GONE",
  "SOURCE_IDENTITY_CHANGED",
  "FOLDER_SCOPE_UNSUPPORTED",
  "FOLDER_SCOPE_MISMATCH",
  "SCAN_MODE_UNSUPPORTED",
  "UAC_CANCELLED",
  "BROKER_UNAVAILABLE",
  "BROKER_PROTOCOL",
  "SOURCE_IO",
  "SCAN_CORRUPT",
  "SCAN_INTERNAL",
  "REPORT_INCOMPATIBLE",
]);

const KNOWN_RESTORE_ERROR_CODES = new Set<RestoreErrorCode>([
  "REPORT_INCOMPATIBLE",
  "RESTORE_DESTINATION_INVALID",
  "RESTORE_DESTINATION_LIMIT",
  "RESTORE_DESTINATION_EXPIRED",
  "RESTORE_DIFFERENT_DISK_REQUIRED",
  "RESTORE_SOURCE_CHANGED",
  "RESTORE_SELECTION_STALE",
  "RESTORE_SELECTION_EMPTY",
  "RESTORE_ITEM_INELIGIBLE",
  "RESTORE_PARTIAL_POLICY_REQUIRED",
  "RESTORE_PLAN_INVALID",
  "RESTORE_PLAN_EXPIRED",
  "RESTORE_JOB_LIMIT",
  "RESTORE_JOB_NOT_FOUND",
  "RESTORE_JOB_NOT_COMPLETE",
  "RESTORE_INTERNAL",
]);

export class StorageCommandError extends Error {
  readonly code: StorageErrorCode;

  constructor(code: StorageErrorCode) {
    super(code);
    this.name = "StorageCommandError";
    this.code = code;
  }
}

function requireDesktop() {
  if (!isTauri()) {
    throw new StorageCommandError("DESKTOP_RUNTIME_UNAVAILABLE");
  }
}

function parseResponse<T>(parser: (value: unknown) => T, value: unknown): T {
  try {
    return parser(value);
  } catch (error) {
    if (error instanceof StorageContractError) {
      throw new StorageCommandError("REPORT_INCOMPATIBLE");
    }
    throw error;
  }
}

export class RestoreCommandError extends Error {
  readonly code: RestoreErrorCode;

  constructor(code: RestoreErrorCode) {
    super(code);
    this.name = "RestoreCommandError";
    this.code = code;
  }
}

const MAX_TRACKED_RESTORE_JOBS = 8;
const restoreJobSnapshots = new Map<string, RestoreJobSnapshot>();

function opaqueRestoreId(value: string): string {
  return parseResponse(parseRestoreOpaqueId, value);
}

function restoreSelectionRevision(value: string): string {
  return parseResponse(parseRestoreSelectionRevision, value);
}

function rememberRestoreSnapshot(snapshot: RestoreJobSnapshot) {
  if (
    !restoreJobSnapshots.has(snapshot.jobId) &&
    restoreJobSnapshots.size === MAX_TRACKED_RESTORE_JOBS
  ) {
    const oldest = restoreJobSnapshots.keys().next().value;
    if (oldest !== undefined) {
      restoreJobSnapshots.delete(oldest);
    }
  }
  restoreJobSnapshots.set(snapshot.jobId, snapshot);
}

function parseTrackedRestoreJob(
  value: unknown,
  expectedJobId?: string,
  expectedPlanId?: string,
): RestoreJobSnapshot {
  const previous =
    expectedJobId === undefined
      ? undefined
      : restoreJobSnapshots.get(expectedJobId);
  let snapshot = parseResponse(
    (response) => parseRestoreJobSnapshot(response, previous),
    value,
  );
  if (
    (expectedJobId !== undefined && snapshot.jobId !== expectedJobId) ||
    (expectedPlanId !== undefined && snapshot.planId !== expectedPlanId)
  ) {
    throw new StorageCommandError("REPORT_INCOMPATIBLE");
  }
  if (expectedJobId === undefined) {
    const existing = restoreJobSnapshots.get(snapshot.jobId);
    if (existing !== undefined) {
      snapshot = parseResponse(
        (response) => parseRestoreJobSnapshot(response, existing),
        value,
      );
    }
  }
  rememberRestoreSnapshot(snapshot);
  return snapshot;
}

export function normalizeStorageError(error: unknown): StorageCommandError {
  if (error instanceof StorageCommandError) {
    return error;
  }
  if (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    typeof error.code === "string" &&
    KNOWN_ERROR_CODES.has(error.code as StorageErrorCode)
  ) {
    return new StorageCommandError(error.code as StorageErrorCode);
  }
  return new StorageCommandError("SCAN_INTERNAL");
}

export function normalizeRestoreError(error: unknown): RestoreCommandError {
  if (error instanceof RestoreCommandError) {
    return error;
  }
  if (
    error instanceof StorageCommandError &&
    error.code === "REPORT_INCOMPATIBLE"
  ) {
    return new RestoreCommandError("REPORT_INCOMPATIBLE");
  }
  if (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    typeof error.code === "string" &&
    KNOWN_RESTORE_ERROR_CODES.has(error.code as RestoreErrorCode)
  ) {
    return new RestoreCommandError(error.code as RestoreErrorCode);
  }
  return new RestoreCommandError("RESTORE_INTERNAL");
}

export async function listStorageSources(
  requestId: string,
): Promise<StorageInventory> {
  requireDesktop();
  const response = await invoke<unknown>("list_storage_sources", { requestId });
  return parseResponse(parseStorageInventory, response);
}

export async function selectScanFolder(
  requestId: string,
  generation: string,
  volumeId: string,
): Promise<FolderSelection | null> {
  requireDesktop();
  const response = await invoke<unknown>("select_scan_folder", {
    requestId,
    generation,
    volumeId,
  });
  return response === null
    ? null
    : parseResponse(parseFolderSelection, response);
}

export async function scanStorageVolume(
  requestId: string,
  generation: string,
  volumeId: string,
  scopeId: string | null,
  mode: ScanMode,
): Promise<ScanSummary> {
  requireDesktop();
  const response = await invoke<unknown>("scan_storage_volume", {
    requestId,
    generation,
    volumeId,
    scopeId,
    mode,
  });
  return parseResponse(parseScanSummary, response);
}

export async function getCandidatePage(
  requestId: string,
  scanId: string,
  cursor: string | null,
): Promise<CandidatePage> {
  requireDesktop();
  const response = await invoke<unknown>("get_candidate_page", {
    requestId,
    scanId,
    cursor,
    limit: 100,
  });
  const page = parseResponse(parseCandidatePage, response);
  if (page.candidates.length > 100) {
    throw new StorageCommandError("REPORT_INCOMPATIBLE");
  }
  return page;
}

export async function queryCandidatePage(
  requestId: string,
  scanId: string,
  query: CandidateQuery,
  sort: CandidateSort,
  cursor: string | null,
): Promise<CandidateQueryPage> {
  requireDesktop();
  const response = await invoke<unknown>("query_candidate_page", {
    requestId,
    scanId,
    query,
    sort,
    cursor,
  });
  return parseResponse(parseCandidateQueryPage, response);
}

export async function updateCandidateSelection(
  requestId: string,
  scanId: string,
  queryId: string,
  operation: CandidateSelectionOperation,
  selectionRevision: string,
): Promise<CandidateSelectionUpdate> {
  requireDesktop();
  const response = await invoke<unknown>("update_candidate_selection", {
    requestId,
    scanId,
    queryId,
    operation,
    selectionRevision,
  });
  return parseResponse(parseCandidateSelectionUpdate, response);
}

export async function selectRestoreDestination(
  requestId: string,
  scanId: string,
): Promise<RestoreDestinationSummary | null> {
  requireDesktop();
  const parsedRequestId = opaqueRestoreId(requestId);
  const parsedScanId = opaqueRestoreId(scanId);
  const response = await invoke<unknown>("select_restore_destination", {
    requestId: parsedRequestId,
    scanId: parsedScanId,
  });
  return response === null
    ? null
    : parseResponse(parseRestoreDestinationSummary, response);
}

export async function createRestorePlan(
  requestId: string,
  scanId: string,
  selectionRevision: string,
  destinationId: string,
  collisionPolicy: RestoreCollisionPolicy,
  partialFilePolicy: PartialFilePolicy,
): Promise<RestorePlanSummary> {
  requireDesktop();
  const args = {
    requestId: opaqueRestoreId(requestId),
    scanId: opaqueRestoreId(scanId),
    selectionRevision: restoreSelectionRevision(selectionRevision),
    destinationId: opaqueRestoreId(destinationId),
    collisionPolicy: parseResponse(
      parseRestoreCollisionPolicy,
      collisionPolicy,
    ),
    partialFilePolicy: parseResponse(
      parsePartialFilePolicy,
      partialFilePolicy,
    ),
  };
  const response = await invoke<unknown>("create_restore_plan", args);
  const plan = parseResponse(parseRestorePlanSummary, response);
  if (
    plan.scanId !== args.scanId ||
    plan.destinationId !== args.destinationId ||
    plan.selectionRevision !== args.selectionRevision ||
    plan.collisionPolicy !== args.collisionPolicy ||
    plan.partialFilePolicy !== args.partialFilePolicy
  ) {
    throw new StorageCommandError("REPORT_INCOMPATIBLE");
  }
  return plan;
}

export async function startRestore(
  requestId: string,
  planId: string,
): Promise<RestoreJobSnapshot> {
  requireDesktop();
  const args = {
    requestId: opaqueRestoreId(requestId),
    planId: opaqueRestoreId(planId),
  };
  const response = await invoke<unknown>("start_restore", args);
  return parseTrackedRestoreJob(response, undefined, args.planId);
}

export async function getRestoreJob(
  requestId: string,
  jobId: string,
): Promise<RestoreJobSnapshot> {
  requireDesktop();
  const args = {
    requestId: opaqueRestoreId(requestId),
    jobId: opaqueRestoreId(jobId),
  };
  const response = await invoke<unknown>("get_restore_job", args);
  return parseTrackedRestoreJob(response, args.jobId);
}

export async function cancelRestore(
  requestId: string,
  jobId: string,
): Promise<RestoreJobSnapshot> {
  requireDesktop();
  const args = {
    requestId: opaqueRestoreId(requestId),
    jobId: opaqueRestoreId(jobId),
  };
  const response = await invoke<unknown>("cancel_restore", args);
  return parseTrackedRestoreJob(response, args.jobId);
}

export async function openRestoreDestination(
  requestId: string,
  jobId: string,
): Promise<OpenRestoreDestinationResponse> {
  requireDesktop();
  const args = {
    requestId: opaqueRestoreId(requestId),
    jobId: opaqueRestoreId(jobId),
  };
  const response = await invoke<unknown>("open_restore_destination", args);
  return parseResponse(parseOpenRestoreDestinationResponse, response);
}
