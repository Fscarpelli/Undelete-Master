import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  getCandidatePage,
  listStorageSources,
  scanStorageVolume,
  selectScanFolder,
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
});
