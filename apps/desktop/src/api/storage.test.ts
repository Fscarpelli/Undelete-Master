import { describe, expect, it } from "vitest";
import {
  MAX_RETAINED_CANDIDATES_PER_SCAN,
  StorageContractError,
  parseCandidateQueryPage,
  parseCandidatePage,
  parseFolderSelection,
  parseOpenRestoreDestinationResponse,
  parseRestoreDestinationSummary,
  parseRestoreJobSnapshot,
  parseRestorePlanSummary,
  parseScanSummary,
  parseStorageInventory,
} from "./storage";

const queryPage = {
  schemaVersion: 1,
  scanId: "scan-1",
  queryId: "query-1",
  queryRevision: "7",
  cursor: null,
  nextCursor: "cursor-2",
  filteredTotal: "1",
  extensionFacets: [{ extension: "txt", count: "1" }],
  selection: {
    selectionRevision: "3",
    selectedCandidates: "1",
    selectedFiles: "1",
    selectedDirectories: "0",
    selectedLogicalBytes: "42",
    bestEffortCandidates: "0",
    conflictedCandidates: "0",
    ineligibleCandidates: "0",
    matchingSelectedCandidates: "1",
  },
  candidates: [
    {
      id: "42",
      displayPath: "Documents/deleted.txt",
      extension: "txt",
      kind: "file",
      state: "likelyComplete",
      sizeBytes: "42",
      metadataConfidence: "high",
      recoverabilityScore: 88,
      pathState: "exact",
      method: "ntfsMetadata",
      eligibility: "complete",
      selected: true,
      warnings: [],
    },
  ],
};

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

const restoreDestination = {
  schemaVersion: 1,
  destinationId: "destination-1",
  label: "Recovered files",
  volumeLabel: "Backup (E:)",
  fileSystem: "NTFS",
  freeBytes: "9007199254740993",
  relation: "different",
};

const restorePlan = {
  schemaVersion: 1,
  planId: "plan-1",
  planDigest:
    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
  scanId: "scan-1",
  destinationId: "destination-1",
  selectionRevision: "7",
  collisionPolicy: "rename",
  partialFilePolicy: "zeroFillAndMap",
  itemsTotal: "3",
  filesTotal: "2",
  directoriesTotal: "1",
  logicalBytes: "9007199254740993",
  bestEffortItems: "1",
};

const queuedRestoreJob = {
  schemaVersion: 1,
  jobId: "job-1",
  planId: "plan-1",
  status: "queued",
  itemsTotal: "3",
  itemsCompleted: "0",
  itemsFailed: "0",
  itemsCancelled: "0",
  bytesTotal: "9007199254740993",
  bytesCompleted: "0",
  currentItem: null,
  warnings: [],
  manifest: null,
};

const runningRestoreJob = {
  ...queuedRestoreJob,
  status: "running",
  itemsCompleted: "1",
  bytesCompleted: "42",
  currentItem: {
    ordinal: "1",
    candidateId: "42",
    kind: "file",
  },
};

const completedRestoreJob = {
  ...runningRestoreJob,
  status: "completed",
  itemsCompleted: "3",
  bytesCompleted: "9007199254740993",
  currentItem: null,
  manifest: {
    manifestSha256:
      "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
    completionStatus: "completedDurable",
    publishedItems: "3",
  },
};

