import { fireEvent, render, screen, within } from "@testing-library/react";
import { createRef } from "react";
import { describe, expect, it, vi } from "vitest";
import type {
  ActionableCandidateRow,
  CandidateQueryPage,
  CandidateSelectionSummary,
} from "../api/storage";
import type { MessageKey } from "../i18n/messages";
import type { RestoreWorkflowController } from "../state/restoreWorkflow";
import type { ResultsWorkspaceController } from "../state/resultsWorkspace";
import type { StorageWorkflowState } from "../state/storageScan";
import { AnalysisView } from "./AnalysisView";

const t = (key: MessageKey) => key;
const noOp = async () => undefined;

const selection: CandidateSelectionSummary = {
  selectionRevision: "0",
  selectedCandidates: "0",
  selectedFiles: "0",
  selectedDirectories: "0",
  selectedLogicalBytes: "0",
  bestEffortCandidates: "0",
  conflictedCandidates: "0",
  ineligibleCandidates: "0",
  matchingSelectedCandidates: "0",
};

const actionableCandidate: ActionableCandidateRow = {
  id: "candidate-1",
  displayPath: "Evidence/שלום.txt",
  extension: "txt",
  kind: "file",
  state: "partial",
  sizeBytes: "4096",
  metadataConfidence: "medium",
  recoverabilityScore: 61,
  pathState: "incomplete",
  method: "ntfsMetadata",
  eligibility: "bestEffort",
  selected: false,
  warnings: [],
};

function candidatePage(
  overrides: Partial<CandidateQueryPage> = {},
): CandidateQueryPage {
  return {
    schemaVersion: 1,
    scanId: "scan-1",
    queryId: "query-1",
    queryRevision: "0",
    cursor: null,
    nextCursor: null,
    filteredTotal: "1",
    extensionFacets: [{ extension: "txt", count: "1" }],
    selection,
    candidates: [actionableCandidate],
    ...overrides,
  };
}

