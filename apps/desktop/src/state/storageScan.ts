import { useCallback, useEffect, useRef, useState } from "react";
import type {
  FolderSelection,
  ScanProgressEvent,
  ScanMode,
  ScanSummary,
  StorageInventory,
} from "../api/storage";
import {
  listStorageSources,
  normalizeStorageError,
  scanStorageVolume,
  listenScanProgress,
  selectScanFolder,
  StorageCommandError,
  type StorageErrorCode,
} from "../api/storageDesktop";

type InventoryPhase =
  | "unavailable"
  | "loading"
  | "refreshing"
  | "ready"
  | "error";
type FolderPhase = "idle" | "selecting";
type ScanPhase = "idle" | "scanning" | "success" | "error";

export interface StorageWorkflowState {
  inventoryPhase: InventoryPhase;
  inventory: StorageInventory | null;
  inventoryError: StorageErrorCode | null;
  selectedVolumeId: string | null;
  folderSelection: FolderSelection | null;
  scanMode: ScanMode;
  folderPhase: FolderPhase;
  folderCancelled: boolean;
  scanPhase: ScanPhase;
  scanError: StorageErrorCode | null;
  scanProgress: ScanProgressEvent | null;
  scanStartedAt: number | null;
  summary: ScanSummary | null;
}

let requestSequence = 0;

function nextRequestId(kind: "inventory" | "folder" | "scan") {
  requestSequence += 1;
  return `${kind}-${requestSequence.toString(36)}`;
}

function initialState(runtimeAvailable: boolean): StorageWorkflowState {
  return {
    inventoryPhase: runtimeAvailable ? "loading" : "unavailable",
    inventory: null,
    inventoryError: null,
    selectedVolumeId: null,
    folderSelection: null,
    scanMode: "metadata",
    folderPhase: "idle",
    folderCancelled: false,
    scanPhase: "idle",
    scanError: null,
    scanProgress: null,
    scanStartedAt: null,
    summary: null,
  };
}

function inventoryContainsVolume(
  inventory: StorageInventory,
  volumeId: string,
) {
  return inventory.disks.some((disk) =>
    disk.volumes.some((volume) => volume.id === volumeId),
  );
}

function clearScan(): Pick<
  StorageWorkflowState,
  "scanPhase" | "scanError" | "scanProgress" | "scanStartedAt" | "summary"
> {
  return {
    scanPhase: "idle",
    scanError: null,
    scanProgress: null,
    scanStartedAt: null,
    summary: null,
  };
}

