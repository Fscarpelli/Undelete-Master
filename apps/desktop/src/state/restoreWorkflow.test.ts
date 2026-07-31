import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
  CandidateSelectionSummary,
  RestoreDestinationSummary,
  RestoreJobSnapshot,
  RestorePlanSummary,
} from "../api/storage";
import {
  restoreProgressBasisPoints,
  useRestoreWorkflow,
} from "./restoreWorkflow";

const native = vi.hoisted(() => ({
  selectRestoreDestination: vi.fn(),
  createRestorePlan: vi.fn(),
  startRestore: vi.fn(),
  getRestoreJob: vi.fn(),
  cancelRestore: vi.fn(),
  openRestoreDestination: vi.fn(),
}));

vi.mock("../api/storageDesktop", async (importOriginal) => {
  const actual =
    await importOriginal<typeof import("../api/storageDesktop")>();
  return {
    ...actual,
    selectRestoreDestination: native.selectRestoreDestination,
    createRestorePlan: native.createRestorePlan,
    startRestore: native.startRestore,
    getRestoreJob: native.getRestoreJob,
    cancelRestore: native.cancelRestore,
    openRestoreDestination: native.openRestoreDestination,
  };
});

describe("useRestoreWorkflow", () => {
  beforeEach(() => {
    native.selectRestoreDestination.mockReset();
    native.createRestorePlan.mockReset();
    native.startRestore.mockReset();
    native.getRestoreJob.mockReset();
    native.cancelRestore.mockReset();
    native.openRestoreDestination.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("DESKTOP-RESTORE-FLOW-024 returns picker cancellation to results without an error or retained plan", async () => {
    native.selectRestoreDestination.mockResolvedValue(null);
    const { result } = renderHook(() =>
      useRestoreWorkflow(true, "scan-1", selection()),
    );

    act(() => result.current.chooseDestination());
    expect(result.current.state.phase).toBe("selectingDestination");
    await waitFor(() => expect(result.current.state.phase).toBe("idle"));

    expect(result.current.state.destination).toBeNull();
    expect(result.current.state.plan).toBeNull();
    expect(result.current.state.error).toBeNull();
  });

  it("DESKTOP-RESTORE-FLOW-024 requires zero-fill policy and explicit consent for best effort", async () => {
    native.selectRestoreDestination.mockResolvedValue(destination());
    native.createRestorePlan.mockResolvedValue(
      plan({ partialFilePolicy: "zeroFillAndMap", bestEffortItems: "1" }),
    );
    const { result } = renderHook(() =>
      useRestoreWorkflow(
        true,
        "scan-1",
        selection({ selected: "1", bestEffort: "1" }),
      ),
    );

    act(() => result.current.chooseDestination());
    await waitFor(() => expect(result.current.state.phase).toBe("setup"));

    act(() => result.current.createPlan());
    expect(native.createRestorePlan).not.toHaveBeenCalled();
    expect(result.current.state.error).toBe(
      "RESTORE_PARTIAL_POLICY_REQUIRED",
    );
    expect(result.current.state.phase).toBe("error");

    act(() => {
      result.current.setPartialFilePolicy("zeroFillAndMap");
      result.current.createPlan();
    });
    expect(native.createRestorePlan).not.toHaveBeenCalled();

    act(() => result.current.setBestEffortConsent(true));
    act(() => result.current.createPlan());
    await waitFor(() => expect(result.current.state.phase).toBe("review"));

    expect(native.createRestorePlan).toHaveBeenCalledWith(
      expect.stringMatching(/^restore-/),
      "scan-1",
      "7",
      "destination-1",
      "rename",
      "zeroFillAndMap",
    );
  });

  it("DESKTOP-RESTORE-FLOW-024 blocks ineligible selection before a native plan call", async () => {
    native.selectRestoreDestination.mockResolvedValue(destination());
    const { result } = renderHook(() =>
      useRestoreWorkflow(
        true,
        "scan-1",
        selection({ selected: "1", ineligible: "1" }),
      ),
    );

    act(() => result.current.chooseDestination());
    await waitFor(() => expect(result.current.state.phase).toBe("setup"));
    act(() => result.current.createPlan());

    expect(native.createRestorePlan).not.toHaveBeenCalled();
    expect(result.current.state.error).toBe("RESTORE_ITEM_INELIGIBLE");
    expect(result.current.state.phase).toBe("error");
  });

  it("does not reopen the destination picker from planning, review, or finished phases", async () => {
    native.selectRestoreDestination.mockResolvedValue(destination());
    native.createRestorePlan.mockResolvedValue(plan());
    native.startRestore.mockResolvedValue(job("completed"));
    const { result } = renderHook(() =>
      useRestoreWorkflow(
        true,
        "scan-1",
        selection({ revision: "7", selected: "1" }),
      ),
    );

    act(() => result.current.chooseDestination());
    await waitFor(() => expect(result.current.state.phase).toBe("setup"));
    act(() => result.current.createPlan());
    expect(result.current.state.phase).toBe("planning");
    act(() => result.current.chooseDestination());
    expect(native.selectRestoreDestination).toHaveBeenCalledTimes(1);

    await waitFor(() => expect(result.current.state.phase).toBe("review"));
    act(() => result.current.chooseDestination());
    expect(native.selectRestoreDestination).toHaveBeenCalledTimes(1);

    act(() => result.current.start());
    await waitFor(() => expect(result.current.state.phase).toBe("finished"));
    act(() => result.current.chooseDestination());
    expect(native.selectRestoreDestination).toHaveBeenCalledTimes(1);
  });

  it("invalidates plan review when the authoritative selection revision changes before start", async () => {
    native.selectRestoreDestination.mockResolvedValue(destination());
    native.createRestorePlan.mockResolvedValue(plan());
    const { result, rerender } = renderHook(
      ({ currentSelection }) =>
        useRestoreWorkflow(true, "scan-1", currentSelection),
      {
        initialProps: {
          currentSelection: selection({ revision: "7", selected: "1" }),
        },
      },
    );

    act(() => result.current.chooseDestination());
    await waitFor(() => expect(result.current.state.phase).toBe("setup"));
    act(() => result.current.createPlan());
    await waitFor(() => expect(result.current.state.phase).toBe("review"));

    rerender({
      currentSelection: selection({ revision: "8", selected: "1" }),
    });

    await waitFor(() => expect(result.current.state.phase).toBe("setup"));
    expect(result.current.state.plan).toBeNull();
    expect(result.current.state.destination?.destinationId).toBe(
      "destination-1",
    );
    act(() => result.current.start());
    expect(native.startRestore).not.toHaveBeenCalled();
  });

  it("uses native snapshots and BigInt basis points, then stops polling at terminal state", async () => {
    vi.useFakeTimers();
    const runningPoll = deferred<RestoreJobSnapshot>();
    native.selectRestoreDestination.mockResolvedValue(destination());
    native.createRestorePlan.mockResolvedValue(plan());
    native.startRestore.mockResolvedValue(job("queued"));
    native.getRestoreJob
      .mockReturnValueOnce(runningPoll.promise)
      .mockResolvedValueOnce(job("completed"));

    const { result, unmount } = renderHook(() =>
      useRestoreWorkflow(
        true,
        "scan-1",
        selection({ revision: "7", selected: "1" }),
      ),
    );
    await reachActiveRestore(result);

    expect(result.current.state.job?.status).toBe("queued");
    expect(result.current.state.progressBasisPoints).toBe(0);

    await act(async () => {
      vi.advanceTimersByTime(1_000);
      await Promise.resolve();
    });
    expect(native.getRestoreJob).toHaveBeenCalledTimes(1);
    expect(result.current.state.job?.status).toBe("queued");

    await act(async () => {
      runningPoll.resolve(
        job("running", {
          itemsCompleted: "1",
          bytesCompleted: "4503599627370496",
          currentItem: { ordinal: "1", candidateId: "2", kind: "file" },
        }),
      );
      await runningPoll.promise;
    });

    expect(result.current.state.job?.bytesCompleted).toBe("4503599627370496");
    expect(result.current.state.progressBasisPoints).toBe(4999);

    await act(async () => {
      vi.advanceTimersByTime(1_000);
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.state.phase).toBe("finished");
    expect(result.current.state.job?.manifest?.manifestSha256).toBe(
      "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
    );
    expect(result.current.state.progressBasisPoints).toBe(10_000);

    await act(async () => {
      vi.advanceTimersByTime(10_000);
      await Promise.resolve();
    });
    expect(native.getRestoreJob).toHaveBeenCalledTimes(2);
    unmount();
  });

  it("serializes poll and cancel, ignores the superseded poll, and invokes cancel once", async () => {
    vi.useFakeTimers();
    const latePoll = deferred<RestoreJobSnapshot>();
    const cancelResponse = deferred<RestoreJobSnapshot>();
    native.selectRestoreDestination.mockResolvedValue(destination());
    native.createRestorePlan.mockResolvedValue(plan());
    native.startRestore.mockResolvedValue(job("queued"));
    native.getRestoreJob
      .mockReturnValueOnce(latePoll.promise)
      .mockResolvedValueOnce(
        job("cancelled", {
          itemsCancelled: "3",
        }),
      );
    native.cancelRestore.mockReturnValue(cancelResponse.promise);

    const { result, unmount } = renderHook(() =>
      useRestoreWorkflow(
        true,
        "scan-1",
        selection({ revision: "7", selected: "1" }),
      ),
    );
    await reachActiveRestore(result);

    await act(async () => {
      vi.advanceTimersByTime(1_000);
      await Promise.resolve();
    });
    expect(native.getRestoreJob).toHaveBeenCalledTimes(1);

    act(() => {
      result.current.cancel();
      result.current.cancel();
    });
    expect(result.current.state.operationPhase).toBe("cancelRequest");
    expect(result.current.state.cancelRequested).toBe(false);
    expect(native.cancelRestore).not.toHaveBeenCalled();

    await act(async () => {
      latePoll.resolve(
        job("running", {
          itemsCompleted: "1",
          bytesCompleted: "42",
          currentItem: { ordinal: "1", candidateId: "2", kind: "file" },
        }),
      );
      await latePoll.promise;
    });
    await flushUntil(() => native.cancelRestore.mock.calls.length === 1);
    expect(result.current.state.job?.status).toBe("queued");
    expect(result.current.state.cancelRequested).toBe(false);

    await act(async () => {
      cancelResponse.resolve(job("cancelling"));
      await cancelResponse.promise;
    });
    expect(result.current.state.job?.status).toBe("cancelling");
    expect(result.current.state.cancelRequested).toBe(true);
    expect(native.cancelRestore).toHaveBeenCalledTimes(1);

    await act(async () => {
      vi.advanceTimersByTime(1_000);
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(native.getRestoreJob).toHaveBeenCalledTimes(2);
    expect(result.current.state.job?.status).toBe("cancelled");
    expect(result.current.state.phase).toBe("finished");
    unmount();
  });

  it("preserves and continues polling a non-terminal native job after one transient internal poll error", async () => {
    vi.useFakeTimers();
    native.selectRestoreDestination.mockResolvedValue(destination());
    native.createRestorePlan.mockResolvedValue(plan());
    native.startRestore.mockResolvedValue(job("running"));
    native.getRestoreJob
      .mockRejectedValueOnce({ code: "RESTORE_INTERNAL" })
      .mockResolvedValueOnce(job("completed"));

    const { result, unmount } = renderHook(() =>
      useRestoreWorkflow(
        true,
        "scan-1",
        selection({ revision: "7", selected: "1" }),
      ),
    );
    await reachActiveRestore(result);

    await act(async () => {
      vi.advanceTimersByTime(1_000);
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.state.phase).toBe("active");
    expect(result.current.state.job?.status).toBe("running");
    expect(result.current.state.error).toBe("RESTORE_INTERNAL");

    act(() => result.current.close());
    expect(result.current.state.phase).toBe("active");
    expect(result.current.state.job?.jobId).toBe("job-1");

    await act(async () => {
      vi.advanceTimersByTime(1_000);
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(native.getRestoreJob).toHaveBeenCalledTimes(2);
    expect(result.current.state.phase).toBe("finished");
    unmount();
  });

  it.each([
    "RESTORE_JOB_NOT_FOUND",
    "REPORT_INCOMPATIBLE",
  ] as const)(
    "stops polling and exposes closeable tracking loss after permanent %s",
    async (code) => {
      vi.useFakeTimers();
      native.selectRestoreDestination.mockResolvedValue(destination());
      native.createRestorePlan.mockResolvedValue(plan());
      native.startRestore.mockResolvedValue(job("running"));
      native.getRestoreJob.mockRejectedValueOnce({ code });

      const { result, unmount } = renderHook(() =>
        useRestoreWorkflow(
          true,
          "scan-1",
          selection({ revision: "7", selected: "1" }),
        ),
      );
      await reachActiveRestore(result);

      await act(async () => {
        vi.advanceTimersByTime(1_000);
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(result.current.state.phase).toBe("trackingLost");
      expect(result.current.state.job?.status).toBe("running");
      expect(result.current.state.error).toBe(code);

      await act(async () => {
        vi.advanceTimersByTime(10_000);
        await Promise.resolve();
      });
      expect(native.getRestoreJob).toHaveBeenCalledTimes(1);

      act(() => result.current.close());
      expect(result.current.state.phase).toBe("idle");
      expect(result.current.state.job).toBeNull();
      unmount();
    },
  );

  it("bounds consecutive internal poll retries before entering unknown-outcome tracking loss", async () => {
    vi.useFakeTimers();
    native.selectRestoreDestination.mockResolvedValue(destination());
    native.createRestorePlan.mockResolvedValue(plan());
    native.startRestore.mockResolvedValue(job("running"));
    native.getRestoreJob.mockRejectedValue({ code: "RESTORE_INTERNAL" });

    const { result, unmount } = renderHook(() =>
      useRestoreWorkflow(
        true,
        "scan-1",
        selection({ revision: "7", selected: "1" }),
      ),
    );
    await reachActiveRestore(result);

    for (let attempt = 1; attempt <= 3; attempt += 1) {
      await act(async () => {
        vi.advanceTimersByTime(1_000);
        await Promise.resolve();
        await Promise.resolve();
      });
      expect(native.getRestoreJob).toHaveBeenCalledTimes(attempt);
    }

    expect(result.current.state.phase).toBe("trackingLost");
    expect(result.current.state.job?.status).toBe("running");
    expect(result.current.state.error).toBe("RESTORE_INTERNAL");

    await act(async () => {
      vi.advanceTimersByTime(10_000);
      await Promise.resolve();
    });
    expect(native.getRestoreJob).toHaveBeenCalledTimes(3);
    unmount();
  });

  it("resets the internal poll-failure budget after a valid non-terminal snapshot", async () => {
    vi.useFakeTimers();
    native.selectRestoreDestination.mockResolvedValue(destination());
    native.createRestorePlan.mockResolvedValue(plan());
    native.startRestore.mockResolvedValue(job("running"));
    native.getRestoreJob
      .mockRejectedValueOnce({ code: "RESTORE_INTERNAL" })
      .mockResolvedValueOnce(
        job("running", { itemsCompleted: "1", bytesCompleted: "42" }),
      )
      .mockRejectedValue({ code: "RESTORE_INTERNAL" });

    const { result, unmount } = renderHook(() =>
      useRestoreWorkflow(
        true,
        "scan-1",
        selection({ revision: "7", selected: "1" }),
      ),
    );
    await reachActiveRestore(result);

    for (let poll = 1; poll <= 4; poll += 1) {
      await act(async () => {
        vi.advanceTimersByTime(1_000);
        await Promise.resolve();
        await Promise.resolve();
      });
      expect(native.getRestoreJob).toHaveBeenCalledTimes(poll);
    }
    expect(result.current.state.phase).toBe("active");
    expect(result.current.state.job?.itemsCompleted).toBe("1");

    await act(async () => {
      vi.advanceTimersByTime(1_000);
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(native.getRestoreJob).toHaveBeenCalledTimes(5);
    expect(result.current.state.phase).toBe("trackingLost");
    unmount();
  });

  it("cleans up the sole polling timer on unmount", async () => {
    vi.useFakeTimers();
    native.selectRestoreDestination.mockResolvedValue(destination());
    native.createRestorePlan.mockResolvedValue(plan());
    native.startRestore.mockResolvedValue(job("running"));

    const hook = renderHook(() =>
      useRestoreWorkflow(
        true,
        "scan-1",
        selection({ revision: "7", selected: "1" }),
      ),
    );
    await reachActiveRestore(hook.result);
    hook.unmount();

    await act(async () => {
      vi.advanceTimersByTime(10_000);
      await Promise.resolve();
    });
    expect(native.getRestoreJob).not.toHaveBeenCalled();
  });

  it("DESKTOP-RESTORE-OPEN-DESTINATION-026 opens only the completed opaque job", async () => {
    vi.useFakeTimers();
    native.selectRestoreDestination.mockResolvedValue(destination());
    native.createRestorePlan.mockResolvedValue(plan());
    native.startRestore.mockResolvedValue(job("completed"));
    native.openRestoreDestination.mockResolvedValue({
      schemaVersion: 1,
      opened: true,
    });
    const { result } = renderHook(() =>
      useRestoreWorkflow(
        true,
        "scan-1",
        selection({ revision: "7", selected: "1" }),
      ),
    );
    await reachActiveRestore(result);
    expect(result.current.state.phase).toBe("finished");

    act(() => result.current.openDestination());
    await flushUntil(
      () => native.openRestoreDestination.mock.calls.length === 1,
    );
    expect(native.openRestoreDestination).toHaveBeenCalledWith(
      expect.stringMatching(/^restore-/),
      "job-1",
    );
    expect(JSON.stringify(result.current.state)).not.toMatch(
      /(?:path|extent|offset|handle|executable|recoveredBytes)/i,
    );
  });

  it("keeps a completed job and permits retry when opening its opaque destination fails", async () => {
    native.selectRestoreDestination.mockResolvedValue(destination());
    native.createRestorePlan.mockResolvedValue(plan());
    native.startRestore.mockResolvedValue(job("completed"));
    native.openRestoreDestination
      .mockRejectedValueOnce({ code: "RESTORE_INTERNAL" })
      .mockResolvedValueOnce({ schemaVersion: 1, opened: true });

    const { result } = renderHook(() =>
      useRestoreWorkflow(
        true,
        "scan-1",
        selection({ revision: "7", selected: "1" }),
      ),
    );
    await reachActiveRestore(result);
    expect(result.current.state.phase).toBe("finished");

    act(() => result.current.openDestination());
    await flushUntil(() => result.current.state.error === "RESTORE_INTERNAL");

    expect(result.current.state.phase).toBe("finished");
    expect(result.current.state.job?.status).toBe("completed");
    expect(result.current.state.job?.manifest).not.toBeNull();
    expect(result.current.state.operationPhase).toBe("idle");

    act(() => result.current.openDestination());
    await flushUntil(
      () => native.openRestoreDestination.mock.calls.length === 2,
    );
    await flushUntil(() => result.current.state.error === null);

    expect(result.current.state.phase).toBe("finished");
    expect(result.current.state.job?.status).toBe("completed");
  });

  it("calculates directory-only progress with BigInt item basis points", () => {
    expect(
      restoreProgressBasisPoints(
        job("running", {
          itemsTotal: "3",
          itemsCompleted: "1",
          bytesTotal: "0",
          bytesCompleted: "0",
          currentItem: { ordinal: "1", candidateId: "2", kind: "directory" },
        }),
      ),
    ).toBe(3333);
  });
});

async function reachActiveRestore(
  result: ReturnType<
    typeof renderHook<ReturnType<typeof useRestoreWorkflow>, unknown>
  >["result"],
) {
  act(() => result.current.chooseDestination());
  await flushUntil(() => result.current.state.phase === "setup");
  act(() => result.current.createPlan());
  await flushUntil(() => result.current.state.phase === "review");
  act(() => result.current.start());
  await flushUntil(
    () =>
      result.current.state.phase === "active" ||
      result.current.state.phase === "finished",
  );
}

async function flushUntil(predicate: () => boolean) {
  for (let attempt = 0; attempt < 20; attempt += 1) {
    if (predicate()) {
      return;
    }
    await act(async () => {
      await Promise.resolve();
    });
  }
  expect(predicate()).toBe(true);
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function selection({
  revision = "7",
  selected = "1",
  bestEffort = "0",
  ineligible = "0",
}: {
  revision?: string;
  selected?: string;
  bestEffort?: string;
  ineligible?: string;
} = {}): CandidateSelectionSummary {
  return {
    selectionRevision: revision,
    selectedCandidates: selected,
    selectedFiles: selected,
    selectedDirectories: "0",
    selectedLogicalBytes: selected === "0" ? "0" : "9007199254740993",
    bestEffortCandidates: bestEffort,
    conflictedCandidates: "0",
    ineligibleCandidates: ineligible,
    matchingSelectedCandidates: selected,
  };
}

function destination(): RestoreDestinationSummary {
  return {
    schemaVersion: 1,
    destinationId: "destination-1",
    label: "Recovered files",
    volumeLabel: "Backup",
    fileSystem: "NTFS",
    freeBytes: "18446744073709551615",
    relation: "different",
  };
}

function plan(
  overrides: Partial<RestorePlanSummary> = {},
): RestorePlanSummary {
  return {
    schemaVersion: 1,
    planId: "plan-1",
    planDigest:
      "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    scanId: "scan-1",
    destinationId: "destination-1",
    selectionRevision: "7",
    collisionPolicy: "rename",
    partialFilePolicy: "completeOnly",
    itemsTotal: "3",
    filesTotal: "2",
    directoriesTotal: "1",
    logicalBytes: "9007199254740993",
    bestEffortItems: "0",
    ...overrides,
  };
}

function job(
  status: RestoreJobSnapshot["status"],
  overrides: Partial<RestoreJobSnapshot> = {},
): RestoreJobSnapshot {
  const completed = status === "completed";
  return {
    schemaVersion: 1,
    jobId: "job-1",
    planId: "plan-1",
    status,
    itemsTotal: "3",
    itemsCompleted: completed ? "3" : "0",
    itemsFailed: "0",
    itemsCancelled: "0",
    bytesTotal: "9007199254740993",
    bytesCompleted: completed ? "9007199254740993" : "0",
    currentItem: null,
    warnings: [],
    manifest: completed
      ? {
          manifestSha256:
            "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
          completionStatus: "completedDurable",
          publishedItems: "3",
          partialItems: "1",
        }
      : null,
    ...overrides,
  };
}
