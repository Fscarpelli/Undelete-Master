import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  ActionableCandidateRow,
  CandidateQuery,
  CandidateQueryPage,
  CandidateSelectionSummary,
  CandidateSort,
} from "../api/storage";
import { StorageCommandError } from "../api/storageDesktop";
import { useResultsWorkspace } from "./resultsWorkspace";

const native = vi.hoisted(() => ({
  queryCandidatePage: vi.fn(),
  updateCandidateSelection: vi.fn(),
}));

vi.mock("../api/storageDesktop", async (importOriginal) => {
  const actual =
    await importOriginal<typeof import("../api/storageDesktop")>();
  return {
    ...actual,
    queryCandidatePage: native.queryCandidatePage,
    updateCandidateSelection: native.updateCandidateSelection,
  };
});

describe("useResultsWorkspace", () => {
  beforeEach(() => {
    native.queryCandidatePage.mockReset();
    native.updateCandidateSelection.mockReset();
  });

  it("loads one bounded page with native facets and selection using score-descending order", async () => {
    native.queryCandidatePage.mockResolvedValue(
      queryPage({ candidateLabel: "initial" }),
    );

    const { result } = renderHook(() =>
      useResultsWorkspace(true, "scan-1"),
    );

    await waitFor(() => expect(result.current.state.phase).toBe("ready"));

    expect(native.queryCandidatePage).toHaveBeenCalledWith(
      expect.stringMatching(/^result-/),
      "scan-1",
      expect.objectContaining({ revision: "0" }),
      { field: "recoverabilityScore", direction: "descending" },
      null,
    );
    expect(result.current.state.page?.candidates).toHaveLength(1);
    expect(result.current.state.page?.candidates[0]?.displayPath).toBe(
      "initial",
    );
    expect(result.current.state.extensionFacets).toEqual([
      { extension: "jpg", count: "1" },
    ]);
    expect(result.current.state.selection?.selectionRevision).toBe("0");
    expect(result.current.state.pageCache).toHaveLength(1);
    expect(result.current.state.pageCache[0]).not.toHaveProperty(
      "extensionFacets",
    );
    expect(result.current.state.pageCache[0]).not.toHaveProperty("selection");
  });

  it("RESULT-LATE-RESPONSE-008 serializes calls, coalesces intent, and ignores the late response", async () => {
    const first = deferred<CandidateQueryPage>();
    const latest = deferred<CandidateQueryPage>();
    native.queryCandidatePage
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(latest.promise);

    const { result } = renderHook(() =>
      useResultsWorkspace(true, "scan-1"),
    );
    await waitFor(() =>
      expect(native.queryCandidatePage).toHaveBeenCalledTimes(1),
    );

    act(() => {
      result.current.submitSearch("superseded");
      result.current.sortBy("path");
      result.current.submitSearch("latest");
    });

    expect(native.queryCandidatePage).toHaveBeenCalledTimes(1);
    expect(result.current.state.query.search).toBe("latest");
    expect(result.current.state.requestRevision).toBe(3);
    expect(result.current.state.page).toBeNull();

    await act(async () => {
      first.resolve(
        queryPage({
          queryRevision: "0",
          candidateLabel: "late-response",
        }),
      );
      await first.promise;
    });

    await waitFor(() =>
      expect(native.queryCandidatePage).toHaveBeenCalledTimes(2),
    );
    const latestCall = native.queryCandidatePage.mock.calls[1];
    expect(latestCall?.[2]).toMatchObject({
      revision: "3",
      search: "latest",
    });
    expect(latestCall?.[3]).toEqual({
      field: "path",
      direction: "ascending",
    });
    expect(latestCall?.[4]).toBeNull();
    expect(result.current.state.page).toBeNull();

    await act(async () => {
      latest.resolve(
        queryPage({
          queryId: "query-latest",
          queryRevision: "3",
          candidateLabel: "latest-response",
        }),
      );
      await latest.promise;
    });

    await waitFor(() =>
      expect(result.current.state.page?.candidates[0]?.displayPath).toBe(
        "latest-response",
      ),
    );
  });

  it("uses native cursor identity and evicts the least recently used fourth page", async () => {
    native.queryCandidatePage.mockImplementation(
      async (
        _requestId: string,
        scanId: string,
        query: CandidateQuery,
        _sort: CandidateSort,
        cursor: string | null,
      ) =>
        queryPage({
          scanId,
          queryRevision: query.revision,
          cursor,
          nextCursor:
            cursor === null
              ? "cursor-1"
              : cursor === "cursor-1"
                ? "cursor-2"
                : cursor === "cursor-2"
                  ? "cursor-3"
                  : "cursor-4",
          candidateLabel: cursor ?? "first",
        }),
    );

    const { result } = renderHook(() =>
      useResultsWorkspace(true, "scan-1"),
    );
    await waitFor(() => expect(result.current.state.phase).toBe("ready"));

    for (const pageIndex of [1, 2, 3]) {
      act(() => result.current.nextPage());
      await waitFor(() =>
        expect(result.current.state.currentPageIndex).toBe(pageIndex),
      );
    }

    expect(
      native.queryCandidatePage.mock.calls.map((call) => call[4]),
    ).toEqual([null, "cursor-1", "cursor-2", "cursor-3"]);
    expect(result.current.state.pageCache.map((page) => page.cursor)).toEqual([
      "cursor-1",
      "cursor-2",
      "cursor-3",
    ]);
    expect(result.current.state.page?.cursor).toBe("cursor-3");

    act(() => result.current.previousPage());
    await waitFor(() =>
      expect(result.current.state.currentPageIndex).toBe(2),
    );
    expect(result.current.state.page?.cursor).toBe("cursor-2");
    expect(native.queryCandidatePage).toHaveBeenCalledTimes(4);
    expect(result.current.state.pageCache.map((page) => page.cursor)).toEqual([
      "cursor-1",
      "cursor-3",
      "cursor-2",
    ]);
  });

  it("shows checkbox changes only after a native selection response and refreshed page", async () => {
    const selectionUpdate = deferred<ReturnType<typeof selectionResponse>>();
    const refreshedPage = deferred<CandidateQueryPage>();
    native.queryCandidatePage
      .mockResolvedValueOnce(queryPage())
      .mockReturnValueOnce(refreshedPage.promise);
    native.updateCandidateSelection.mockReturnValue(selectionUpdate.promise);

    const { result } = renderHook(() =>
      useResultsWorkspace(true, "scan-1"),
    );
    await waitFor(() => expect(result.current.state.phase).toBe("ready"));

    act(() => result.current.setCandidateSelected("1", true));
    expect(result.current.state.selectionPhase).toBe("updating");
    expect(result.current.state.page?.candidates[0]?.selected).toBe(false);
    expect(native.updateCandidateSelection).toHaveBeenCalledWith(
      expect.stringMatching(/^result-/),
      "scan-1",
      "query-1",
      { type: "setIds", candidateIds: ["1"], selected: true },
      "0",
    );

    await act(async () => {
      selectionUpdate.resolve(selectionResponse({ revision: "1" }));
      await selectionUpdate.promise;
    });
    await waitFor(() =>
      expect(native.queryCandidatePage).toHaveBeenCalledTimes(2),
    );
    expect(result.current.state.page?.candidates[0]?.selected).toBe(false);
    expect(result.current.state.selection?.selectionRevision).toBe("1");

    await act(async () => {
      refreshedPage.resolve(
        queryPage({
          candidateSelected: true,
          selection: selectionSummary({ revision: "1", selected: "1" }),
        }),
      );
      await refreshedPage.promise;
    });
    await waitFor(() =>
      expect(result.current.state.selectionPhase).toBe("idle"),
    );
    expect(result.current.state.page?.candidates[0]?.selected).toBe(true);
  });

  it("keeps page identity stable while a native selection update refreshes its originating page", async () => {
    const selectionUpdate = deferred<ReturnType<typeof selectionResponse>>();
    let rootPageReads = 0;
    native.queryCandidatePage.mockImplementation(
      async (
        _requestId: string,
        scanId: string,
        query: CandidateQuery,
        _sort: CandidateSort,
        cursor: string | null,
      ) => {
        if (cursor === "cursor-1") {
          return queryPage({
            scanId,
            queryRevision: query.revision,
            cursor,
            candidateLabel: "page-2",
          });
        }
        rootPageReads += 1;
        return queryPage({
          scanId,
          queryRevision: query.revision,
          cursor: null,
          nextCursor: "cursor-1",
          candidateLabel: "page-1",
          candidateSelected: rootPageReads > 1,
          selection: selectionSummary({
            revision: rootPageReads > 1 ? "1" : "0",
            selected: rootPageReads > 1 ? "1" : "0",
          }),
        });
      },
    );
    native.updateCandidateSelection.mockReturnValue(selectionUpdate.promise);

    const { result } = renderHook(() =>
      useResultsWorkspace(true, "scan-1"),
    );
    await waitFor(() => expect(result.current.state.phase).toBe("ready"));

    act(() => result.current.nextPage());
    await waitFor(() =>
      expect(result.current.state.page?.candidates[0]?.displayPath).toBe(
        "page-2",
      ),
    );
    act(() => result.current.previousPage());
    expect(result.current.state.currentPageIndex).toBe(0);
    expect(result.current.state.page?.cursor).toBeNull();

    act(() => {
      result.current.setCandidateSelected("1", true);
      result.current.nextPage();
    });

    expect(result.current.state.selectionPhase).toBe("updating");
    expect(result.current.state.currentPageIndex).toBe(0);
    expect(result.current.state.page?.cursor).toBeNull();

    await act(async () => {
      selectionUpdate.resolve(
        selectionResponse({ revision: "1", selected: "1" }),
      );
      await selectionUpdate.promise;
    });
    await waitFor(() =>
      expect(result.current.state.selectionPhase).toBe("idle"),
    );

    expect(result.current.state.currentPageIndex).toBe(0);
    expect(result.current.state.page?.cursor).toBeNull();
    expect(result.current.state.page?.candidates[0]?.displayPath).toBe(
      "page-1",
    );
    expect(result.current.state.page?.candidates[0]?.selected).toBe(true);
  });

  it("sends select-all as one tagged native operation without enumerating IDs", async () => {
    native.queryCandidatePage.mockResolvedValue(queryPage());
    native.updateCandidateSelection.mockResolvedValue(
      selectionResponse({ revision: "1", selected: "1" }),
    );

    const { result } = renderHook(() =>
      useResultsWorkspace(true, "scan-1"),
    );
    await waitFor(() => expect(result.current.state.phase).toBe("ready"));

    act(() => result.current.selectAllMatching());
    await waitFor(() =>
      expect(native.updateCandidateSelection).toHaveBeenCalledTimes(1),
    );

    expect(native.updateCandidateSelection.mock.calls[0]?.[3]).toEqual({
      type: "selectAllMatching",
    });
  });

  it("resets selected-only pagination to the null cursor after selection changes", async () => {
    let authoritativeRevision = "1";
    native.queryCandidatePage.mockImplementation(
      async (
        _requestId: string,
        scanId: string,
        query: CandidateQuery,
        _sort: CandidateSort,
        cursor: string | null,
      ) =>
        queryPage({
          scanId,
          queryRevision: query.revision,
          queryId: query.selectedOnly
            ? `query-selected-rev${authoritativeRevision}`
            : "query-global",
          cursor,
          nextCursor: cursor === null ? "selected-next" : null,
          selection: selectionSummary({
            revision: authoritativeRevision,
            selected: "1",
          }),
        }),
    );
    native.updateCandidateSelection.mockImplementation(async () => {
      authoritativeRevision = "2";
      return selectionResponse({
        queryId: "query-selected-rev1",
        revision: "2",
      });
    });

    const { result } = renderHook(() =>
      useResultsWorkspace(true, "scan-1"),
    );
    await waitFor(() => expect(result.current.state.phase).toBe("ready"));

    act(() => result.current.setSelectedOnly(true));
    await waitFor(() =>
      expect(result.current.state.query.selectedOnly).toBe(true),
    );
    await waitFor(() => expect(result.current.state.phase).toBe("ready"));

    act(() => result.current.nextPage());
    await waitFor(() =>
      expect(result.current.state.currentPageIndex).toBe(1),
    );

    act(() => result.current.clearAll());
    await waitFor(() =>
      expect(result.current.state.selectionPhase).toBe("idle"),
    );

    expect(native.queryCandidatePage.mock.calls.at(-1)?.[4]).toBeNull();
    expect(result.current.state.currentPageIndex).toBe(0);
    expect(result.current.state.cursorHistory).toEqual([null]);
    expect(result.current.state.page?.queryId).toBe("query-selected-rev2");
    expect(result.current.state.error).toBeNull();
  });

  it("recovers a stale cursor through one fresh null-cursor native query", async () => {
    native.queryCandidatePage
      .mockResolvedValueOnce(queryPage({ nextCursor: "stale-cursor" }))
      .mockRejectedValueOnce(new StorageCommandError("RESULT_CURSOR_STALE"))
      .mockResolvedValueOnce(
        queryPage({
          queryId: "query-refreshed",
          candidateLabel: "fresh",
        }),
      );

    const { result } = renderHook(() =>
      useResultsWorkspace(true, "scan-1"),
    );
    await waitFor(() => expect(result.current.state.phase).toBe("ready"));

    act(() => result.current.nextPage());

    await waitFor(() =>
      expect(result.current.state.page?.candidates[0]?.displayPath).toBe(
        "fresh",
      ),
    );
    expect(
      native.queryCandidatePage.mock.calls.map((call) => call[4]),
    ).toEqual([null, "stale-cursor", null]);
    expect(result.current.state.error).toBeNull();
    expect(result.current.state.currentPageIndex).toBe(0);
  });

  it("rejects response bindings and a 101st row before exposing them", async () => {
    native.queryCandidatePage.mockResolvedValueOnce(
      queryPage({ cursor: "wrong-cursor" }),
    );

    const first = renderHook(() => useResultsWorkspace(true, "scan-1"));
    await waitFor(() =>
      expect(first.result.current.state.phase).toBe("error"),
    );
    expect(first.result.current.state.error).toBe("REPORT_INCOMPATIBLE");
    expect(first.result.current.state.page).toBeNull();
    first.unmount();

    native.queryCandidatePage.mockReset();
    native.queryCandidatePage.mockResolvedValueOnce(
      queryPage({
        candidates: Array.from({ length: 101 }, (_, index) =>
          candidate(String(index + 1), `candidate-${index + 1}`),
        ),
        filteredTotal: "101",
      }),
    );

    const second = renderHook(() => useResultsWorkspace(true, "scan-2"));
    await waitFor(() =>
      expect(second.result.current.state.phase).toBe("error"),
    );
    expect(second.result.current.state.error).toBe("REPORT_INCOMPATIBLE");
    expect(second.result.current.state.page).toBeNull();
  });

  it("ignores a previous scan generation and loads the replacement scan serially", async () => {
    const oldScan = deferred<CandidateQueryPage>();
    native.queryCandidatePage
      .mockReturnValueOnce(oldScan.promise)
      .mockResolvedValueOnce(
        queryPage({
          scanId: "scan-2",
          queryId: "query-scan-2",
          candidateLabel: "scan-2-row",
        }),
      );

    const { result, rerender } = renderHook(
      ({ scanId }) => useResultsWorkspace(true, scanId),
      { initialProps: { scanId: "scan-1" as string | null } },
    );
    await waitFor(() =>
      expect(native.queryCandidatePage).toHaveBeenCalledTimes(1),
    );

    rerender({ scanId: "scan-2" });
    expect(native.queryCandidatePage).toHaveBeenCalledTimes(1);

    await act(async () => {
      oldScan.resolve(queryPage({ candidateLabel: "obsolete" }));
      await oldScan.promise;
    });

    await waitFor(() =>
      expect(result.current.state.page?.candidates[0]?.displayPath).toBe(
        "scan-2-row",
      ),
    );
    expect(result.current.state.scanId).toBe("scan-2");
  });

  it("fails closed before dispatch for a 129th extension or invalid search and score bounds", async () => {
    const first = deferred<CandidateQueryPage>();
    native.queryCandidatePage
      .mockReturnValueOnce(first.promise)
      .mockImplementation(
        async (
          _requestId: string,
          scanId: string,
          query: CandidateQuery,
        ) =>
          queryPage({
            scanId,
            queryRevision: query.revision,
            queryId: "query-bounded",
          }),
      );

    const { result } = renderHook(() =>
      useResultsWorkspace(true, "scan-1"),
    );
    await waitFor(() =>
      expect(native.queryCandidatePage).toHaveBeenCalledTimes(1),
    );

    act(() => {
      for (let index = 0; index < 129; index += 1) {
        result.current.toggleExtension(`ext${index}`);
      }
    });

    expect(result.current.state.query.extensions).toHaveLength(128);
    expect(result.current.state.error).toBe("RESULT_QUERY_INVALID");
    expect(result.current.state.phase).toBe("error");
    expect(native.queryCandidatePage).toHaveBeenCalledTimes(1);

    await act(async () => {
      first.resolve(queryPage());
      await first.promise;
    });
    await waitFor(() =>
      expect(native.queryCandidatePage).toHaveBeenCalledTimes(2),
    );
    expect(native.queryCandidatePage.mock.calls[1]?.[2].extensions).toHaveLength(
      128,
    );

    const callsBeforeInvalidInput = native.queryCandidatePage.mock.calls.length;
    act(() => {
      result.current.submitSearch(`unsafe\u202E`);
      result.current.setScoreRange(90, 10);
      result.current.setScoreRange(-1, 101);
    });
    expect(native.queryCandidatePage).toHaveBeenCalledTimes(
      callsBeforeInvalidInput,
    );
    expect(result.current.state.error).toBe("RESULT_QUERY_INVALID");
  });

  it("does not enqueue selection against a retained page while a page query is loading", async () => {
    const nextPage = deferred<CandidateQueryPage>();
    native.queryCandidatePage
      .mockResolvedValueOnce(queryPage({ nextCursor: "cursor-next" }))
      .mockReturnValueOnce(nextPage.promise);

    const { result } = renderHook(() =>
      useResultsWorkspace(true, "scan-1"),
    );
    await waitFor(() => expect(result.current.state.phase).toBe("ready"));
    act(() => result.current.nextPage());
    expect(result.current.state.phase).toBe("loading");

    act(() => result.current.setCandidateSelected("1", true));
    expect(native.updateCandidateSelection).not.toHaveBeenCalled();
    expect(result.current.state.selectionPhase).toBe("idle");

    await act(async () => {
      nextPage.resolve(
        queryPage({ cursor: "cursor-next", queryId: "query-1" }),
      );
      await nextPage.promise;
    });
  });
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function candidate(
  id = "1",
  displayPath = "candidate",
  selected = false,
): ActionableCandidateRow {
  return {
    id,
    displayPath,
    extension: "jpg",
    kind: "file",
    state: "likelyComplete",
    sizeBytes: "42",
    metadataConfidence: "high",
    recoverabilityScore: 90,
    pathState: "exact",
    method: "ntfsMetadata",
    eligibility: "complete",
    selected,
    warnings: [],
  };
}

function selectionSummary({
  revision = "0",
  selected = "0",
}: {
  revision?: string;
  selected?: string;
} = {}): CandidateSelectionSummary {
  return {
    selectionRevision: revision,
    selectedCandidates: selected,
    selectedFiles: selected,
    selectedDirectories: "0",
    selectedLogicalBytes: selected === "0" ? "0" : "42",
    bestEffortCandidates: "0",
    conflictedCandidates: "0",
    ineligibleCandidates: "0",
    matchingSelectedCandidates: selected,
  };
}

function queryPage({
  scanId = "scan-1",
  queryId = "query-1",
  queryRevision = "0",
  cursor = null,
  nextCursor = null,
  candidateLabel = "candidate",
  candidateSelected = false,
  candidates,
  filteredTotal,
  selection = selectionSummary(),
}: {
  scanId?: string;
  queryId?: string;
  queryRevision?: string;
  cursor?: string | null;
  nextCursor?: string | null;
  candidateLabel?: string;
  candidateSelected?: boolean;
  candidates?: ActionableCandidateRow[];
  filteredTotal?: string;
  selection?: CandidateSelectionSummary;
} = {}): CandidateQueryPage {
  const rows = candidates ?? [candidate("1", candidateLabel, candidateSelected)];
  return {
    schemaVersion: 1,
    scanId,
    queryId,
    queryRevision,
    cursor,
    nextCursor,
    filteredTotal: filteredTotal ?? String(rows.length),
    extensionFacets: [{ extension: "jpg", count: String(rows.length) }],
    selection,
    candidates: rows,
  };
}

function selectionResponse({
  queryId = "query-1",
  revision = "1",
  selected = "0",
}: {
  queryId?: string;
  revision?: string;
  selected?: string;
} = {}) {
  return {
    schemaVersion: 1 as const,
    scanId: "scan-1",
    queryId,
    selectionRevision: revision,
    selection: selectionSummary({ revision, selected }),
  };
}
