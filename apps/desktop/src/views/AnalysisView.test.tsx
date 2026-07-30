import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { MessageKey } from "../i18n/messages";
import type { StorageWorkflowState } from "../state/storageScan";
import { AnalysisView } from "./AnalysisView";

const t = (key: MessageKey) => key;
const noOp = async () => undefined;

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
  candidates: [],
  nextCursor: null,
  pagePhase: "idle",
  pageError: null,
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
  candidates: [
    {
      id: "candidate-1",
      displayPath: "Evidence/שלום.txt",
      kind: "file",
      state: "partial",
      sizeBytes: "4096",
      metadataConfidence: "medium",
      recoverabilityScore: 61,
      pathState: "incomplete",
      method: "ntfsMetadata",
      contentSha256: null,
      validator: null,
      warnings: [],
    },
  ],
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
    loadMore: noOp,
    resetScan: vi.fn(),
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
    const region = screen.getByRole("region", {
      name: "analysis.results.table",
    });
    expect(region).toHaveAttribute("tabindex", "0");
    const table = within(region).getByRole("table", {
      name: "analysis.results.table",
    });
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

  it("WIN-DEEP-PROVENANCE-001 shows bounded coverage and candidate evidence", () => {
    const hash =
      "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    renderAnalysis({
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
      candidates: [
        {
          ...resultState.candidates[0]!,
          displayPath: "carved-0000000000100000.jpg",
          state: "structurallyValid",
          method: "ntfsMetadata",
          contentSha256: hash,
          validator: "jpeg-structural-v1",
        },
      ],
    });

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
    expect(screen.getByText("candidate.method.ntfsMetadata")).toBeTruthy();
    expect(
      screen.getByText("candidate.method.jpegCorroborated"),
    ).toBeTruthy();
    expect(screen.getByText(hash)).toBeTruthy();
    expect(screen.getByText("jpeg-structural-v1")).toBeTruthy();
  });

  it("WIN-DEEP-ZERO-001 keeps a zero bounded deep result non-exhaustive", () => {
    renderAnalysis({
      ...resultState,
      folderSelection: null,
      candidates: [],
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
    });

    expect(
      screen.getByText("analysis.results.noCandidatesDeepPartial"),
    ).toBeTruthy();
  });

  it("WIN-ZERO-PARTIAL-001 states that a zero partial metadata result is not exhaustive", () => {
    renderAnalysis({
      ...resultState,
      candidates: [],
      summary: {
        ...resultState.summary!,
        totalCandidates: "0",
        matchedCandidates: "0",
        unknownCandidates: "0",
      },
    });

    expect(
      screen.getByText("analysis.results.noCandidatesPartial"),
    ).toBeTruthy();
  });

  it("WIN-ZERO-COMPLETE-001 never equates zero metadata candidates with zero recoverable bytes", () => {
    renderAnalysis({
      ...resultState,
      candidates: [],
      summary: {
        ...resultState.summary!,
        scanStatus: "complete",
        totalCandidates: "0",
        matchedCandidates: "0",
        unknownCandidates: "0",
      },
    });

    expect(
      screen.getByText("analysis.results.noCandidatesComplete"),
    ).toBeTruthy();
  });

  it("WIN-FIRST-PAGE-ERROR-001 gives the page failure precedence over a zero-result message", () => {
    renderAnalysis({
      ...resultState,
      candidates: [],
      pagePhase: "error",
      pageError: "REPORT_INCOMPATIBLE",
    });

    expect(screen.getByRole("alert")).toHaveTextContent(
      "error.REPORT_INCOMPATIBLE",
    );
    expect(
      screen.queryByText("analysis.results.noCandidates"),
    ).toBeNull();
  });
});
