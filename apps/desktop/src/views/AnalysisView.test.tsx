import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { DesktopScanReport } from "../api/report";
import type { MessageKey } from "../i18n/messages";
import { AnalysisView } from "./AnalysisView";

const report: DesktopScanReport = {
  schemaVersion: 2,
  source: {
    label: "invoice-cod.exe",
    sizeBytes: "4096",
  },
  partitionTable: "gpt",
  volumes: [],
  warnings: [],
  warningCount: "0",
  warningsOmitted: "0",
};

const partialReport: DesktopScanReport = {
  ...report,
  volumes: [
    {
      index: 0,
      offsetBytes: "0",
      lengthBytes: "4096",
      fileSystem: "ntfs",
      scanStatus: "partial",
      candidateCount: "2",
      warnings: ["MFT scan stopped at the configured work budget."],
    },
  ],
  warningCount: "1",
};

describe("analysis source label isolation", () => {
  it("DESKTOP-BIDI-ISOLATE-001 uses semantic auto-direction isolation in the heading", () => {
    render(
      <AnalysisView
        runtimeAvailable
        locale="en-US"
        t={(key: MessageKey) => key}
        scanState={{ phase: "success", report }}
        startScan={async () => undefined}
      />,
    );

    const heading = screen.getByRole("heading", {
      level: 1,
      name: report.source.label,
    });
    const isolation = heading.querySelector("bdi");

    expect(isolation).not.toBeNull();
    expect(isolation).toHaveAttribute("dir", "auto");
    expect(isolation).toHaveTextContent(report.source.label);
  });

  it("DESKTOP-PARTIAL-SCAN-STATUS-001 exposes bounded NTFS coverage explicitly", () => {
    render(
      <AnalysisView
        runtimeAvailable
        locale="en-US"
        t={(key: MessageKey) => key}
        scanState={{ phase: "success", report: partialReport }}
        startScan={async () => undefined}
      />,
    );

    expect(screen.getByText("scanStatus.partial")).toBeTruthy();
    expect(screen.getByText("analysis.report.partialCaveat")).toBeTruthy();
    expect(
      screen.getByText("MFT scan stopped at the configured work budget."),
    ).toBeTruthy();
  });

  it("DESKTOP-VOLUME-TABLE-A11Y-001 gives the scroll region and table an accessible name", () => {
    render(
      <AnalysisView
        runtimeAvailable
        locale="en-US"
        t={(key: MessageKey) => key}
        scanState={{ phase: "success", report: partialReport }}
        startScan={async () => undefined}
      />,
    );

    expect(
      screen.getByRole("region", { name: "analysis.report.volumeTable" }),
    ).toHaveAttribute("tabindex", "0");
    expect(
      screen.getByRole("table", { name: "analysis.report.volumeTable" }),
    ).toBeTruthy();
  });
});
