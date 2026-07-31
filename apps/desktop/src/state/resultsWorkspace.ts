import { useCallback, useEffect, useRef, useState } from "react";
import type {
  ActionableCandidateRow,
  CandidateExtensionFacet,
  CandidateKind,
  CandidateQuery,
  CandidateSelectionOperation,
  CandidateSelectionSummary,
  CandidateSort,
  CandidateSortField,
  CandidateState,
  DiscoveryMethod,
  MetadataConfidence,
  RecoveryEligibility,
} from "../api/storage";
import {
  normalizeStorageError,
  queryCandidatePage,
  StorageCommandError,
  type StorageErrorCode,
  updateCandidateSelection,
} from "../api/storageDesktop";

const MAX_CANDIDATES_PER_PAGE = 100;
const MAX_CACHED_PAGES = 3;
const MAX_QUERY_TEXT_SCALARS = 512;
const MAX_QUERY_EXTENSIONS = 128;
const MAX_EXTENSION_SCALARS = 255;
const MAX_U64 = 18_446_744_073_709_551_615n;

export type ResultsWorkspacePhase = "idle" | "loading" | "ready" | "error";
export type ResultsSelectionPhase = "idle" | "updating";

export interface CachedCandidatePage {
  scanId: string;
  queryId: string;
  queryRevision: string;
  cursor: string | null;
  nextCursor: string | null;
  filteredTotal: string;
  candidates: ActionableCandidateRow[];
}

export interface ResultsWorkspaceState {
  phase: ResultsWorkspacePhase;
  scanId: string | null;
  query: CandidateQuery;
  sort: CandidateSort;
  requestRevision: number;
  currentPageIndex: number;
  cursorHistory: Array<string | null>;
  pageCache: CachedCandidatePage[];
  page: CachedCandidatePage | null;
  selection: CandidateSelectionSummary | null;
  extensionFacets: CandidateExtensionFacet[];
  selectionPhase: ResultsSelectionPhase;
  error: StorageErrorCode | null;
}

export interface ResultsWorkspaceController {
  state: ResultsWorkspaceState;
  submitSearch(search: string): void;
  toggleExtension(extension: string): void;
  toggleKind(kind: CandidateKind): void;
  toggleMetadataConfidence(confidence: MetadataConfidence): void;
  toggleMethod(method: DiscoveryMethod): void;
  toggleCandidateState(candidateState: CandidateState): void;
  toggleEligibility(eligibility: RecoveryEligibility): void;
  setScoreRange(minimum: number | null, maximum: number | null): void;
  setSelectedOnly(selectedOnly: boolean): void;
  sortBy(field: CandidateSortField): void;
  nextPage(): void;
  previousPage(): void;
  retry(): void;
  setCandidateSelected(candidateId: string, selected: boolean): void;
  selectAllMatching(): void;
  clearMatching(): void;
  clearAll(): void;
}

interface PageTask {
  kind: "query" | "page";
  generation: number;
  intentRevision: number;
  scanId: string;
  query: CandidateQuery;
  sort: CandidateSort;
  cursor: string | null;
  pageIndex: number;
  cursorHistory: Array<string | null>;
  expectedQueryId: string | null;
}

interface SelectionTask {
  kind: "selection";
  generation: number;
  scanId: string;
  operation: CandidateSelectionOperation;
}

type ResultTask = PageTask | SelectionTask;

let requestSequence = 0;

function nextRequestId(): string {
  requestSequence += 1;
  return `result-${requestSequence.toString(36)}`;
}

function initialQuery(): CandidateQuery {
  return {
    revision: "0",
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
  };
}

function initialState(
  runtimeAvailable: boolean,
  scanId: string | null,
): ResultsWorkspaceState {
  return {
    phase: runtimeAvailable && scanId !== null ? "loading" : "idle",
    scanId,
    query: initialQuery(),
    sort: { field: "recoverabilityScore", direction: "descending" },
    requestRevision: 0,
    currentPageIndex: 0,
    cursorHistory: [null],
    pageCache: [],
    page: null,
    selection: null,
    extensionFacets: [],
    selectionPhase: "idle",
    error: null,
  };
}

function cachedPage(
  page: Awaited<ReturnType<typeof queryCandidatePage>>,
): CachedCandidatePage {
  return {
    scanId: page.scanId,
    queryId: page.queryId,
    queryRevision: page.queryRevision,
    cursor: page.cursor,
    nextCursor: page.nextCursor,
    filteredTotal: page.filteredTotal,
    candidates: page.candidates,
  };
}

