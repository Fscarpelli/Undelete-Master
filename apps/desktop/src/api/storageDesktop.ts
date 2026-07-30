import { invoke, isTauri } from "@tauri-apps/api/core";
import {
  parseCandidatePage,
  parseFolderSelection,
  parseScanSummary,
  parseStorageInventory,
  StorageContractError,
  type CandidatePage,
  type FolderSelection,
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
  | "UAC_CANCELLED"
  | "BROKER_UNAVAILABLE"
  | "BROKER_PROTOCOL"
  | "SOURCE_IO"
  | "SCAN_CORRUPT"
  | "SCAN_INTERNAL"
  | "REPORT_INCOMPATIBLE";

const KNOWN_ERROR_CODES = new Set<StorageErrorCode>([
  "DESKTOP_RUNTIME_UNAVAILABLE",
  "INVENTORY_UNAVAILABLE",
  "SOURCE_UNSUPPORTED",
  "SOURCE_GONE",
  "SOURCE_IDENTITY_CHANGED",
  "FOLDER_SCOPE_UNSUPPORTED",
  "FOLDER_SCOPE_MISMATCH",
  "UAC_CANCELLED",
  "BROKER_UNAVAILABLE",
  "BROKER_PROTOCOL",
  "SOURCE_IO",
  "SCAN_CORRUPT",
  "SCAN_INTERNAL",
  "REPORT_INCOMPATIBLE",
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
): Promise<ScanSummary> {
  requireDesktop();
  const response = await invoke<unknown>("scan_storage_volume", {
    requestId,
    generation,
    volumeId,
    scopeId,
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
