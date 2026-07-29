import { describe, expect, it } from "vitest";
import { formatBytes, formatInteger } from "../format";
import {
  parseDesktopScanReport,
  ReportContractError,
} from "./report";

const validReport = {
  schemaVersion: 2,
  source: {
    label: "boundary.raw",
    sizeBytes: "18446744073709551615",
  },
  partitionTable: "mbr",
  volumes: [
    {
      index: 4294967295,
      offsetBytes: "9007199254740991",
      lengthBytes: "9007199254740992",
      fileSystem: "fat32",
      scanStatus: "complete",
      candidateCount: "18446744073709551615",
      warnings: ["One real parser warning."],
    },
  ],
  warnings: [],
  warningCount: "2",
  warningsOmitted: "1",
};

describe("DesktopScanReport boundary", () => {
  it("DESKTOP-U64-IPC-001 accepts canonical u64 strings without converting through Number", () => {
    const parsed = parseDesktopScanReport(validReport);

    expect(parsed.source.sizeBytes).toBe("18446744073709551615");
    expect(formatInteger(parsed.source.sizeBytes, "en-US")).toBe(
      "18,446,744,073,709,551,615",
    );
    expect(formatInteger(parsed.volumes[0]!.offsetBytes, "en-US")).toBe(
      "9,007,199,254,740,991",
    );
    expect(formatInteger(parsed.volumes[0]!.lengthBytes, "en-US")).toBe(
      "9,007,199,254,740,992",
    );
    expect(formatBytes("1024", "en-US")).toBe("1 KiB");
  });

  it.each([
    [{ ...validReport, schemaVersion: 1 }],
    [
      {
        ...validReport,
        source: { label: String.raw`C:\secret\boundary.raw`, sizeBytes: "1" },
      },
    ],
    [
      {
        ...validReport,
        source: { label: "boundary.raw", sizeBytes: 1 },
      },
    ],
    [
      {
        ...validReport,
        source: { label: "boundary.raw", sizeBytes: "01" },
      },
    ],
    [
      {
        ...validReport,
        source: {
          label: "boundary.raw",
          sizeBytes: "18446744073709551616",
        },
      },
    ],
    [
      {
        ...validReport,
        volumes: [
          {
            ...validReport.volumes[0],
            index: 4294967296,
          },
        ],
      },
    ],
    [{ ...validReport, warningCount: "1", warningsOmitted: "1" }],
    [
      {
        ...validReport,
        volumes: [
          {
            ...validReport.volumes[0],
            fileSystem: "unrecognized",
            scanStatus: "complete",
          },
        ],
      },
    ],
  ])("DESKTOP-REPORT-SCHEMA-001 rejects incompatible or imprecise report payloads", (payload) => {
    expect(() => parseDesktopScanReport(payload)).toThrow(ReportContractError);
  });

  it("DESKTOP-FAT-PARTIAL-001 accepts an explicitly partial FAT scan", () => {
    const parsed = parseDesktopScanReport({
      ...validReport,
      volumes: [
        {
          ...validReport.volumes[0],
          scanStatus: "partial",
          warnings: ["directory chain incomplete"],
        },
      ],
    });

    expect(parsed.volumes[0]?.scanStatus).toBe("partial");
  });
});
