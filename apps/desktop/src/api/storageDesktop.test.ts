import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  cancelRestore,
  createRestorePlan,
  getCandidatePage,
  getRestoreJob,
  listStorageSources,
  normalizeRestoreError,
  normalizeStorageError,
  openRestoreDestination,
  queryCandidatePage,
  scanStorageVolume,
  selectRestoreDestination,
  selectScanFolder,
  startRestore,
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

  it("uses only opaque authority IDs for the complete native restore command set", async () => {
    tauri.invoke
      .mockResolvedValueOnce(restoreDestinationResponse())
      .mockResolvedValueOnce(restorePlanResponse())
      .mockResolvedValueOnce(restoreJobResponse("job-contract", "queued"))
      .mockResolvedValueOnce(restoreJobResponse("job-contract", "running"))
      .mockResolvedValueOnce(restoreJobResponse("job-contract", "cancelling"))
      .mockResolvedValueOnce({ schemaVersion: 1, opened: true });

    await selectRestoreDestination("request-destination", "scan-1");
    await createRestorePlan(
      "request-plan",
      "scan-1",
      "7",
      "destination-1",
      "rename",
      "zeroFillAndMap",
    );
    await startRestore("request-start", "plan-1");
    await getRestoreJob("request-poll", "job-contract");
    await cancelRestore("request-cancel", "job-contract");
    await openRestoreDestination("request-open", "job-complete");

    expect(tauri.invoke.mock.calls).toEqual([
      [
        "select_restore_destination",
        { requestId: "request-destination", scanId: "scan-1" },
      ],
      [
        "create_restore_plan",
        {
          requestId: "request-plan",
          scanId: "scan-1",
          selectionRevision: "7",
          destinationId: "destination-1",
          collisionPolicy: "rename",
          partialFilePolicy: "zeroFillAndMap",
        },
      ],
      [
        "start_restore",
        { requestId: "request-start", planId: "plan-1" },
      ],
      [
        "get_restore_job",
        { requestId: "request-poll", jobId: "job-contract" },
      ],
      [
        "cancel_restore",
        { requestId: "request-cancel", jobId: "job-contract" },
      ],
      [
        "open_restore_destination",
        { requestId: "request-open", jobId: "job-complete" },
      ],
    ]);
  });

  it("returns native destination-picker cancellation without fabricating authority", async () => {
    tauri.invoke.mockResolvedValue(null);

    await expect(
      selectRestoreDestination("request-picker-cancel", "scan-1"),
    ).resolves.toBeNull();
    expect(tauri.invoke).toHaveBeenCalledTimes(1);
  });

  it("fails closed on authority leaks or response/request binding changes", async () => {
    tauri.invoke
      .mockResolvedValueOnce({
        ...restoreDestinationResponse(),
        path: "E:\\Recovered",
      })
      .mockResolvedValueOnce({
        ...restorePlanResponse(),
        scanId: "scan-substituted",
      })
      .mockResolvedValueOnce(
        restoreJobResponse("job-binding", "queued", {
          planId: "plan-substituted",
        }),
      )
      .mockResolvedValueOnce(
        restoreJobResponse("job-binding", "running"),
      );

    await expect(
      selectRestoreDestination("request-leak", "scan-1"),
    ).rejects.toMatchObject({ code: "REPORT_INCOMPATIBLE" });
    await expect(
      createRestorePlan(
        "request-plan-binding",
        "scan-1",
        "7",
        "destination-1",
        "rename",
        "zeroFillAndMap",
      ),
    ).rejects.toMatchObject({ code: "REPORT_INCOMPATIBLE" });
    await expect(
      startRestore("request-job-binding", "plan-1"),
    ).rejects.toMatchObject({ code: "REPORT_INCOMPATIBLE" });
    await expect(
      startRestore("request-job-binding-retry", "plan-1"),
    ).resolves.toMatchObject({ jobId: "job-binding", status: "running" });
  });

  it("rejects malformed outbound restore authority before native invocation", async () => {
    const calls = [
      () => selectRestoreDestination("C:\\request", "scan-1"),
      () => selectRestoreDestination("request-1", ".."),
      () =>
        createRestorePlan(
          "request-2",
          "scan-1",
          "07",
          "destination-1",
          "rename",
          "completeOnly",
        ),
      () =>
        createRestorePlan(
          "request-3",
          "scan-1",
          "7",
          "E:\\Recovered",
          "rename",
          "completeOnly",
        ),
      () => startRestore("request-4", "../plan"),
      () => getRestoreJob("request-5", "job/one"),
      () => cancelRestore("request-6", "job\\one"),
      () => openRestoreDestination("request-7", "explorer.exe C:"),
    ];

    for (const call of calls) {
      await expect(call()).rejects.toMatchObject({
        code: "REPORT_INCOMPATIBLE",
      });
    }
    expect(tauri.invoke).not.toHaveBeenCalled();
  });

  it("tracks native snapshots and rejects a job that revives from cancelling", async () => {
    tauri.invoke
      .mockResolvedValueOnce(
        restoreJobResponse("job-transition", "queued"),
      )
      .mockResolvedValueOnce(
        restoreJobResponse("job-transition", "cancelling"),
      )
      .mockResolvedValueOnce(
        restoreJobResponse("job-transition", "running"),
      );

    await startRestore("request-transition-start", "plan-1");
    await getRestoreJob("request-transition-cancelling", "job-transition");
    await expect(
      getRestoreJob("request-transition-revival", "job-transition"),
    ).rejects.toMatchObject({ code: "REPORT_INCOMPATIBLE" });
  });

  it("retains a terminal snapshot long enough to reject job-ID revival", async () => {
    tauri.invoke
      .mockResolvedValueOnce(
        restoreJobResponse("job-terminal", "queued"),
      )
      .mockResolvedValueOnce(completedRestoreJobResponse("job-terminal"))
      .mockResolvedValueOnce(
        restoreJobResponse("job-terminal", "running"),
      );

    await startRestore("request-terminal-start", "plan-1");
    await getRestoreJob("request-terminal-complete", "job-terminal");
    await expect(
      getRestoreJob("request-terminal-revival", "job-terminal"),
    ).rejects.toMatchObject({ code: "REPORT_INCOMPATIBLE" });
  });

  it("preserves structured native restore errors", () => {
    expect(normalizeRestoreError({ code: "RESTORE_JOB_NOT_FOUND" })).toMatchObject(
      {
        code: "RESTORE_JOB_NOT_FOUND",
      },
    );
    expect(
      normalizeRestoreError({ code: "RESTORE_DIFFERENT_DISK_REQUIRED" }),
    ).toMatchObject({ code: "RESTORE_DIFFERENT_DISK_REQUIRED" });
    expect(
      normalizeRestoreError({ code: "RESTORE_DESTINATION_LIMIT" }),
    ).toMatchObject({ code: "RESTORE_DESTINATION_LIMIT" });
    expect(
      normalizeRestoreError({ code: "RESTORE_PLAN_LIMIT" }),
    ).toMatchObject({ code: "RESTORE_PLAN_LIMIT" });
    expect(
      normalizeRestoreError({ code: "RESTORE_JOB_LIMIT" }),
    ).toMatchObject({ code: "RESTORE_JOB_LIMIT" });
  });

  it.each([
    "RESULT_QUERY_INVALID",
    "RESULT_CURSOR_STALE",
    "RESULT_SELECTION_STALE",
    "RESULT_SELECTION_INVALID",
  ] as const)("preserves the closed native result error %s", (code) => {
    expect(normalizeStorageError({ code })).toMatchObject({ code });
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

function restoreDestinationResponse() {
  return {
    schemaVersion: 1,
    destinationId: "destination-1",
    label: "Recovered files",
    volumeLabel: "Backup (E:)",
    fileSystem: "NTFS",
    freeBytes: "9007199254740993",
    relation: "different",
  };
}

function restorePlanResponse() {
  return {
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
}

function restoreJobResponse(
  jobId: string,
  status: "queued" | "running" | "cancelling",
  overrides: Record<string, unknown> = {},
) {
  return {
    schemaVersion: 1,
    jobId,
    planId: "plan-1",
    status,
    itemsTotal: "3",
    itemsCompleted: status === "queued" ? "0" : "1",
    itemsFailed: "0",
    itemsCancelled: "0",
    bytesTotal: "9007199254740993",
    bytesCompleted: status === "queued" ? "0" : "42",
    currentItem:
      status === "queued"
        ? null
        : { ordinal: "1", candidateId: "42", kind: "file" },
    warnings: [],
    manifest: null,
    ...overrides,
  };
}

function completedRestoreJobResponse(jobId: string) {
  return {
    schemaVersion: 1,
    jobId,
    planId: "plan-1",
    status: "completed",
    itemsTotal: "3",
    itemsCompleted: "3",
    itemsFailed: "0",
    itemsCancelled: "0",
    bytesTotal: "9007199254740993",
    bytesCompleted: "9007199254740993",
    currentItem: null,
    warnings: [],
    manifest: {
      manifestSha256:
        "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
      completionStatus: "completedDurable",
      publishedItems: "3",
      partialItems: "1",
    },
  };
}