function touchPage(
  pages: readonly CachedCandidatePage[],
  page: CachedCandidatePage,
): CachedCandidatePage[] {
  return [
    ...pages.filter(
      (cached) =>
        cached.queryRevision !== page.queryRevision ||
        cached.cursor !== page.cursor,
    ),
    page,
  ].slice(-MAX_CACHED_PAGES);
}

function toggleValue<T>(values: readonly T[], value: T): T[] {
  return values.includes(value)
    ? values.filter((entry) => entry !== value)
    : [...values, value];
}

function forbiddenQueryCharacter(character: string): boolean {
  const codePoint = character.codePointAt(0);
  return (
    codePoint === undefined ||
    codePoint <= 0x1f ||
    (codePoint >= 0x7f && codePoint <= 0x9f) ||
    codePoint === 0x061c ||
    codePoint === 0x200e ||
    codePoint === 0x200f ||
    (codePoint >= 0x202a && codePoint <= 0x202e) ||
    (codePoint >= 0x2066 && codePoint <= 0x2069) ||
    codePoint === 0xfeff
  );
}

function validSearch(search: string): boolean {
  const characters = [...search];
  return (
    characters.length <= MAX_QUERY_TEXT_SCALARS &&
    !characters.some(forbiddenQueryCharacter)
  );
}

function canonicalExtension(extension: string): string | null {
  if (extension === "") {
    return "";
  }
  if ([...extension].some(forbiddenQueryCharacter)) {
    return null;
  }
  const trimmed = extension.trim();
  if (
    trimmed.length === 0 ||
    [...trimmed].length > MAX_EXTENSION_SCALARS ||
    trimmed.includes(".") ||
    trimmed.includes("/") ||
    trimmed.includes("\\")
  ) {
    return null;
  }
  const normalized = trimmed.toLocaleLowerCase("en-US");
  return [...normalized].length <= MAX_EXTENSION_SCALARS ? normalized : null;
}

function validScore(score: number | null): boolean {
  return (
    score === null ||
    (Number.isInteger(score) && Number.isFinite(score) && score >= 0 && score <= 100)
  );
}

function validCandidateId(candidateId: string): boolean {
  if (!/^(0|[1-9][0-9]*)$/u.test(candidateId) || candidateId.length > 20) {
    return false;
  }
  return BigInt(candidateId) <= MAX_U64;
}

function nextSelectionRevision(current: string, next: string): boolean {
  return BigInt(next) === BigInt(current) + 1n;
}

function isRecoverableStaleError(code: StorageErrorCode): boolean {
  return code === "RESULT_CURSOR_STALE" || code === "RESULT_SELECTION_STALE";
}

