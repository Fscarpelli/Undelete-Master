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
      schemaVersion: 3,
      scanId: "scan-1",
      sourceLabel: "OS (C:)",
      scope: { kind: "folder", label: "Documents" },
      scanMode: "metadata",
      fileSystem: "ntfs",
      scanStatus: "partial",
      totalCandidates: "12",
      matchedCandidates: "7",
      unknownCandidates: "2",
      mftCoverage: {
        recordsDeclared: "18446744073709551615",
        recordsAvailable: "9007199254740993",
        recordsExamined: "65536",
        bytesDeclared: "18446744073709551615",
        bytesAvailable: "9223372036854775808",
        bytesExamined: "67108864",
      },
      jpegCarveCoverage: null,
      warnings: ["The active volume changed while it was being read."],
    });

    expect(parsed.scope.kind).toBe("folder");
    expect(parsed.matchedCandidates).toBe("7");
    expect(parsed.unknownCandidates).toBe("2");
    expect(parsed.mftCoverage?.recordsExamined).toBe("65536");
    expect(parsed.mftCoverage?.bytesAvailable).toBe("9223372036854775808");
  });

  it("parses bounded JPEG coverage only for a deep whole-volume NTFS scan", () => {
    const deepSummary = {
      schemaVersion: 3,
      scanId: "scan-deep-1",
      sourceLabel: "OS (C:)",
      scope: { kind: "volume", label: "OS (C:)" },
      scanMode: "deepJpeg",
      fileSystem: "ntfs",
      scanStatus: "partial",
      totalCandidates: "3",
      matchedCandidates: "3",
      unknownCandidates: "0",
      mftCoverage: null,
      jpegCarveCoverage: {
        bytesRequested: "9223372036854775808",
        bytesScanned: "67108864",
        signaturesAttempted: "10000000",
        validationBytesRead: "8388608",
        partial: true,
        readErrorCount: "1",
        candidateLimitReached: false,
        candidateByteLimitHits: "2",
        signatureAttemptLimitReached: true,
        validationByteLimitReached: false,
        rejectedSignatures: "7",
        truncatedSignatures: "1",
        regionsSubmitted: "4",
        regionLimitReached: false,
      },
      warnings: ["The deep JPEG scan reached a safe bound."],
    };

    const parsed = parseScanSummary(deepSummary);

    expect(parsed.scanMode).toBe("deepJpeg");
    expect(parsed.jpegCarveCoverage?.bytesScanned).toBe("67108864");
    expect(parsed.jpegCarveCoverage?.signaturesAttempted).toBe("10000000");
    expect(parsed.jpegCarveCoverage?.validationBytesRead).toBe("8388608");
    expect(parsed.jpegCarveCoverage?.signatureAttemptLimitReached).toBe(true);
    expect(parsed.jpegCarveCoverage?.partial).toBe(true);

    expect(() =>
      parseScanSummary({
        ...deepSummary,
        scope: { kind: "folder", label: "Documents" },
      }),
    ).toThrow(StorageContractError);
    expect(() =>
      parseScanSummary({
        ...deepSummary,
        scanMode: "metadata",
      }),
    ).toThrow(StorageContractError);
    expect(() =>
      parseScanSummary({
        ...deepSummary,
        scanStatus: "complete",
      }),
    ).toThrow(StorageContractError);
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
      schemaVersion: 2,
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
          method: "ntfsMetadata",
          contentSha256: null,
          validator: null,
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

  it("parses JPEG carving provenance and rejects malformed content hashes", () => {
    const carved = {
      schemaVersion: 2,
      scanId: "scan-deep-1",
      cursor: null,
      nextCursor: null,
      candidates: [
        {
          id: "candidate-carved-1",
          displayPath: "carved-0000000000100000.jpg",
          kind: "file",
          state: "structurallyValid",
          sizeBytes: "4096",
          metadataConfidence: "low",
          recoverabilityScore: 72,
          pathState: "incomplete",
          method: "carving",
          contentSha256:
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
          validator: "jpeg-structural-v1",
          warnings: [],
        },
      ],
    };

    const parsed = parseCandidatePage(carved).candidates[0]!;
    expect(parsed.method).toBe("carving");
    expect(parsed.validator).toBe("jpeg-structural-v1");

    expect(() =>
      parseCandidatePage({
        ...carved,
        candidates: [
          {
            ...carved.candidates[0]!,
            contentSha256: "not-a-sha256",
          },
        ],
      }),
    ).toThrow(StorageContractError);
    const corroboratedMetadata = parseCandidatePage({
      ...carved,
      candidates: [
        {
          ...carved.candidates[0]!,
          method: "ntfsMetadata",
        },
      ],
    }).candidates[0]!;
    expect(corroboratedMetadata.method).toBe("ntfsMetadata");
    expect(corroboratedMetadata.contentSha256).not.toBeNull();
    expect(() =>
      parseCandidatePage({
        ...carved,
        candidates: [
          {
            ...carved.candidates[0]!,
            contentSha256: null,
            validator: null,
          },
        ],
      }),
    ).toThrow(StorageContractError);
    expect(() =>
      parseCandidatePage({
        ...carved,
        candidates: [
          {
            ...carved.candidates[0]!,
            validator: "jpeg-structural-v2",
          },
        ],
      }),
    ).toThrow(StorageContractError);
    expect(() =>
      parseCandidatePage({
        ...carved,
        candidates: [
          {
            ...carved.candidates[0]!,
            method: "ntfsMetadata",
            validator: null,
          },
        ],
      }),
    ).toThrow(StorageContractError);
  });
});
