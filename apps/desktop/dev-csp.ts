const PRODUCTION_CONNECT_SOURCE =
  "connect-src ipc: http://ipc.localhost";
const DEVELOPMENT_CONNECT_SOURCE =
  `${PRODUCTION_CONNECT_SOURCE} ws://localhost:1420`;

/**
 * Vite serves the source index.html, whose production meta CSP is intentionally
 * stricter than Tauri's development CSP. Add the single fixed HMR endpoint only
 * while Vite is serving the document so both enforced policies agree.
 */
export function applyDevelopmentCsp(html: string): string {
  const occurrenceCount =
    html.split(PRODUCTION_CONNECT_SOURCE).length - 1;

  if (occurrenceCount !== 1) {
    throw new Error(
      "Expected exactly one production connect-src directive in index.html",
    );
  }

  return html.replace(
    PRODUCTION_CONNECT_SOURCE,
    DEVELOPMENT_CONNECT_SOURCE,
  );
}