export function useStorageScan(runtimeAvailable: boolean) {
  const [state, setState] = useState<StorageWorkflowState>(() =>
    initialState(runtimeAvailable),
  );
  const stateRef = useRef(state);
  stateRef.current = state;

  const mounted = useRef(false);
  const inventoryInFlight = useRef(false);
  const folderInFlight = useRef(false);
  const scanInFlight = useRef(false);
  const inventoryGeneration = useRef(0);
  const scanGeneration = useRef(0);

  const refreshInventory = useCallback(async () => {
    if (
      !runtimeAvailable ||
      inventoryInFlight.current ||
      scanInFlight.current
    ) {
      return;
    }

    inventoryInFlight.current = true;
    inventoryGeneration.current += 1;
    const requestGeneration = inventoryGeneration.current;
    setState((current) => ({
      ...current,
      inventoryPhase: current.inventory === null ? "loading" : "refreshing",
      inventoryError: null,
      folderCancelled: false,
    }));

    try {
      const inventory = await listStorageSources(nextRequestId("inventory"));
      if (
        !mounted.current ||
        inventoryGeneration.current !== requestGeneration
      ) {
        return;
      }
      setState((current) => {
        const selectedVolumeId =
          current.selectedVolumeId !== null &&
          inventoryContainsVolume(inventory, current.selectedVolumeId)
            ? current.selectedVolumeId
            : null;
        return {
          ...current,
          inventoryPhase: "ready",
          inventory,
          inventoryError: null,
          selectedVolumeId,
          folderSelection: null,
          scanMode: "metadata",
          folderPhase: "idle",
          folderCancelled: false,
          ...clearScan(),
        };
      });
    } catch (error) {
      if (
        mounted.current &&
        inventoryGeneration.current === requestGeneration
      ) {
        setState((current) => ({
          ...current,
          inventoryPhase: "error",
          inventoryError: normalizeStorageError(error).code,
        }));
      }
    } finally {
      if (inventoryGeneration.current === requestGeneration) {
        inventoryInFlight.current = false;
      }
    }
  }, [runtimeAvailable]);

  useEffect(() => {
    mounted.current = true;
    if (runtimeAvailable) {
      void refreshInventory();
    }
    return () => {
      mounted.current = false;
      inventoryGeneration.current += 1;
      scanGeneration.current += 1;
      inventoryInFlight.current = false;
      folderInFlight.current = false;
      scanInFlight.current = false;
    };
  }, [refreshInventory, runtimeAvailable]);

  const selectVolume = useCallback((volumeId: string) => {
    if (scanInFlight.current) {
      return;
    }
    setState((current) => {
      if (
        current.inventory === null ||
        !inventoryContainsVolume(current.inventory, volumeId)
      ) {
        return current;
      }
      return {
        ...current,
        selectedVolumeId: volumeId,
        folderSelection: null,
        scanMode: "metadata",
        folderPhase: "idle",
        folderCancelled: false,
        ...clearScan(),
      };
    });
  }, []);

  const selectFolder = useCallback(async () => {
    const current = stateRef.current;
    const volumeId = current.selectedVolumeId;
    const generation = current.inventory?.generation;
    const volume = current.inventory?.disks
      .flatMap((disk) => disk.volumes)
      .find((item) => item.id === volumeId);
    if (
      volumeId === null ||
      generation === undefined ||
      volume?.folderScopeSupported !== true ||
      folderInFlight.current ||
      scanInFlight.current
    ) {
      return;
    }

    folderInFlight.current = true;
    const requestGeneration = scanGeneration.current;
    setState((latest) => ({
      ...latest,
      folderPhase: "selecting",
      folderCancelled: false,
      scanError: null,
    }));

    try {
      const selection = await selectScanFolder(
        nextRequestId("folder"),
        generation,
        volumeId,
      );
      if (
        !mounted.current ||
        scanGeneration.current !== requestGeneration ||
        stateRef.current.inventory?.generation !== generation ||
        stateRef.current.selectedVolumeId !== volumeId
      ) {
        return;
      }
      if (selection === null) {
        setState((latest) => ({
          ...latest,
          folderPhase: "idle",
          folderCancelled: true,
        }));
      } else if (selection.volumeId !== volumeId) {
        throw new StorageCommandError("FOLDER_SCOPE_MISMATCH");
      } else {
        setState((latest) => ({
          ...latest,
          folderSelection: selection,
          scanMode: "metadata",
          folderPhase: "idle",
          folderCancelled: false,
          ...clearScan(),
        }));
      }
    } catch (error) {
      if (
        mounted.current &&
        scanGeneration.current === requestGeneration
      ) {
        setState((latest) => ({
          ...latest,
          folderPhase: "idle",
          folderCancelled: false,
          scanPhase: "error",
          scanError: normalizeStorageError(error).code,
        }));
      }
    } finally {
      folderInFlight.current = false;
    }
  }, []);

  const clearFolder = useCallback(() => {
    if (folderInFlight.current || scanInFlight.current) {
      return;
    }
    setState((current) => ({
      ...current,
      folderSelection: null,
      folderCancelled: false,
      ...clearScan(),
    }));
  }, []);

  const selectScanMode = useCallback((mode: ScanMode) => {
    if (scanInFlight.current || folderInFlight.current) {
      return;
    }
    setState((current) => {
      const volume = current.inventory?.disks
        .flatMap((disk) => disk.volumes)
        .find((item) => item.id === current.selectedVolumeId);
      const deepJpegAvailable =
        current.folderSelection === null &&
        volume?.fileSystem.toLocaleLowerCase("en-US") === "ntfs";
      if (mode === "deepJpeg" && !deepJpegAvailable) {
        return current;
      }
      return {
        ...current,
        scanMode: mode,
        ...clearScan(),
      };
    });
  }, []);

  const startScan = useCallback(async () => {
    const current = stateRef.current;
    const volumeId = current.selectedVolumeId;
    const generation = current.inventory?.generation;
    const volume = current.inventory?.disks
      .flatMap((disk) => disk.volumes)
      .find((item) => item.id === volumeId);
    if (
      volumeId === null ||
      generation === undefined ||
      volume?.scanSupported !== true ||
      (current.scanMode === "deepJpeg" &&
        (current.folderSelection !== null ||
          volume.fileSystem.toLocaleLowerCase("en-US") !== "ntfs")) ||
      scanInFlight.current ||
      folderInFlight.current
    ) {
      return;
    }

    scanInFlight.current = true;
    scanGeneration.current += 1;
    const requestGeneration = scanGeneration.current;
    const scopeId =
      current.folderSelection?.volumeId === volumeId
        ? current.folderSelection.scopeId
        : null;
    const scanMode = current.scanMode;
    setState((latest) => ({
      ...latest,
      scanPhase: "scanning",
      scanError: null,
      scanProgress: null,
      scanStartedAt: Date.now(),
      summary: null,
      folderCancelled: false,
    }));

    const scanRequestId = nextRequestId("scan");
    const unlistenPromise = listenScanProgress(scanRequestId, (progress) => {
      if (
        mounted.current &&
        scanGeneration.current === requestGeneration
      ) {
        setState((latest) => ({ ...latest, scanProgress: progress }));
      }
    });
    try {
      const completedSummary = await scanStorageVolume(
        scanRequestId,
        generation,
        volumeId,
        scopeId,
        scanMode,
      );
      if (
        !mounted.current ||
        scanGeneration.current !== requestGeneration
      ) {
        return;
      }
      if (
        completedSummary.scanMode !== scanMode ||
        completedSummary.scope.kind !==
          (scopeId === null ? "volume" : "folder")
      ) {
        throw new StorageCommandError("REPORT_INCOMPATIBLE");
      }

      setState((latest) => ({
        ...latest,
        scanPhase: "success",
        scanError: null,
        scanProgress: {
          requestId: scanRequestId,
          phase: "complete",
          completed: "1",
          total: "1",
        },
        summary: completedSummary,
      }));
    } catch (error) {
      if (
        mounted.current &&
        scanGeneration.current === requestGeneration
      ) {
        const code = normalizeStorageError(error).code;
        setState((latest) => ({
          ...latest,
          scanPhase: "error",
          scanError: code,
          scanProgress: null,
          summary: null,
        }));
      }
    } finally {
      void unlistenPromise.then((unlisten) => unlisten());
      if (scanGeneration.current === requestGeneration) {
        scanInFlight.current = false;
      }
    }
  }, []);

  const resetScan = useCallback(() => {
    if (scanInFlight.current) {
      return;
    }
    setState((current) => ({
      ...current,
      ...clearScan(),
    }));
  }, []);

  return {
    state,
    refreshInventory,
    selectVolume,
    selectFolder,
    clearFolder,
    selectScanMode,
    startScan,
    resetScan,
  };
}
