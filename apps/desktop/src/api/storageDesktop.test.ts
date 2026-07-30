import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  getCandidatePage,
  listStorageSources,
  queryCandidatePage,
  scanStorageVolume,
  selectScanFolder,
  updateCandidateSelection,
} from "./storageDesktop";

const tauri = vi.hoisted(() => ({
  isTauri: vi.fn<() => boolean>(),
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  isTauri: tauri.isTauri,
  invoke: tauri.invoke,
}));

describe("native connected-storage commands", () => {
  beforeEach(() => {
    tauri.isTauri.mockReturnValue(true);
    tauri.invoke.mockReset();
  });

  it("invokes real inventory without a path argument", async () => {
    tauri.invoke.mockResolvedValue({
      schemaVersion: 1,
      generation: "inventory-1",
      disks: [],
    });

    await listStorageSources("request-1");

    expect(tauri.invoke).toHaveBeenCalledWith("list_storage_sources", {
      requestId: "request-1",
    });
  });

  it("passes only opaque volume/scope IDs to folder selection and scanning", async () => {
    tauri.invoke
      .mockResolvedValueOnce({
        schemaVersion: 1,
        scopeId: "scope-1",
        volumeId: "volume-1",
        label: "Documents",
      })
      .mockResolvedValueOnce({
        schemaVersion: 3,
        scanId: "scan-1",
        sourceLabel: "OS (C:)",
        scope: { kind: "folder", label: "Documents" },
        scanMode: "metadata",
        fileSystem: "ntfs",
        scanStatus: "complete",
        totalCandidates: "1",
        matchedCandidates: "1",
        unknownCandidates: "0",
        mftCoverage: {
          recordsDeclared: "64",
          recordsAvailable: "64",
          recordsExamined: "64",
          bytesDeclared: "65536",
          bytesAvailable: "65536",
          bytesExamined: "65536",
        },
        jpegCarveCoverage: null,
        warnings: [],
      });

    await selectScanFolder("request-2", "inventory-1", "volume-1");
    await scanStorageVolume(
      "request-3",
      "inventory-1",
      "volume-1",
      "scope-1",
      "metadata",
    );

    expect(tauri.invoke.mock.calls).toEqual([
      [
        "select_scan_folder",
        {
          requestId: "request-2",
          generation: "inventory-1",
          volumeId: "volume-1",
        },
      ],
      [
        "scan_storage_volume",
        {
          requestId: "request-3",
          generation: "inventory-1",
          volumeId: "volume-1",
          scopeId: "scope-1",
          mode: "metadata",
        },
      ],
    ]);
  });

  it("fails closed outside Tauri without fabricating inventory", async () => {
    tauri.isTauri.mockReturnValue(false);

    await expect(listStorageSources("request-4")).rejects.toMatchObject({
      code: "DESKTOP_RUNTIME_UNAVAILABLE",
    });
    expect(tauri.invoke).not.toHaveBeenCalled();
  });

  it("requests bounded candidate pages of at most 100 rows", async () => {
    tauri.invoke.mockResolvedValue({
      schemaVersion: 2,
      scanId: "scan-1",
      cursor: null,
      nextCursor: null,
      candidates: [],
    });

    await getCandidatePage("request-5", "scan-1", null);

    expect(tauri.invoke).toHaveBeenCalledWith("get_candidate_page", {
      requestId: "request-5",
      scanId: "scan-1",
      cursor: null,
      limit: 100,
    });
  });

  it("sends only the closed query and selection command arguments", async () => {
    tauri.invoke
      .mockResolvedValueOnce(queryPageResponse())
      .mockResolvedValueOnce({
        schemaVersion: 1,
        scanId: "scan-1",
        queryId: "query-1",
        selectionRevision: "1",
        selection: {
          selectionRevision: "1",
          selectedCandidates: "0",
          selectedFiles: "0",
          selectedDirectories: "0",
          selectedLogicalBytes: "0",
          bestEffortCandidates: "0",
          conflictedCandidates: "0",
          ineligibleCandidates: "0",
          matchingSelectedCandidates: "0",
        },
      });
    const query = {
      revision: "7",
      search: "",
      extensions: [],
      kinds: [],
      metadataConfidences: [],
      methods: [],
      states: [],
      minRecoverabilityScore: null,
      maxRecoverabilityScore: null,
      eligibilities: [],
      selectedOnly: false,
    } as const;
    const sort = { field: "path", direction: "ascending" } as const;

    await queryCandidatePage("request-6", "scan-1", query, sort, null);
    await updateCandidateSelection(
      "request-7",
      "scan-1",
      "query-1",
      { type: "clearAll" },
      "0",
    );

    expect(tauri.invoke.mock.calls).toEqual([
      [
        "query_candidate_page",
        { requestId: "request-6", scanId: "scan-1", query, sort, cursor: null },
      ],
      [
        "update_candidate_selection",
        {
          requestId: "request-7",
          scanId: "scan-1",
          queryId: "query-1",
          operation: { type: "clearAll" },
          selectionRevision: "0",
        },
      ],
    ]);
  });
});

function queryPageResponse() {
  return {
    schemaVersion: 1,
    scanId: "scan-1",
    queryId: "query-1",
    queryRevision: "7",
    cursor: null,
    nextCursor: null,
    filteredTotal: "0",
    extensionFacets: [],
    selection: {
      selectionRevision: "0",
      selectedCandidates: "0",
      selectedFiles: "0",
      selectedDirectories: "0",
      selectedLogicalBytes: "0",
      bestEffortCandidates: "0",
      conflictedCandidates: "0",
      ineligibleCandidates: "0",
      matchingSelectedCandidates: "0",
    },
    candidates: [],
  };
}
