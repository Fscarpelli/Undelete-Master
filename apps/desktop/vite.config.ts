import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import { applyDevelopmentCsp } from "./dev-csp";

const developmentCspPlugin: Plugin = {
  name: "undelete-master-development-csp",
  apply: "serve",
  transformIndexHtml: applyDevelopmentCsp,
};

// Tauri expects a fixed dev port; `clearScreen: false` keeps Rust logs visible
// when running under `tauri dev` later.
export default defineConfig({
  plugins: [react(), developmentCspPlugin],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    target: "es2022",
    outDir: "dist",
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
  },
} as never);
