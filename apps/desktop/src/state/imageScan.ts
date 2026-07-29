import { useEffect, useRef, useState } from "react";
import {
  normalizeDesktopError,
  selectAndScanImage,
  type DesktopErrorCode,
} from "../api/desktop";
import type { DesktopScanReport } from "../api/report";

export type ScanState =
  | { phase: "idle"; cancelled: boolean }
  | { phase: "pending" }
  | { phase: "success"; report: DesktopScanReport }
  | { phase: "error"; code: DesktopErrorCode };

let requestSequence = 0;

function nextRequestId(): string {
  requestSequence += 1;
  return `scan-${requestSequence.toString(36)}`;
}

export function useImageScan() {
  const [state, setState] = useState<ScanState>({
    phase: "idle",
    cancelled: false,
  });
  const inFlight = useRef(false);
  const generation = useRef(0);

  useEffect(
    () => () => {
      generation.current += 1;
      inFlight.current = false;
    },
    [],
  );

  const start = async () => {
    if (inFlight.current) {
      return;
    }

    inFlight.current = true;
    const requestGeneration = generation.current + 1;
    generation.current = requestGeneration;
    setState({ phase: "pending" });

    try {
      const report = await selectAndScanImage(nextRequestId());
      if (generation.current !== requestGeneration) {
        return;
      }
      if (report === null) {
        setState({ phase: "idle", cancelled: true });
      } else {
        setState({ phase: "success", report });
      }
    } catch (error) {
      if (generation.current === requestGeneration) {
        setState({ phase: "error", code: normalizeDesktopError(error).code });
      }
    } finally {
      if (generation.current === requestGeneration) {
        inFlight.current = false;
      }
    }
  };

  return { state, start };
}
