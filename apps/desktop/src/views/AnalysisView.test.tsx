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
    schemaVersion: 1,
    scanId: "scan-1",
    sourceLabel: "Evidence (E:)",
    scope: { kind: "folder", label: "Evidence" },
    fileSystem: "ntfs",
    scanStatus: "partial",
    totalCandidates: "5",
    matchedCandidates: "2",
    unknownCandidates: "3",
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
});
