import { createRef } from "react";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type {
  ActionableCandidateRow,
  CandidateQueryPage,
  CandidateSelectionSummary,
  ScanSummary,
} from "../../api/storage";
import type { ResultsWorkspaceController } from "../../state/resultsWorkspace";
import type { RestoreWorkflowController } from "../../state/restoreWorkflow";
import { CandidateResultsTable } from "./CandidateResultsTable";
import { ResultsWorkspace } from "./ResultsWorkspace";

const selection: CandidateSelectionSummary = {
  selectionRevision: "7",
  selectedCandidates: "2",
  selectedFiles: "2",
  selectedDirectories: "0",
  selectedLogicalBytes: "6144",
  bestEffortCandidates: "1",
  conflictedCandidates: "1",
  ineligibleCandidates: "0",
  matchingSelectedCandidates: "1",
};

const candidates: ActionableCandidateRow[] = [
  {
    id: "candidate-1",
    displayPath: "Documents/invoice.pdf",
    extension: "pdf",
    kind: "file",
    state: "likelyComplete",
    sizeBytes: "2048",
    metadataConfidence: "high",
    recoverabilityScore: 91,
    pathState: "reconstructed",
    method: "ntfsMetadata",
    eligibility: "complete",
    selected: true,
    warnings: [],
  },
  {
    id: "candidate-2",
    displayPath: "Résumé/notes.txt",
    extension: "txt",
    kind: "file",
    state: "conflicted",
    sizeBytes: "4096",
    metadataConfidence: "medium",
    recoverabilityScore: 52,
    pathState: "incomplete",
    method: "ntfsMetadata",
    eligibility: "bestEffort",
    selected: false,
    warnings: ["Bounded conflicting ranges were observed."],
  },
];

function page(
  rows: ActionableCandidateRow[] = candidates,
): CandidateQueryPage {
  return {
    schemaVersion: 1,
    scanId: "scan-1",
    queryId: "query-1",
    queryRevision: "1",
    cursor: null,
    nextCursor: "cursor-2",
    filteredTotal: "201",
    extensionFacets: [
      { extension: "pdf", count: "120" },
      { extension: "txt", count: "81" },
    ],
    selection,
    candidates: rows,
  };
}

function resultsController(
  overrides: Partial<ResultsWorkspaceController["state"]> = {},
): ResultsWorkspaceController {
  const nativePage = page();
  return {
    state: {
      phase: "ready",
      scanId: "scan-1",
      query: {
        revision: "1",
        search: "",
        extensions: ["pdf"],
        kinds: [],
        metadataConfidences: [],
        methods: [],
        states: [],
        minRecoverabilityScore: null,
        maxRecoverabilityScore: null,
        eligibilities: [],
        selectedOnly: false,
      },
      sort: {
        field: "recoverabilityScore",
        direction: "descending",
      },
      requestRevision: 1,
      currentPageIndex: 0,
      cursorHistory: [null],
      pageCache: [],
      page: nativePage,
      selection,
      extensionFacets: nativePage.extensionFacets,
      selectionPhase: "idle",
      error: null,
      ...overrides,
    },
    submitSearch: vi.fn(),
    toggleExtension: vi.fn(),
    toggleKind: vi.fn(),
    toggleMetadataConfidence: vi.fn(),
    toggleMethod: vi.fn(),
    toggleCandidateState: vi.fn(),
    toggleEligibility: vi.fn(),
    setScoreRange: vi.fn(),
    setSelectedOnly: vi.fn(),
    sortBy: vi.fn(),
    nextPage: vi.fn(),
    previousPage: vi.fn(),
    retry: vi.fn(),
    setCandidateSelected: vi.fn(),
    selectAllMatching: vi.fn(),
    clearMatching: vi.fn(),
    clearAll: vi.fn(),
  };
}

function restoreController(): RestoreWorkflowController {
  return {
    state: {
      phase: "idle",
      scanId: "scan-1",
      selectionRevision: "7",
      destination: null,
      collisionPolicy: "rename",
      partialFilePolicy: "completeOnly",
      bestEffortConsent: false,
      plan: null,
      job: null,
      operationPhase: "idle",
      progressBasisPoints: 0,
      cancelRequested: false,
      error: null,
    },
    chooseDestination: vi.fn(),
    setPartialFilePolicy: vi.fn(),
    setBestEffortConsent: vi.fn(),
    createPlan: vi.fn(),
    start: vi.fn(),
    cancel: vi.fn(),
    openDestination: vi.fn(),
    close: vi.fn(),
  };
}

const summary: ScanSummary = {
  schemaVersion: 3,
  scanId: "scan-1",
  sourceLabel: "Archive",
  scope: { kind: "volume", label: "Archive" },
  scanMode: "metadata",
  fileSystem: "ntfs",
  scanStatus: "complete",
  totalCandidates: "201",
  matchedCandidates: "201",
  unknownCandidates: "0",
  mftCoverage: null,
  jpegCarveCoverage: null,
  warnings: [],
};

