import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { cwd } from "node:process";
import { describe, expect, it } from "vitest";
import { applyDevelopmentCsp } from "../dev-csp";
import indexHtml from "../index.html?raw";
import tauriConfig from "../src-tauri/tauri.conf.json";
import mainSource from "./main.tsx?raw";

const stylesheetSource = readFileSync(
  resolve(cwd(), "src/styles/global.css"),
  "utf8",
);

describe("CSP-compatible stylesheet loading", () => {
  it("DESKTOP-CSP-STYLES-001 loads the production stylesheet as a same-origin resource", () => {
    expect(indexHtml).toContain(
      '<link rel="stylesheet" href="/src/styles/global.css" />',
    );
    expect(indexHtml).not.toContain("'unsafe-inline'");
    expect(mainSource).not.toContain('import "./styles/global.css"');
  });

  it("DESKTOP-FORCED-COLORS-FOCUS-001 keeps focus visible without box shadows", () => {
    const forcedColors =
      stylesheetSource.match(
        /@media\s*\(forced-colors:\s*active\)\s*\{([\s\S]*)\}\s*$/u,
      )?.[1] ?? "";

    expect(forcedColors).toContain("button:focus-visible");
    expect(forcedColors).toContain("select:focus-visible");
    expect(forcedColors).toContain("input:focus-visible");
    expect(forcedColors).toContain("[data-view-heading]:focus-visible");
    expect(forcedColors).toContain("outline: 2px solid Highlight;");
    expect(forcedColors).toContain("outline-offset: 3px;");
    expect(forcedColors).toContain("box-shadow: none;");
  });

  it("DESKTOP-DEV-CSP-001 keeps production free of WebSocket sources", () => {
    const productionCsp = tauriConfig.app.security.csp;
    const developmentCsp = tauriConfig.app.security.devCsp;

    expect(indexHtml).toContain(productionCsp);
    expect(indexHtml).not.toContain("ws:");
    expect(indexHtml).not.toContain("wss:");
    expect(productionCsp).not.toContain("ws:");
    expect(productionCsp).not.toContain("wss:");
    expect(developmentCsp).toContain(
      "connect-src ipc: http://ipc.localhost ws://localhost:1420",
    );
    expect(developmentCsp.replace(" ws://localhost:1420", "")).toBe(
      productionCsp,
    );
  });

  it("DESKTOP-DEV-CSP-002 aligns the served meta policy with the fixed HMR endpoint", () => {
    const developmentHtml = applyDevelopmentCsp(indexHtml);
    const developmentCsp = tauriConfig.app.security.devCsp;

    expect(developmentHtml).toContain(
      `content="${developmentCsp}"`,
    );
    expect(developmentHtml.match(/ws:\/\/localhost:1420/gu)).toHaveLength(1);
    expect(developmentHtml).not.toContain("ws://0.0.0.0");
    expect(developmentHtml).not.toContain("ws://127.0.0.1");
  });
});
