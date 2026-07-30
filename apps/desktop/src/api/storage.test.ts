import { describe, expect, it } from "vitest";
import {
  StorageContractError,
  parseCandidatePage,
  parseFolderSelection,
  parseScanSummary,
  parseStorageInventory,
} from "./storage";

const inventory = {
  schemaVersion: 1,
  generation: "inventory-7",
  disks: [
    {
      id: "disk-a1",
      displayName: "Internal NVMe",
      busType: "nvme",
      sizeBytes: "2048000000000",
      volumes: [
        {
          id: "volume-c1",
          mountLabel: "C:",
          label: "OS",
          fileSystem: "ntfs",
          sizeBytes: "2027000000000",
          freeBytes: "700000000000",
          isSystem: true,
          scanSupported: true,
          folderScopeSupported: true,
          warnings: [],
        },
      ],
    },
  ],
};

describe("real storage contracts", () => {
  it("parses a bounded connected-disk inventory without native paths", () => {
    const parsed = parseStorageInventory(inventory);

    expect(parsed.disks[0]!.volumes[0]!.mountLabel).toBe("C:");
    expect(parsed.disks[0]!.volumes[0]!.sizeBytes).toBe("2027000000000");
    expect(parsed.disks[0]).not.toHaveProperty("number");
  });

  it("rejects path-bearing, numbered, or duplicate inventory authority", () => {
    expect(() =>
      parseStorageInventory({
        ...inventory,
        sourcePath: ["\\\\.", "\\", "Physical", "Drive0"].join(""),
      }),
    ).toThrow(StorageContractError);

    expect(() =>
      parseStorageInventory({
        ...inventory,
        disks: [{ ...inventory.disks[0], number: 0 }],
      }),
    ).toThrow(StorageContractError);

    expect(() =>
      parseStorageInventory({
        ...inventory,
        disks: [inventory.disks[0], inventory.disks[0]],
      }),
    ).toThrow(StorageContractError);
  });

  it("parses a truthful folder-scoped summary with unknown ancestry separate", () => {
    const parsed = parseScanSummary({
      schemaVersion: 1,
      scanId: "scan-1",
      sourceLabel: "OS (C:)",
      scope: { kind: "folder", label: "Documents" },
      fileSystem: "ntfs",
      scanStatus: "partial",
      totalCandidates: "12",
      matchedCandidates: "7",
      unknownCandidates: "2",
      warnings: ["The active volume changed while it was being read."],
    });

    expect(parsed.scope.kind).toBe("folder");
    expect(parsed.matchedCandidates).toBe("7");
    expect(parsed.unknownCandidates).toBe("2");
  });

  it("parses an opaque native folder selection without exposing its path", () => {
    const parsed = parseFolderSelection({
      schemaVersion: 1,
      scopeId: "scope-3",
      volumeId: "volume-c1",
      label: "Documents",
    });

    expect(parsed.label).toBe("Documents");
    expect(() =>
      parseFolderSelection({
        ...parsed,
        folderPath: "C:\\Users\\private\\Documents",
      }),
    ).toThrow(StorageContractError);
  });

  it("parses real paginated candidates and rejects a leaked native path", () => {
    const page = {
      schemaVersion: 1,
      scanId: "scan-1",
      cursor: null,
      nextCursor: "cursor-2",
      candidates: [
        {
          id: "candidate-42",
          displayPath: "Documents/deleted.txt",
          kind: "file",
          state: "likelyComplete",
          sizeBytes: "18446744073709551615",
          metadataConfidence: "high",
          recoverabilityScore: 88,
          pathState: "exact",
          warnings: [],
        },
      ],
    };

    expect(
      parseCandidatePage(page).candidates[0]!.recoverabilityScore,
    ).toBe(88);
    expect(
      parseCandidatePage({
        ...page,
        candidates: [
          {
            ...page.candidates[0]!,
            id: "candidate-directory",
            kind: "directory",
            recoverabilityScore: null,
          },
        ],
      }).candidates[0]!.recoverabilityScore,
    ).toBeNull();
    expect(() =>
      parseCandidatePage({
        ...page,
        candidates: [
          {
            ...page.candidates[0]!,
            sourcePath: "C:\\Documents\\deleted.txt",
          },
        ],
      }),
    ).toThrow(StorageContractError);
  });
});
