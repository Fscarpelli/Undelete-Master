import { describe, expect, it } from "vitest";

const productionModules = import.meta.glob(
  [
    "./App.tsx",
    "./main.tsx",
    "./api/**/*.ts",
    "./components/**/*.tsx",
    "./i18n/**/*.ts",
    "./state/**/*.ts",
    "./views/**/*.tsx",
    "!./**/*.test.ts",
    "!./**/*.test.tsx",
  ],
  {
    eager: true,
    import: "default",
    query: "?raw",
  },
) as Record<string, string>;

const storageDesktopSuffix = "/api/storageDesktop.ts";
const expectedCommands = [
  "cancel_restore",
  "create_restore_plan",
  "get_candidate_page",
  "get_restore_job",
  "list_storage_sources",
  "open_restore_destination",
  "query_candidate_page",
  "scan_storage_volume",
  "select_restore_destination",
  "select_scan_folder",
  "start_restore",
  "update_candidate_selection",
];

const invokeCallPattern =
  /\binvoke(?:\s*<[^<>{}\r\n]+>)?\s*\(\s*["']([a-z_]+)["']/gu;
const invokeStartPattern = /\binvoke(?:\s*<[^<>{}\r\n]+>)?\s*\(/gu;
const invokeStartDetection = /\binvoke(?:\s*<[^<>{}\r\n]+>)?\s*\(/u;
const coreApiLoad =
  /(?:\bfrom\s*|\bimport\s*(?:\(\s*)?|\brequire\s*\(\s*)["']@tauri-apps\/api\/core["']/u;
const forbiddenLaunchPackage =
  /(?:\bfrom\s*|\bimport\s*(?:\(\s*)?|\brequire\s*\(\s*)["'](?:@tauri-apps\/(?:plugin-(?:opener|shell)|api\/(?:process|shell))|(?:node:)?child_process)["']/u;
const directBrowserOpen =
  /\b(?:globalThis|parent|self|top|window)\s*(?:\?\.\s*open|\.\s*open|\??\.\s*\[\s*["']open["']\s*\]|\[\s*["']open["']\s*\])/u;
const explicitCallerAuthority =
  /\b(?:sourcePath|destinationPath|filePath|directoryPath|rootPath|workingDirectory|executable|program|commandLine|shellCommand|cwd)\b/u;
const genericPathAuthority =
  /(?:\b(?:const|let|var)\s+path\b|\.\s*path\b|\bpath\s*=>)/u;
const callerAuthorityName =
  "(?:sourcePath|destinationPath|filePath|directoryPath|rootPath|workingDirectory|path|executable|program|commandLine|shellCommand|cwd)";
const callerAuthorityParameter = new RegExp(
  String.raw`(?:\(|,)\s*${callerAuthorityName}\s*(?::|\?|,|\))`,
  "u",
);
const callerAuthorityProperty = new RegExp(
  String.raw`(?:\{|,)\s*${callerAuthorityName}\s*(?::|,|\})`,
  "u",
);

function maskNonCode(source: string): string {
  return source.replace(
    /"(?:\\[\s\S]|[^"\\])*"|'(?:\\[\s\S]|[^'\\])*'/gu,
    (token) =>
      /^(?:["']invoke["']|["']open["'])$/u.test(token)
        ? token
        : token.replace(/[^\r\n]/gu, " "),
  );
}

function normalizedModulePath(path: string): string {
  return path.replaceAll("\\", "/");
}

function invokedCommands(source: string): string[] {
  return [...source.matchAll(invokeCallPattern)]
    .map((match) => match[1]!)
    .sort();
}

function auditStorageDesktopInvokes(source: string): string[] {
  const findings: string[] = [];
  const literalCalls = invokedCommands(source);
  const invokeStarts = [...source.matchAll(invokeStartPattern)].length;
  const invokeIdentifiers = [...source.matchAll(/\binvoke\b/gu)].length;

  if (
    invokeStarts !== literalCalls.length ||
    invokeIdentifiers !== expectedCommands.length + 1
  ) {
    findings.push("indirect-or-dynamic-native-invoke");
  }
  if (
    literalCalls.length !== expectedCommands.length ||
    literalCalls.some(
      (command, index) => command !== expectedCommands[index],
    )
  ) {
    findings.push("native-command-inventory");
  }
  return findings;
}

function auditAdversarialFixture(path: string, source: string): string[] {
  const findings = new Set<string>();
  const normalizedPath = normalizedModulePath(path);
  const isStorageDesktop = normalizedPath.endsWith(storageDesktopSuffix);
  const code = maskNonCode(source);
  const referencesCoreInvoke =
    coreApiLoad.test(source) && /\binvoke\b/u.test(code);
  const invokesByKnownName =
    invokeStartDetection.test(code) ||
    /(?:\.|\[\s*["'])invoke(?:["']\s*\])?\s*(?:<[^>]+>)?\s*\(/u.test(
      code,
    );

  if (!isStorageDesktop && (referencesCoreInvoke || invokesByKnownName)) {
    findings.add("native-invoke-outside-wrapper");
  }
  if (forbiddenLaunchPackage.test(source) || directBrowserOpen.test(code)) {
    findings.add("launch-surface");
  }
  if (
    callerAuthorityParameter.test(code) ||
    callerAuthorityProperty.test(code) ||
    explicitCallerAuthority.test(code) ||
    genericPathAuthority.test(code) ||
    /\b(?:sourcePath|destinationPath|extents|offsets|handle|executable|recoveredBytes)\s*:/u.test(
      code,
    )
  ) {
    findings.add("caller-authority");
  }
  if (isStorageDesktop) {
    for (const finding of auditStorageDesktopInvokes(source)) {
      findings.add(finding);
    }
  }
  return [...findings];
}

describe("production surface inventory", () => {
  it("WIN-PRODUCTION-SURFACE-001 recursively inventories only the real native command flow", () => {
    const paths = Object.keys(productionModules).join("\n");
    const source = Object.values(productionModules).join("\n");
    const prohibitedSourceTokens = [
      "Mock" + "DataProvider",
      "fake" + "Progress",
      "simulated" + "Progress",
      "set" + "Interval(",
      'screen: "restore"',
      'screen: "sessions"',
      'screen: "preview"',
      'invoke("open_physical',
      'invoke("start_carving',
      "style={{",
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
    for (const requiredPath of [
      "/components/results/ResultsWorkspace.tsx",
      "/components/results/RestoreWorkflowDialog.tsx",
      "/state/resultsWorkspace.ts",
      "/state/restoreWorkflow.ts",
    ]) {
      expect(
        Object.keys(productionModules).some((path) =>
          path.replaceAll("\\", "/").endsWith(requiredPath),
        ),
      ).toBe(true);
    }

    for (const [path, moduleSource] of Object.entries(productionModules)) {
      expect(
        auditAdversarialFixture(path, moduleSource),
        normalizedModulePath(path),
      ).toEqual([]);
      if (moduleSource.includes("setTimeout(")) {
        expect(normalizedModulePath(path)).toMatch(
          /\/state\/restoreWorkflow\.ts$/u,
        );
        expect(moduleSource).toContain("getRestoreJob");
      }
    }

    expect(source).not.toMatch(
      /\b(?:sourcePath|destinationPath|extents|offsets|handle|executable|recoveredBytes)\s*:/gu,
    );
  });
});

describe("production surface adversarial validator fixtures", () => {
  it.each([
    [
      "non-generic invoke",
      "./state/escape.ts",
      `import { invoke } from "@tauri-apps/api/core";
       export const escape = () => invoke("erase_scan_source");`,
      "native-invoke-outside-wrapper",
    ],
    [
      "aliased invoke",
      "./views/escape.tsx",
      `import { invoke as nativeCall } from "@tauri-apps/api/core";
       export const escape = () => nativeCall("erase_scan_source");`,
      "native-invoke-outside-wrapper",
    ],
    [
      "namespace invoke",
      "./components/escape.tsx",
      `import * as tauriCore from "@tauri-apps/api/core";
       export const escape = () => tauriCore.invoke("erase_scan_source");`,
      "native-invoke-outside-wrapper",
    ],
    [
      "dynamic core invoke",
      "./views/escape.tsx",
      `export const escape = async () =>
         (await import("@tauri-apps/api/core")).invoke("erase_scan_source");`,
      "native-invoke-outside-wrapper",
    ],
  ])("rejects %s", (_name, path, source, finding) => {
    expect(auditAdversarialFixture(path, source)).toContain(finding);
  });

  it.each([
    [
      "window.open",
      `export const escape = (url: string) => window.open(url);`,
    ],
    [
      "aliased window.open",
      `export const escape = () => {
         const launch = window.open;
         return launch;
       };`,
    ],
    [
      "bracketed global open",
      `export const escape = (url: string) => globalThis["open"](url);`,
    ],
    [
      "Tauri shell command",
      `import { Command } from "@tauri-apps/plugin-shell";
       export const escape = (program: string) => Command.create(program);`,
    ],
    [
      "Tauri opener alias",
      `import { openPath as launch } from "@tauri-apps/plugin-opener";
       export const escape = (path: string) => launch(path);`,
    ],
  ])("rejects the %s launch surface", (_name, source) => {
    expect(auditAdversarialFixture("./views/escape.tsx", source)).toContain(
      "launch-surface",
    );
  });

  it.each([
    `export const escape = (sourcePath: string) => ({ sourcePath });`,
    `export const escape = (path: string) => ({ path });`,
    `export const escape = (executable: string) => ({ executable });`,
    `export const escape = (program: string) => ({ program });`,
    `export const escape = (input: string) => {
       const path = input;
       return path;
     };`,
    `export const escape = (input: string) => {
       const executable = input;
       return executable;
     };`,
    'export const escape = (input: string) => `${(() => { const path = input; return path; })()}`;',
  ])("rejects caller-supplied path or executable authority", (source) => {
    expect(auditAdversarialFixture("./api/escape.ts", source)).toContain(
      "caller-authority",
    );
  });

  it("does not treat inert copy as path or browser-launch authority", () => {
    expect(
      auditAdversarialFixture(
        "./i18n/messages.ts",
        `export const copy = {
          search: "Name, path, or extension",
          warning: "Do not call window.open or import a shell plugin",
        };`,
      ),
    ).toEqual([]);
  });

  it("rejects an extra non-generic command in the approved wrapper", () => {
    const wrapper = Object.entries(productionModules).find(([path]) =>
      normalizedModulePath(path).endsWith(storageDesktopSuffix),
    )?.[1];
    expect(wrapper).toBeDefined();

    expect(
      auditAdversarialFixture(
        `.${storageDesktopSuffix}`,
        `${wrapper!}\nvoid invoke("erase_scan_source", {});`,
      ),
    ).toContain("native-command-inventory");
  });

  it("rejects a dynamic command even when it stays in the approved wrapper", () => {
    const wrapper = Object.entries(productionModules).find(([path]) =>
      normalizedModulePath(path).endsWith(storageDesktopSuffix),
    )?.[1];
    expect(wrapper).toBeDefined();

    expect(
      auditAdversarialFixture(
        `.${storageDesktopSuffix}`,
        `${wrapper!}\nvoid invoke(commandName, {});`,
      ),
    ).toContain("indirect-or-dynamic-native-invoke");
  });

  it("rejects an invoke alias even when it stays in the approved wrapper", () => {
    const wrapper = Object.entries(productionModules).find(([path]) =>
      normalizedModulePath(path).endsWith(storageDesktopSuffix),
    )?.[1];
    expect(wrapper).toBeDefined();

    expect(
      auditAdversarialFixture(
        `.${storageDesktopSuffix}`,
        `${wrapper!}\nconst callNative = invoke; void callNative("cancel_restore", {});`,
      ),
    ).toContain("indirect-or-dynamic-native-invoke");
  });
});