export function useResultsWorkspace(
  runtimeAvailable: boolean,
  scanId: string | null,
): ResultsWorkspaceController {
  const [state, setState] = useState<ResultsWorkspaceState>(() =>
    initialState(runtimeAvailable, scanId),
  );
  const stateRef = useRef(state);
  const mountedRef = useRef(false);
  const generationRef = useRef(0);
  const intentRevisionRef = useRef(0);
  const tasksRef = useRef<ResultTask[]>([]);
  const runningRef = useRef(false);
  const pumpRef = useRef<() => void>(() => undefined);
  const executeTaskRef = useRef<(task: ResultTask) => Promise<void>>(
    async () => undefined,
  );

  const commitState = useCallback(
    (
      update:
        | ResultsWorkspaceState
        | ((current: ResultsWorkspaceState) => ResultsWorkspaceState),
    ) => {
      const next =
        typeof update === "function" ? update(stateRef.current) : update;
      stateRef.current = next;
      if (mountedRef.current) {
        setState(next);
      }
    },
    [],
  );

  const pump = useCallback(() => {
    if (!mountedRef.current || runningRef.current) {
      return;
    }
    const task = tasksRef.current.shift();
    if (task === undefined) {
      return;
    }
    runningRef.current = true;
    void executeTaskRef
      .current(task)
      .finally(() => {
        runningRef.current = false;
        pumpRef.current();
      });
  }, []);

  const enqueueTask = useCallback(
    (task: ResultTask) => {
      if (task.kind === "query") {
        tasksRef.current = tasksRef.current.filter(
          (pending) => pending.kind === "selection",
        );
      } else if (task.kind === "page") {
        tasksRef.current = tasksRef.current.filter(
          (pending) => pending.kind !== "page",
        );
      }
      tasksRef.current.push(task);
      pump();
    },
    [pump],
  );

  const failWith = useCallback(
    (code: StorageErrorCode) => {
      commitState((current) => ({
        ...current,
        phase: "error",
        selectionPhase: "idle",
        error: code,
      }));
    },
    [commitState],
  );

  const freshQueryTask = useCallback(
    (current: ResultsWorkspaceState): PageTask | null => {
      if (current.scanId === null) {
        return null;
      }
      return {
        kind: "query",
        generation: generationRef.current,
        intentRevision: current.requestRevision,
        scanId: current.scanId,
        query: current.query,
        sort: current.sort,
        cursor: null,
        pageIndex: 0,
        cursorHistory: [null],
        expectedQueryId: null,
      };
    },
    [],
  );

  const recoverThroughFreshQuery = useCallback(() => {
    const current = stateRef.current;
    const task = freshQueryTask(current);
    if (task === null) {
      return;
    }
    commitState({
      ...current,
      phase: "loading",
      currentPageIndex: 0,
      cursorHistory: [null],
      pageCache: [],
      page: null,
      error: null,
    });
    enqueueTask(task);
  }, [commitState, enqueueTask, freshQueryTask]);

  const taskIsCurrent = useCallback((task: PageTask) => {
    return (
      mountedRef.current &&
      task.generation === generationRef.current &&
      task.intentRevision === intentRevisionRef.current &&
      stateRef.current.scanId === task.scanId &&
      stateRef.current.requestRevision === task.intentRevision
    );
  }, []);

  const applyNativePage = useCallback(
    (
      task: PageTask,
      page: Awaited<ReturnType<typeof queryCandidatePage>>,
    ) => {
      if (!taskIsCurrent(task)) {
        return;
      }
      if (
        page.scanId !== task.scanId ||
        page.queryRevision !== task.query.revision ||
        page.cursor !== task.cursor ||
        (task.expectedQueryId !== null &&
          page.queryId !== task.expectedQueryId) ||
        page.candidates.length > MAX_CANDIDATES_PER_PAGE
      ) {
        throw new StorageCommandError("REPORT_INCOMPATIBLE");
      }
      const currentSelection = stateRef.current.selection;
      if (
        currentSelection !== null &&
        BigInt(page.selection.selectionRevision) <
          BigInt(currentSelection.selectionRevision)
      ) {
        throw new StorageCommandError("REPORT_INCOMPATIBLE");
      }
      const visiblePage = cachedPage(page);
      commitState((current) => ({
        ...current,
        phase: "ready",
        currentPageIndex: task.pageIndex,
        cursorHistory: task.cursorHistory,
        pageCache: touchPage(current.pageCache, visiblePage),
        page: visiblePage,
        selection: page.selection,
        extensionFacets: page.extensionFacets,
        selectionPhase: "idle",
        error: null,
      }));
    },
    [commitState, taskIsCurrent],
  );

  const runPageTask = useCallback(
    async (task: PageTask) => {
      try {
        const page = await queryCandidatePage(
          nextRequestId(),
          task.scanId,
          task.query,
          task.sort,
          task.cursor,
        );
        applyNativePage(task, page);
      } catch (error) {
        if (!taskIsCurrent(task)) {
          return;
        }
        const code = normalizeStorageError(error).code;
        if (isRecoverableStaleError(code) && task.cursor !== null) {
          recoverThroughFreshQuery();
          return;
        }
        failWith(code);
      }
    },
    [applyNativePage, failWith, recoverThroughFreshQuery, taskIsCurrent],
  );

  const runSelectionTask = useCallback(
    async (task: SelectionTask) => {
      if (
        !mountedRef.current ||
        task.generation !== generationRef.current ||
        stateRef.current.scanId !== task.scanId
      ) {
        return;
      }
      const before = stateRef.current;
      const page = before.page;
      const selection = before.selection;
      if (page === null || selection === null) {
        commitState((current) => ({
          ...current,
          selectionPhase: "idle",
        }));
        return;
      }
      const queryId = page.queryId;
      const expectedRevision = selection.selectionRevision;
      const operationIntentRevision = before.requestRevision;

      try {
        const response = await updateCandidateSelection(
          nextRequestId(),
          task.scanId,
          queryId,
          task.operation,
          expectedRevision,
        );
        if (
          !mountedRef.current ||
          task.generation !== generationRef.current ||
          stateRef.current.scanId !== task.scanId
        ) {
          return;
        }
        if (
          response.scanId !== task.scanId ||
          response.queryId !== queryId ||
          response.selectionRevision !==
            response.selection.selectionRevision ||
          !nextSelectionRevision(expectedRevision, response.selectionRevision)
        ) {
          throw new StorageCommandError("REPORT_INCOMPATIBLE");
        }

        commitState((current) => ({
          ...current,
          selection: response.selection,
          pageCache: [],
          error: null,
        }));

        const current = stateRef.current;
        if (current.requestRevision !== operationIntentRevision) {
          return;
        }
        const refreshCursor = current.query.selectedOnly ? null : page.cursor;
        const refreshIndex = current.query.selectedOnly
          ? 0
          : current.currentPageIndex;
        const refreshHistory = current.query.selectedOnly
          ? [null]
          : current.cursorHistory;
        if (current.query.selectedOnly) {
          commitState({
            ...current,
            phase: "loading",
            currentPageIndex: 0,
            cursorHistory: [null],
            page: null,
          });
        }
        await runPageTask({
          kind: "page",
          generation: task.generation,
          intentRevision: operationIntentRevision,
          scanId: task.scanId,
          query: current.query,
          sort: current.sort,
          cursor: refreshCursor,
          pageIndex: refreshIndex,
          cursorHistory: refreshHistory,
          expectedQueryId: current.query.selectedOnly ? null : queryId,
        });
      } catch (error) {
        if (
          !mountedRef.current ||
          task.generation !== generationRef.current ||
          stateRef.current.scanId !== task.scanId
        ) {
          return;
        }
        const code = normalizeStorageError(error).code;
        if (isRecoverableStaleError(code)) {
          recoverThroughFreshQuery();
          return;
        }
        failWith(code);
      }
    },
    [commitState, failWith, recoverThroughFreshQuery, runPageTask],
  );

  useEffect(() => {
    pumpRef.current = pump;
  }, [pump]);

  useEffect(() => {
    executeTaskRef.current = async (task) => {
      if (task.kind === "selection") {
        await runSelectionTask(task);
      } else {
        await runPageTask(task);
      }
    };
  }, [runPageTask, runSelectionTask]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      generationRef.current += 1;
      tasksRef.current = [];
    };
  }, []);

  useEffect(() => {
    generationRef.current += 1;
    intentRevisionRef.current = 0;
    tasksRef.current = [];
    const next = initialState(runtimeAvailable, scanId);
    commitState(next);
    if (runtimeAvailable && scanId !== null) {
      const task = freshQueryTask(next);
      if (task !== null) {
        enqueueTask(task);
      }
    }
  }, [
    commitState,
    enqueueTask,
    freshQueryTask,
    runtimeAvailable,
    scanId,
  ]);

  const recordLocalQueryError = useCallback(() => {
    commitState((current) => ({
      ...current,
      phase: "error",
      error: "RESULT_QUERY_INVALID",
    }));
  }, [commitState]);

  const scheduleIntent = useCallback(
    (query: CandidateQuery, sort: CandidateSort) => {
      const current = stateRef.current;
      if (
        !runtimeAvailable ||
        current.scanId === null ||
        current.phase === "idle"
      ) {
        return;
      }
      const requestRevision = current.requestRevision + 1;
      const revisedQuery = {
        ...query,
        revision: requestRevision.toString(),
      };
      intentRevisionRef.current = requestRevision;
      const next: ResultsWorkspaceState = {
        ...current,
        phase: "loading",
        query: revisedQuery,
        sort,
        requestRevision,
        currentPageIndex: 0,
        cursorHistory: [null],
        pageCache: [],
        page: null,
        selectionPhase:
          current.selectionPhase === "updating" ? "updating" : "idle",
        error: null,
      };
      commitState(next);
      const task = freshQueryTask(next);
      if (task !== null) {
        enqueueTask(task);
      }
    },
    [
      commitState,
      enqueueTask,
      freshQueryTask,
      runtimeAvailable,
    ],
  );

  const submitSearch = useCallback(
    (search: string) => {
      if (!validSearch(search)) {
        recordLocalQueryError();
        return;
      }
      const current = stateRef.current;
      scheduleIntent({ ...current.query, search }, current.sort);
    },
    [recordLocalQueryError, scheduleIntent],
  );

  const toggleExtension = useCallback(
    (extension: string) => {
      const normalized = canonicalExtension(extension);
      if (normalized === null) {
        recordLocalQueryError();
        return;
      }
      const current = stateRef.current;
      const extensions = toggleValue(current.query.extensions, normalized);
      if (extensions.length > MAX_QUERY_EXTENSIONS) {
        recordLocalQueryError();
        return;
      }
      scheduleIntent({ ...current.query, extensions }, current.sort);
    },
    [recordLocalQueryError, scheduleIntent],
  );

  const toggleKind = useCallback(
    (kind: CandidateKind) => {
      const current = stateRef.current;
      scheduleIntent(
        { ...current.query, kinds: toggleValue(current.query.kinds, kind) },
        current.sort,
      );
    },
    [scheduleIntent],
  );

  const toggleMetadataConfidence = useCallback(
    (confidence: MetadataConfidence) => {
      const current = stateRef.current;
      scheduleIntent(
        {
          ...current.query,
          metadataConfidences: toggleValue(
            current.query.metadataConfidences,
            confidence,
          ),
        },
        current.sort,
      );
    },
    [scheduleIntent],
  );

  const toggleMethod = useCallback(
    (method: DiscoveryMethod) => {
      const current = stateRef.current;
      scheduleIntent(
        {
          ...current.query,
          methods: toggleValue(current.query.methods, method),
        },
        current.sort,
      );
    },
    [scheduleIntent],
  );

  const toggleCandidateState = useCallback(
    (candidateState: CandidateState) => {
      const current = stateRef.current;
      scheduleIntent(
        {
          ...current.query,
          states: toggleValue(current.query.states, candidateState),
        },
        current.sort,
      );
    },
    [scheduleIntent],
  );

  const toggleEligibility = useCallback(
    (eligibility: RecoveryEligibility) => {
      const current = stateRef.current;
      scheduleIntent(
        {
          ...current.query,
          eligibilities: toggleValue(
            current.query.eligibilities,
            eligibility,
          ),
        },
        current.sort,
      );
    },
    [scheduleIntent],
  );

  const setScoreRange = useCallback(
    (minimum: number | null, maximum: number | null) => {
      if (
        !validScore(minimum) ||
        !validScore(maximum) ||
        (minimum !== null && maximum !== null && minimum > maximum)
      ) {
        recordLocalQueryError();
        return;
      }
      const current = stateRef.current;
      scheduleIntent(
        {
          ...current.query,
          minRecoverabilityScore: minimum,
          maxRecoverabilityScore: maximum,
        },
        current.sort,
      );
    },
    [recordLocalQueryError, scheduleIntent],
  );

  const setSelectedOnly = useCallback(
    (selectedOnly: boolean) => {
      const current = stateRef.current;
      if (current.query.selectedOnly === selectedOnly) {
        return;
      }
      scheduleIntent({ ...current.query, selectedOnly }, current.sort);
    },
    [scheduleIntent],
  );

  const sortBy = useCallback(
    (field: CandidateSortField) => {
      const current = stateRef.current;
      const direction =
        current.sort.field === field
          ? current.sort.direction === "ascending"
            ? "descending"
            : "ascending"
          : field === "recoverabilityScore"
            ? "descending"
            : "ascending";
      scheduleIntent(current.query, { field, direction });
    },
    [scheduleIntent],
  );

  const activatePage = useCallback(
    (
      cursor: string,
      pageIndex: number,
      cursorHistory: Array<string | null>,
    ) => {
      const current = stateRef.current;
      const cached = current.pageCache.find(
        (page) =>
          page.cursor === cursor &&
          page.queryRevision === current.query.revision,
      );
      if (cached !== undefined) {
        commitState({
          ...current,
          phase: "ready",
          currentPageIndex: pageIndex,
          cursorHistory,
          pageCache: touchPage(current.pageCache, cached),
          page: cached,
          error: null,
        });
        return;
      }
      const queryId = current.page?.queryId;
      if (current.scanId === null || queryId === undefined) {
        return;
      }
      commitState({
        ...current,
        phase: "loading",
        error: null,
      });
      enqueueTask({
        kind: "page",
        generation: generationRef.current,
        intentRevision: current.requestRevision,
        scanId: current.scanId,
        query: current.query,
        sort: current.sort,
        cursor,
        pageIndex,
        cursorHistory,
        expectedQueryId: queryId,
      });
    },
    [commitState, enqueueTask],
  );

  const nextPage = useCallback(() => {
    const current = stateRef.current;
    const cursor = current.page?.nextCursor;
    if (
      current.phase !== "ready" ||
      current.selectionPhase !== "idle" ||
      cursor === null ||
      cursor === undefined
    ) {
      return;
    }
    const nextIndex = current.currentPageIndex + 1;
    const cursorHistory = current.cursorHistory.slice(0, nextIndex);
    cursorHistory[nextIndex] = cursor;
    activatePage(cursor, nextIndex, cursorHistory);
  }, [activatePage]);

  const previousPage = useCallback(() => {
    const current = stateRef.current;
    if (
      current.phase !== "ready" ||
      current.selectionPhase !== "idle" ||
      current.currentPageIndex === 0
    ) {
      return;
    }
    const previousIndex = current.currentPageIndex - 1;
    const cursor = current.cursorHistory[previousIndex];
    if (cursor === undefined || cursor === null) {
      if (cursor === null && previousIndex === 0) {
        const cached = current.pageCache.find(
          (page) =>
            page.cursor === null &&
            page.queryRevision === current.query.revision,
        );
        if (cached !== undefined) {
          commitState({
            ...current,
            currentPageIndex: 0,
            pageCache: touchPage(current.pageCache, cached),
            page: cached,
          });
          return;
        }
        const queryId = current.page?.queryId;
        if (current.scanId !== null && queryId !== undefined) {
          commitState({ ...current, phase: "loading", error: null });
          enqueueTask({
            kind: "page",
            generation: generationRef.current,
            intentRevision: current.requestRevision,
            scanId: current.scanId,
            query: current.query,
            sort: current.sort,
            cursor: null,
            pageIndex: 0,
            cursorHistory: current.cursorHistory,
            expectedQueryId: queryId,
          });
        }
      }
      return;
    }
    activatePage(cursor, previousIndex, current.cursorHistory);
  }, [activatePage, commitState, enqueueTask]);

  const retry = useCallback(() => {
    const current = stateRef.current;
    if (!runtimeAvailable || current.scanId === null) {
      return;
    }
    scheduleIntent(current.query, current.sort);
  }, [runtimeAvailable, scheduleIntent]);

  const enqueueSelection = useCallback(
    (operation: CandidateSelectionOperation) => {
      const current = stateRef.current;
      if (
        current.scanId === null ||
        current.page === null ||
        current.selection === null ||
        current.phase === "loading" ||
        current.selectionPhase === "updating"
      ) {
        return;
      }
      commitState({
        ...current,
        selectionPhase: "updating",
        error: null,
      });
      enqueueTask({
        kind: "selection",
        generation: generationRef.current,
        scanId: current.scanId,
        operation,
      });
    },
    [commitState, enqueueTask],
  );

  const setCandidateSelected = useCallback(
    (candidateId: string, selected: boolean) => {
      if (!validCandidateId(candidateId)) {
        commitState((current) => ({
          ...current,
          phase: "error",
          error: "RESULT_SELECTION_INVALID",
        }));
        return;
      }
      enqueueSelection({
        type: "setIds",
        candidateIds: [candidateId],
        selected,
      });
    },
    [commitState, enqueueSelection],
  );

  const selectAllMatching = useCallback(
    () => enqueueSelection({ type: "selectAllMatching" }),
    [enqueueSelection],
  );
  const clearMatching = useCallback(
    () => enqueueSelection({ type: "clearMatching" }),
    [enqueueSelection],
  );
  const clearAll = useCallback(
    () => enqueueSelection({ type: "clearAll" }),
    [enqueueSelection],
  );

  return {
    state,
    submitSearch,
    toggleExtension,
    toggleKind,
    toggleMetadataConfidence,
    toggleMethod,
    toggleCandidateState,
    toggleEligibility,
    setScoreRange,
    setSelectedOnly,
    sortBy,
    nextPage,
    previousPage,
    retry,
    setCandidateSelected,
    selectAllMatching,
    clearMatching,
    clearAll,
  };
}
