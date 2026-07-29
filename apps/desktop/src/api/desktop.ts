import { invoke, isTauri } from "@tauri-apps/api/core";
import {
  parseDesktopScanReport,
  type DesktopScanReport,
} from "./report";

export type DesktopErrorCode =
  | "DESKTOP_RUNTIME_UNAVAILABLE"
  | "SOURCE_FORBIDDEN"
  | "SOURCE_UNSUPPORTED"
  | "SOURCE_NOT_REGULAR"
  | "SOURCE_EMPTY"
  | "SOURCE_IO"
  | "SCAN_CORRUPT"
  | "SCAN_REPORT_TOO_LARGE"
  | "SCAN_INTERNAL"
  | "REPORT_INCOMPATIBLE";

const KNOWN_ERROR_CODES = new Set<DesktopErrorCode>([
  "DESKTOP_RUNTIME_UNAVAILABLE",
  "SOURCE_FORBIDDEN",
  "SOURCE_UNSUPPORTED",
  "SOURCE_NOT_REGULAR",
  "SOURCE_EMPTY",
  "SOURCE_IO",
  "SCAN_CORRUPT",
  "SCAN_REPORT_TOO_LARGE",
  "SCAN_INTERNAL",
  "REPORT_INCOMPATIBLE",
]);

export class DesktopCommandError extends Error {
  readonly code: DesktopErrorCode;

  constructor(code: DesktopErrorCode) {
    super(code);
    this.name = "DesktopCommandError";
    this.code = code;
  }
}
export function desktopRuntimeAvailable(): boolean {
  return isTauri();
}

export function normalizeDesktopError(error: unknown): DesktopCommandError {
  if (error instanceof DesktopCommandError) {
    return error;
  }
  if (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    typeof error.code === "string" &&
    KNOWN_ERROR_CODES.has(error.code as DesktopErrorCode)
  ) {
    return new DesktopCommandError(error.code as DesktopErrorCode);
  }
  return new DesktopCommandError("SCAN_INTERNAL");
}

export async function selectAndScanImage(
  requestId: string,
): Promise<DesktopScanReport | null> {
  if (!desktopRuntimeAvailable()) {
    throw new DesktopCommandError("DESKTOP_RUNTIME_UNAVAILABLE");
  }

  const response = await invoke<unknown>("select_and_scan_image", { requestId });
  if (response === null) {
    return null;
  }

  try {
    return parseDesktopScanReport(response);
  } catch {
    throw new DesktopCommandError("REPORT_INCOMPATIBLE");
  }
}