const t = (key: string) => key;

describe("actionable results workspace", () => {
  it("DESKTOP-RESULT-CONTROLS-022 submits search explicitly and renders only native extension facets", () => {
    const controller = resultsController();

    render(
      <ResultsWorkspace
        summary={summary}
        controller={controller}
        restore={restoreController()}
        locale="en-US"
        t={t}
        recoverButtonRef={createRef<HTMLButtonElement>()}
      />,
    );

    const search = screen.getByRole("search");
    const query = within(search).getByRole("searchbox", {
      name: "results.search.label",
    });
    fireEvent.change(query, { target: { value: "invoice" } });
    expect(controller.submitSearch).not.toHaveBeenCalled();
    fireEvent.submit(search);
    expect(controller.submitSearch).toHaveBeenCalledOnce();
    expect(controller.submitSearch).toHaveBeenCalledWith("invoice");

    expect(screen.getByLabelText("results.filters.extensionSearch")).toHaveAttribute(
      "maxlength",
      "128",
    );
    expect(
      screen.getByRole("checkbox", { name: /pdf.*120/u }),
    ).toBeChecked();
    expect(screen.getByRole("checkbox", { name: /txt.*81/u })).toBeTruthy();
    expect(screen.queryByText("zip")).toBeNull();
  });

  it("DESKTOP-RESULT-CONTROLS-022 keeps selected extension chips removable at the native limit", () => {
    const controller = resultsController();
    controller.state.query = {
      ...controller.state.query,
      extensions: Array.from(
        { length: 128 },
        (_, index) => `ext-${index.toString()}`,
      ),
    };
    controller.state.extensionFacets = [
      { extension: "ext-0", count: "1" },
      { extension: "not-selected", count: "2" },
    ];

    render(
      <ResultsWorkspace
        summary={summary}
        controller={controller}
        restore={restoreController()}
        locale="en-US"
        t={t}
        recoverButtonRef={createRef<HTMLButtonElement>()}
      />,
    );

    expect(
      screen.getByRole("checkbox", { name: /not-selected.*2/u }),
    ).toBeDisabled();
    expect(
      screen.getByRole("button", {
        name: /results\.filters\.removeExtension: ext-0/u,
      }),
    ).toBeEnabled();
  });

  it("DESKTOP-RESULT-CONTROLS-022 DESKTOP-RESTORE-A11Y-025 exposes native totals, semantic sorting, directional isolation, and bounded rows", () => {
    const oversized = Array.from({ length: 101 }, (_, index) => ({
      ...candidates[0]!,
      id: `candidate-${index.toString()}`,
      displayPath: `folder/file-${index.toString()}.pdf`,
    }));
    const controller = resultsController({
      page: page(oversized),
    });
    const summaryRef = createRef<HTMLDivElement>();

    render(
      <CandidateResultsTable
        controller={controller}
        locale="en-US"
        t={t}
        summaryRef={summaryRef}
      />,
    );

    const table = screen.getByRole("table", {
      name: "analysis.results.table",
    });
    expect(table).toHaveAttribute("aria-rowcount", "201");
    expect(within(table).getAllByRole("row")).toHaveLength(101);
    expect(
      screen.getByRole("columnheader", { name: "analysis.results.score" }),
    ).toHaveAttribute("aria-sort", "descending");
    fireEvent.click(
      within(
        screen.getByRole("columnheader", {
          name: "analysis.results.path",
        }),
      ).getByRole("button"),
    );
    expect(controller.sortBy).toHaveBeenCalledWith("path");
    expect(
      within(table).getByText("folder/file-0.pdf").closest("bdi"),
    ).toHaveAttribute("dir", "auto");
  });

  it("DESKTOP-RESULT-CONTROLS-022 uses one native filtered-set operation and projects row selection", () => {
    const controller = resultsController();
    render(
      <CandidateResultsTable
        controller={controller}
        locale="en-US"
        t={t}
        summaryRef={createRef<HTMLDivElement>()}
      />,
    );

    const filtered = screen.getByRole("checkbox", {
      name: "results.selection.filtered",
    });
    expect(filtered).toHaveAttribute("aria-checked", "mixed");
    fireEvent.click(filtered);
    expect(controller.selectAllMatching).toHaveBeenCalledOnce();
    expect(controller.setCandidateSelected).not.toHaveBeenCalled();

    fireEvent.click(
      screen.getByRole("checkbox", {
        name: /results\.selection\.row.*notes\.txt/u,
      }),
    );
    expect(controller.setCandidateSelected).toHaveBeenCalledWith(
      "candidate-2",
      true,
    );
  });

  it("DESKTOP-RESULT-CONTROLS-022 disables pagination while native selection authority is updating", () => {
    const controller = resultsController({
      currentPageIndex: 1,
      cursorHistory: [null, "cursor-1"],
      page: {
        ...page(),
        cursor: "cursor-1",
        nextCursor: "cursor-2",
      },
      selectionPhase: "updating",
    });

    render(
      <CandidateResultsTable
        controller={controller}
        locale="en-US"
        t={t}
        summaryRef={createRef<HTMLDivElement>()}
      />,
    );

    expect(
      screen.getByRole("button", { name: "results.pagination.previous" }),
    ).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "results.pagination.next" }),
    ).toBeDisabled();
  });

  it("DESKTOP-RESTORE-A11Y-025 moves focus to the results status when a focused row disappears", () => {
    const controller = resultsController();
    const summaryRef = createRef<HTMLDivElement>();
    const rendered = render(
      <>
        <div ref={summaryRef} role="status" tabIndex={-1}>
          results.summary
        </div>
        <CandidateResultsTable
          controller={controller}
          locale="en-US"
          t={t}
          summaryRef={summaryRef}
        />
      </>,
    );
    const checkbox = screen.getByRole("checkbox", {
      name: /results\.selection\.row.*notes\.txt/u,
    });
    checkbox.focus();

    const nextController = resultsController({
      page: page([candidates[0]!]),
    });
    rendered.rerender(
      <>
        <div ref={summaryRef} role="status" tabIndex={-1}>
          results.summary
        </div>
        <CandidateResultsTable
          controller={nextController}
          locale="en-US"
          t={t}
          summaryRef={summaryRef}
        />
      </>,
    );

    expect(summaryRef.current).toHaveFocus();
  });

  it("DESKTOP-RESULT-CONTROLS-022 reports a filtered zero without making a scan-wide empty claim", () => {
    const controller = resultsController({
      page: {
        ...page([]),
        filteredTotal: "0",
        nextCursor: null,
      },
    });

    render(
      <ResultsWorkspace
        summary={summary}
        controller={controller}
        restore={restoreController()}
        locale="en-US"
        t={t}
        recoverButtonRef={createRef<HTMLButtonElement>()}
      />,
    );

    expect(screen.getByText("results.summary.empty")).toBeTruthy();
    expect(
      screen.queryByText("analysis.results.noCandidatesComplete"),
    ).toBeNull();
  });

  it("DESKTOP-RESULT-CONTROLS-022 renders an initial page error without an empty-table status", () => {
    render(
      <ResultsWorkspace
        summary={summary}
        controller={resultsController({
          phase: "error",
          page: null,
          error: "RESULT_QUERY_INVALID",
        })}
        restore={restoreController()}
        locale="en-US"
        t={t}
        recoverButtonRef={createRef<HTMLButtonElement>()}
      />,
    );

    expect(screen.getByRole("alert")).toHaveTextContent(
      "error.RESULT_QUERY_INVALID",
    );
    expect(screen.queryByText("results.summary.empty")).toBeNull();
    expect(screen.queryByText("results.summary.error")).toBeNull();
  });

  it("DESKTOP-RESTORE-A11Y-025 returns row focus to the results status when filtering unmounts the table", () => {
    const recoverButtonRef = createRef<HTMLButtonElement>();
    const rendered = render(
      <ResultsWorkspace
        summary={summary}
        controller={resultsController()}
        restore={restoreController()}
        locale="en-US"
        t={t}
        recoverButtonRef={recoverButtonRef}
      />,
    );
    screen
      .getByRole("checkbox", {
        name: /results\.selection\.row.*notes\.txt/u,
      })
      .focus();

    rendered.rerender(
      <ResultsWorkspace
        summary={summary}
        controller={resultsController({
          page: {
            ...page([]),
            filteredTotal: "0",
            nextCursor: null,
          },
        })}
        restore={restoreController()}
        locale="en-US"
        t={t}
        recoverButtonRef={recoverButtonRef}
      />,
    );

    expect(
      rendered.container.querySelector(".results-summary"),
    ).toHaveFocus();
  });

  it("DESKTOP-RESULT-CONTROLS-022 blocks recovery while the native global summary includes ineligible items", () => {
    const blockedSelection = {
      ...selection,
      ineligibleCandidates: "1",
    };
    const controller = resultsController({
      selection: blockedSelection,
    });
    render(
      <ResultsWorkspace
        summary={summary}
        controller={controller}
        restore={restoreController()}
        locale="en-US"
        t={t}
        recoverButtonRef={createRef<HTMLButtonElement>()}
      />,
    );

    expect(
      screen.getByRole("button", { name: "results.selection.recover" }),
    ).toBeDisabled();
    expect(screen.getByText("results.selection.ineligibleBlocked")).toBeTruthy();
    expect(screen.getByText("6 KiB")).toBeTruthy();
  });
});