describe("real storage contracts", () => {
  it("parses exact opaque restore destination, plan, job, and open responses", () => {
    expect(parseRestoreDestinationSummary(restoreDestination)).toEqual(
      restoreDestination,
    );
    expect(parseRestorePlanSummary(restorePlan)).toEqual(restorePlan);
    expect(parseRestoreJobSnapshot(runningRestoreJob)).toEqual(
      runningRestoreJob,
    );
    expect(
      parseOpenRestoreDestinationResponse({
        schemaVersion: 1,
        opened: true,
      }),
    ).toEqual({ schemaVersion: 1, opened: true });
  });

  it("rejects restore authority fields recursively instead of leaking paths or bytes", () => {
    expect(() =>
      parseRestoreDestinationSummary({
        ...restoreDestination,
        path: "E:\\Recovered",
      }),
    ).toThrow(StorageContractError);
    expect(() =>
      parseRestoreDestinationSummary({
        ...restoreDestination,
        diskNumber: 1,
      }),
    ).toThrow(StorageContractError);
    expect(() =>
      parseRestorePlanSummary({
        ...restorePlan,
        extents: [{ offset: "4096", length: "512" }],
      }),
    ).toThrow(StorageContractError);
    expect(() =>
      parseRestoreJobSnapshot({
        ...runningRestoreJob,
        currentItem: {
          ...runningRestoreJob.currentItem,
          handle: "native-handle",
        },
      }),
    ).toThrow(StorageContractError);
    expect(() =>
      parseRestoreJobSnapshot({
        ...completedRestoreJob,
        manifest: {
          ...completedRestoreJob.manifest,
          recoveredBytes: "42",
        },
      }),
    ).toThrow(StorageContractError);
    expect(() =>
      parseOpenRestoreDestinationResponse({
        schemaVersion: 1,
        opened: true,
        executable: "explorer.exe",
      }),
    ).toThrow(StorageContractError);
  });

  it("accepts only a bounded, different NTFS destination summary", () => {
    for (const invalid of [
      { ...restoreDestination, relation: "same" },
      { ...restoreDestination, fileSystem: "ntfs" },
      { ...restoreDestination, freeBytes: 42 },
      { ...restoreDestination, freeBytes: "01" },
      { ...restoreDestination, destinationId: "E:\\Recovered" },
      { ...restoreDestination, label: "E:\\Recovered" },
      { ...restoreDestination, label: "x".repeat(513) },
    ]) {
      expect(() => parseRestoreDestinationSummary(invalid)).toThrow(
        StorageContractError,
      );
    }
  });

  it("requires an internally consistent bounded immutable restore plan", () => {
    for (const invalid of [
      { ...restorePlan, schemaVersion: 2 },
      { ...restorePlan, selectionRevision: "07" },
      { ...restorePlan, collisionPolicy: "overwrite" },
      { ...restorePlan, partialFilePolicy: "skip" },
      { ...restorePlan, partialFilePolicy: "completeOnly" },
      { ...restorePlan, itemsTotal: "4" },
      { ...restorePlan, bestEffortItems: "3" },
      {
        ...restorePlan,
        itemsTotal: String(MAX_RETAINED_CANDIDATES_PER_SCAN + 1),
        filesTotal: String(MAX_RETAINED_CANDIDATES_PER_SCAN),
      },
      { ...restorePlan, planDigest: "ABCDEF".repeat(10) + "ABCD" },
      { ...restorePlan, planId: "../plan" },
    ]) {
      expect(() => parseRestorePlanSummary(invalid)).toThrow(
        StorageContractError,
      );
    }
  });

  it("rejects incoherent job counters, unbounded warnings, and nonterminal manifests", () => {
    for (const invalid of [
      { ...runningRestoreJob, itemsCompleted: "4" },
      { ...runningRestoreJob, bytesCompleted: "9007199254740994" },
      { ...runningRestoreJob, itemsFailed: 1 },
      { ...runningRestoreJob, itemsCancelled: "01" },
      { ...runningRestoreJob, warnings: Array(129).fill("warning") },
      {
        ...runningRestoreJob,
        warnings: ["Restore failed at E:\\Recovered\\deleted.txt"],
      },
      {
        ...runningRestoreJob,
        manifest: completedRestoreJob.manifest,
      },
      {
        ...runningRestoreJob,
        currentItem: {
          ordinal: "3",
          candidateId: "42",
          kind: "file",
        },
      },
    ]) {
      expect(() => parseRestoreJobSnapshot(invalid)).toThrow(
        StorageContractError,
      );
    }
  });

  it("enforces legal restore status transitions and immutable job bindings", () => {
    const queued = parseRestoreJobSnapshot(queuedRestoreJob);
    expect(parseRestoreJobSnapshot(completedRestoreJob, queued).status).toBe(
      "completed",
    );

    const cancelling = parseRestoreJobSnapshot({
      ...runningRestoreJob,
      status: "cancelling",
    });
    const completed = parseRestoreJobSnapshot(completedRestoreJob);
    expect(() =>
      parseRestoreJobSnapshot(runningRestoreJob, cancelling),
    ).toThrow(StorageContractError);
    expect(() =>
      parseRestoreJobSnapshot(runningRestoreJob, completed),
    ).toThrow(StorageContractError);
    expect(() =>
      parseRestoreJobSnapshot(
        {
          ...completedRestoreJob,
          manifest: {
            ...completedRestoreJob.manifest,
            manifestSha256: "0".repeat(64),
          },
        },
        completed,
      ),
    ).toThrow(StorageContractError);
    expect(() =>
      parseRestoreJobSnapshot(
        { ...runningRestoreJob, planId: "plan-substituted" },
        queued,
      ),
    ).toThrow(StorageContractError);
    expect(() =>
      parseRestoreJobSnapshot(
        { ...runningRestoreJob, itemsTotal: "4" },
        queued,
      ),
    ).toThrow(StorageContractError);
  });

  it("parses one exact schema-v1 actionable query page", () => {
    expect(parseCandidateQueryPage(queryPage)).toEqual(queryPage);
  });

  it("rejects unbounded, duplicate, inconsistent, or authority-leaking query pages", () => {
    expect(() =>
      parseCandidateQueryPage({
        ...queryPage,
        candidates: Array.from({ length: 101 }, (_, index) => ({
          ...queryPage.candidates[0],
          id: String(index + 1),
        })),
      }),
    ).toThrow(StorageContractError);
    expect(() =>
      parseCandidateQueryPage({
        ...queryPage,
        candidates: [queryPage.candidates[0], queryPage.candidates[0]],
      }),
    ).toThrow(StorageContractError);
    for (const badDecimal of [
      "-1",
      "01",
      "18446744073709551616",
      "9007199254740993.0",
    ]) {
      expect(() =>
        parseCandidateQueryPage({
          ...queryPage,
          filteredTotal: badDecimal,
        }),
      ).toThrow(StorageContractError);
    }
    expect(() =>
      parseCandidateQueryPage({
        ...queryPage,
        filteredTotal: "0",
      }),
    ).toThrow(StorageContractError);

    for (const [field, value] of [
      ["sourcePath", "C:\\private"],
      ["destinationPath", "D:\\restore"],
      ["extents", [{ offset: "0", length: "42" }]],
      ["offset", "0"],
      ["handle", "123"],
      ["recoveredBytes", "42"],
    ] as const) {
      expect(() =>
        parseCandidateQueryPage({
          ...queryPage,
          candidates: [{ ...queryPage.candidates[0], [field]: value }],
        }),
      ).toThrow(StorageContractError);
    }
    expect(() =>
      parseCandidateQueryPage({ ...queryPage, unknown: true }),
    ).toThrow(StorageContractError);
  });

  it("accepts every extension facet within the retained-scan bound", () => {
    const extensionFacets = Array.from({ length: 129 }, (_, index) => ({
      extension: `ext${index}`,
      count: "1",
    }));

    expect(
      parseCandidateQueryPage({ ...queryPage, extensionFacets })
        .extensionFacets,
    ).toHaveLength(129);
  });

  it("binds facet parsing to the complete deep-scan candidate ceiling", () => {
    expect(MAX_RETAINED_CANDIDATES_PER_SCAN).toBe(110_000);

    const beyondBound: unknown[] = [];
    beyondBound.length = MAX_RETAINED_CANDIDATES_PER_SCAN + 1;
    expect(() =>
      parseCandidateQueryPage({
        ...queryPage,
        extensionFacets: beyondBound,
      }),
    ).toThrow(StorageContractError);
  });

  it("rejects non-canonical extension facets from hostile native names", () => {
    for (const extension of ["TXT", "txt ", "t/xt", "t\\xt", ".txt"]) {
      expect(() =>
        parseCandidateQueryPage({
          ...queryPage,
          extensionFacets: [{ extension, count: "1" }],
        }),
      ).toThrow(StorageContractError);
    }
  });

  it("round-trips bounded lowercase expansion and rejects overflow or BOM", () => {
    const safe = "\u0130".repeat(127).toLowerCase();
    const overflowing = "\u0130".repeat(128).toLowerCase();
    expect([...safe]).toHaveLength(254);
    expect([...overflowing]).toHaveLength(256);

    const parsed = parseCandidateQueryPage({
      ...queryPage,
      extensionFacets: [{ extension: safe, count: "1" }],
      candidates: [{ ...queryPage.candidates[0], extension: safe }],
    });
    expect(parsed.extensionFacets[0]!.extension).toBe(safe);
    expect(parsed.candidates[0]!.extension).toBe(safe);

    for (const extension of [
      overflowing,
      "\uFEFFtxt",
      "txt\uFEFF",
      "t\uFEFFxt",
    ]) {
      expect(() =>
        parseCandidateQueryPage({
          ...queryPage,
          extensionFacets: [{ extension, count: "1" }],
        }),
      ).toThrow(StorageContractError);
    }
  });

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
