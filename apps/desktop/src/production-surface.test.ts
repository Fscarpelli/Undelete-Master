import { describe, expect, it } from "vitest";

const productionModules = import.meta.glob(
  [
    "./App.tsx",
    "./main.tsx",
    "./api/*.ts",
    "./components/*.tsx",
    "./i18n/*.ts",
    "./state/*.ts",
    "./views/*.tsx",
    "!./**/*.test.ts",
    "!./**/*.test.tsx",
  ],
  {
    eager: true,
    import: "default",
    query: "?raw",
  },
) as Record<string, string>;

describe("production surface inventory", () => {
  it("DESKTOP-UNSUPPORTED-ABSENT-001 contains no fabricated provider, timer result, or unsupported route", () => {
    const paths = Object.keys(productionModules).join("\n");
    const source = Object.values(productionModules).join("\n");
    const prohibitedSourceTokens = [
      "Mock" + "DataProvider",
      "set" + "Timeout(",
      'screen: "restore"',
      'screen: "sessions"',
      'screen: "preview"',
      'invoke("restore',
      'invoke("open_physical',
      'invoke("start_carving',
    ];
    const prohibitedPathTokens = [
      "Restore" + "View",
      "Sessions" + "View",
      "Results" + "View",
      "LiveScan" + "View",
      "/mock" + ".ts",
    ];

    for (const token of prohibitedSourceTokens) {
      expect(source).not.toContain(token);
    }
    for (const token of prohibitedPathTokens) {
      expect(paths).not.toContain(token);
    }
    expect(
      source.match(/invoke<unknown>\("select_and_scan_image"/gu) ?? [],
    ).toHaveLength(1);
  });
});