function resultsController(
  overrides: Partial<ResultsWorkspaceController["state"]> = {},
): ResultsWorkspaceController {
  const page = candidatePage();
  return {
    state: {
      phase: "ready",
      scanId: "scan-1",
      query: {
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
      },
      sort: {
        field: "recoverabilityScore",
        direction: "descending",
      },
      requestRevision: 0,
      currentPageIndex: 0,
      cursorHistory: [null],
      pageCache: [],
      page,
      selection,
      extensionFacets: page.extensionFacets,
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
      selectionRevision: "0",
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

const inventoryState: StorageWorkflowState = {
  inventoryPhase: "ready",
  inventory: {
    schemaVersion: 1,
    generation: "generation-1",
    disks: [
      {
        id: "disk-1",
        displayName: "External SSD",
        busType: "usb",
        sizeBytes: "1000000",
        volumes: [
          {
            id: "volume-e",
            mountLabel: "E:",
            label: "Evidence",
            fileSystem: "ntfs",
            sizeBytes: "900000",
            freeBytes: "400000",
            isSystem: false,
            scanSupported: true,
            folderScopeSupported: true,
            warnings: [],
          },
        ],
      },
    ],
  },
  inventoryError: null,
  selectedVolumeId: null,
  folderSelection: null,
  scanMode: "metadata",
  folderPhase: "idle",
  folderCancelled: false,
  scanPhase: "idle",
  scanError: null,
  summary: null,
};

const resultState: StorageWorkflowState = {
  ...inventoryState,
  selectedVolumeId: "volume-e",
  folderSelection: {
    schemaVersion: 1,
    scopeId: "scope-1",
    volumeId: "volume-e",
    label: "Evidence",
  },
  scanPhase: "success",
  summary: {
    schemaVersion: 3,
    scanId: "scan-1",
    sourceLabel: "Evidence (E:)",
    scope: { kind: "folder", label: "Evidence" },
    scanMode: "metadata",
    fileSystem: "ntfs",
    scanStatus: "partial",
    totalCandidates: "5",
    matchedCandidates: "2",
    unknownCandidates: "3",
    mftCoverage: {
      recordsDeclared: "120000",
      recordsAvailable: "120000",
      recordsExamined: "65536",
      bytesDeclared: "122880000",
      bytesAvailable: "122880000",
      bytesExamined: "67108864",
    },
    jpegCarveCoverage: null,
    warnings: ["The source changed during the scan."],
  },
};

function renderAnalysis(
  state: StorageWorkflowState,
  overrides: Partial<React.ComponentProps<typeof AnalysisView>> = {},
) {
  const props: React.ComponentProps<typeof AnalysisView> = {
    runtimeAvailable: true,
    locale: "en-US",
    t,
    state,
    refreshInventory: noOp,
    selectVolume: vi.fn(),
    selectFolder: noOp,
    clearFolder: vi.fn(),
    selectScanMode: vi.fn(),
    startScan: noOp,
    resetScan: vi.fn(),
    results: resultsController(),
    restore: restoreController(),
    recoverButtonRef: createRef<HTMLButtonElement>(),
    ...overrides,
  };
  render(<AnalysisView {...props} />);
  return props;
}

describe("connected-storage analysis view", () => {
  it("WIN-VOLUME-RADIO-A11Y-001 gives each real volume a labelled radio", () => {
    const selectVolume = vi.fn();
    renderAnalysis(inventoryState, { selectVolume });

    const radio = screen.getByRole("radio", {
      name: /Evidence.*E:/u,
    });
    fireEvent.click(radio);

    expect(selectVolume).toHaveBeenCalledWith("volume-e");
    expect(screen.getByText("bus.usb")).toBeTruthy();
    expect(screen.getByText("NTFS")).toBeTruthy();
  });

  it("WIN-DISK-PRIVACY-001 renders only the sanitized disk display name", () => {
    renderAnalysis(inventoryState);

    expect(
      screen.getByRole("heading", { name: "External SSD" }),
    ).toBeTruthy();
  });

  it("WIN-CANDIDATE-TABLE-A11Y-001 names the table region and isolates recovered paths", () => {
    renderAnalysis(resultState);

    expect(screen.getByText("analysis.results.caveat")).toBeTruthy();
    const table = screen.getByRole("table", {
      name: "analysis.results.table",
    });
    const region = table.closest(".results-table-scroll");
    expect(region).not.toBeNull();
    expect(region).toHaveAttribute("tabindex", "0");
    const path = within(table).getByText("Evidence/שלום.txt");
    expect(path.closest("bdi")).toHaveAttribute("dir", "auto");
  });

  it("WIN-PARTIAL-UNKNOWN-001 keeps partial coverage and unknown ancestry explicit", () => {
    renderAnalysis(resultState);

    expect(screen.getByText("analysis.results.partialTitle")).toBeTruthy();
    expect(screen.getByText("analysis.results.unknownTitle")).toBeTruthy();
    expect(
      screen.getByText("The source changed during the scan."),
    ).toBeTruthy();
  });

  it("WIN-MFT-COVERAGE-001 shows examined records against the declared MFT total", () => {
    renderAnalysis(resultState);

    expect(screen.getByText("65,536 / 120,000")).toBeTruthy();
    expect(
      screen.getByText("analysis.results.mftRecordsExamined"),
    ).toBeTruthy();
  });

  it("WIN-DEEP-MODE-001 offers an explicit deep JPEG mode for a whole NTFS volume", () => {
    const selectScanMode = vi.fn();
    renderAnalysis(
      {
        ...inventoryState,
        selectedVolumeId: "volume-e",
      },
      { selectScanMode },
    );

    const deepMode = screen.getByRole("radio", {
      name: /analysis\.mode\.deepJpeg\.title/u,
    });
    expect(deepMode).toBeEnabled();
    fireEvent.click(deepMode);
    expect(selectScanMode).toHaveBeenCalledWith("deepJpeg");
    expect(screen.getByText("analysis.mode.deepJpeg.body")).toBeTruthy();
  });

  it("WIN-DEEP-MODE-002 disables deep JPEG for a folder scope", () => {
    renderAnalysis({
      ...inventoryState,
      selectedVolumeId: "volume-e",
      folderSelection: resultState.folderSelection,
    });

    expect(
      screen.getByRole("radio", {
        name: /analysis\.mode\.deepJpeg\.title/u,
      }),
    ).toBeDisabled();
    expect(screen.getByText("analysis.mode.folderBlocked")).toBeTruthy();
  });

  it("WIN-DEEP-MODE-003 disables deep JPEG for a non-NTFS volume", () => {
    const inventory = inventoryState.inventory!;
    renderAnalysis({
      ...inventoryState,
      selectedVolumeId: "volume-e",
      inventory: {
        ...inventory,
        disks: inventory.disks.map((disk) => ({
          ...disk,
          volumes: disk.volumes.map((volume) => ({
            ...volume,
            fileSystem: "fat32",
            folderScopeSupported: false,
          })),
        })),
      },
    });

    expect(
      screen.getByRole("radio", {
        name: /analysis\.mode\.deepJpeg\.title/u,
      }),
    ).toBeDisabled();
    expect(screen.getByText("analysis.mode.ntfsBlocked")).toBeTruthy();
  });

  it("WIN-DEEP-PROVENANCE-001 shows bounded coverage and the actionable carving method", () => {
    const deepPage = candidatePage({
      extensionFacets: [{ extension: "jpg", count: "1" }],
      candidates: [
        {
          ...actionableCandidate,
          displayPath: "carved-0000000000100000.jpg",
          extension: "jpg",
          state: "structurallyValid",
          method: "carving",
          eligibility: "complete",
        },
      ],
    });
    renderAnalysis(
      {
        ...resultState,
        folderSelection: null,
        summary: {
          ...resultState.summary!,
          scope: { kind: "volume", label: "Evidence (E:)" },
          scanMode: "deepJpeg",
          jpegCarveCoverage: {
            bytesRequested: "1048576",
            bytesScanned: "524288",
            signaturesAttempted: "1000",
            validationBytesRead: "262144",
            partial: true,
            readErrorCount: "0",
            candidateLimitReached: false,
            candidateByteLimitHits: "0",
            signatureAttemptLimitReached: true,
            validationByteLimitReached: false,
            rejectedSignatures: "4",
            truncatedSignatures: "1",
            regionsSubmitted: "2",
            regionLimitReached: false,
          },
        },
      },
      {
        results: resultsController({
          page: deepPage,
          extensionFacets: deepPage.extensionFacets,
        }),
      },
    );

    expect(screen.getByText("512 KiB / 1 MiB")).toBeTruthy();
    expect(
      screen.getByText(/analysis\.results\.jpegCoveragePartial/u),
    ).toBeTruthy();
    expect(
      screen.getByText(
        /analysis\.results\.jpegSignaturesAttempted.*1,000/u,
      ),
    ).toBeTruthy();
    expect(
      screen.getByText(
        /analysis\.results\.jpegSignatureLimit.*analysis\.results\.limitReached/u,
      ),
    ).toBeTruthy();
    const table = screen.getByRole("table", {
      name: "analysis.results.table",
    });
    expect(within(table).getByText("candidate.method.carving")).toBeTruthy();
    expect(
      within(table).getByText("carved-0000000000100000.jpg"),
    ).toBeTruthy();
  });

  it("WIN-DEEP-ZERO-001 keeps a zero bounded deep result non-exhaustive", () => {
    const emptyPage = candidatePage({
      filteredTotal: "0",
      extensionFacets: [],
      candidates: [],
    });
    renderAnalysis(
      {
        ...resultState,
        folderSelection: null,
        summary: {
          ...resultState.summary!,
          scope: { kind: "volume", label: "Evidence (E:)" },
          scanMode: "deepJpeg",
          totalCandidates: "0",
          matchedCandidates: "0",
          unknownCandidates: "0",
          jpegCarveCoverage: {
            bytesRequested: "1048576",
            bytesScanned: "524288",
            signaturesAttempted: "8",
            validationBytesRead: "4096",
            partial: true,
            readErrorCount: "0",
            candidateLimitReached: false,
            candidateByteLimitHits: "0",
            signatureAttemptLimitReached: false,
            validationByteLimitReached: false,
            rejectedSignatures: "4",
            truncatedSignatures: "1",
            regionsSubmitted: "2",
            regionLimitReached: false,
          },
        },
      },
      {
        results: resultsController({
          page: emptyPage,
          extensionFacets: [],
        }),
      },
    );

    expect(
      screen.getByText("analysis.results.noCandidatesDeepPartial"),
    ).toBeTruthy();
  });

  it("WIN-ZERO-PARTIAL-001 states that a zero partial metadata result is not exhaustive", () => {
    const emptyPage = candidatePage({
      filteredTotal: "0",
      extensionFacets: [],
      candidates: [],
    });
    renderAnalysis(
      {
        ...resultState,
        summary: {
          ...resultState.summary!,
          totalCandidates: "0",
          matchedCandidates: "0",
          unknownCandidates: "0",
        },
      },
      {
        results: resultsController({
          page: emptyPage,
          extensionFacets: [],
        }),
      },
    );

    expect(
      screen.getByText("analysis.results.noCandidatesPartial"),
    ).toBeTruthy();
  });

  it("WIN-ZERO-COMPLETE-001 never equates zero metadata candidates with zero recoverable bytes", () => {
    const emptyPage = candidatePage({
      filteredTotal: "0",
      extensionFacets: [],
      candidates: [],
    });
    renderAnalysis(
      {
        ...resultState,
        summary: {
          ...resultState.summary!,
          scanStatus: "complete",
          totalCandidates: "0",
          matchedCandidates: "0",
          unknownCandidates: "0",
        },
      },
      {
        results: resultsController({
          page: emptyPage,
          extensionFacets: [],
        }),
      },
    );

    expect(
      screen.getByText("analysis.results.noCandidatesComplete"),
    ).toBeTruthy();
  });

  it("WIN-FIRST-PAGE-ERROR-001 gives the page failure precedence over a zero-result message", () => {
    renderAnalysis(resultState, {
      results: resultsController({
        phase: "error",
        page: null,
        extensionFacets: [],
        error: "REPORT_INCOMPATIBLE",
      }),
    });

    expect(screen.getByRole("alert")).toHaveTextContent(
      "error.REPORT_INCOMPATIBLE",
    );
    expect(
      screen.queryByText("analysis.results.noCandidates"),
    ).toBeNull();
  });
});
