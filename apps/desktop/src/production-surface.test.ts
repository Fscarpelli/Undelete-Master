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
  it("WIN-PRODUCTION-SURFACE-001 contains only the real connected-storage flow", () => {
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
      "/report" + ".ts",
      "/desktop" + ".ts",
    ];

    for (const token of prohibitedSourceTokens) {
      expect(source).not.toContain(token);
    }
    for (const token of prohibitedPathTokens) {
      expect(paths).not.toContain(token);
    }
    for (const command of [
      "list_storage_sources",
      "select_scan_folder",
      "scan_storage_volume",
      "get_candidate_page",
    ]) {
      expect(
        source.match(new RegExp(`invoke<unknown>\\("${command}"`, "gu")) ?? [],
      ).toHaveLength(1);
    }
  });
});
