import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import type {
  CandidatePage,
  FolderSelection,
  ScanSummary,
  StorageInventory,
} from "./api/storage";

const tauri = vi.hoisted(() => ({
  isTauri: vi.fn<() => boolean>(),
  invoke: vi.fn(),
}));
const rawDeviceMarker = "Physical" + "Drive0";

vi.mock("@tauri-apps/api/core", () => ({
  isTauri: tauri.isTauri,
  invoke: tauri.invoke,
}));

const inventory: StorageInventory = {
  schemaVersion: 1,
  generation: "inventory-7",
  disks: [
    {
      id: "disk-0",
      displayName: "Internal NVMe",
      busType: "nvme",
      sizeBytes: "2048000000000",
      volumes: [
        {
          id: "volume-c",
          mountLabel: "C:",
          label: "System",
          fileSystem: "ntfs",
          sizeBytes: "2027000000000",
          freeBytes: "751619276800",
          isSystem: true,
          scanSupported: true,
          folderScopeSupported: true,
          warnings: [],
        },
        {
          id: "volume-r",
          mountLabel: "R:",
          label: "Recovery",
          fileSystem: "unknown",
          sizeBytes: "21000000000",
          freeBytes: "0",
          isSystem: false,
          scanSupported: false,
          folderScopeSupported: false,
          warnings: ["This volume is not supported by the scanner."],
        },
      ],
    },
    {
      id: "disk-1",
      displayName: "Portable SSD",
      busType: "usb",
      sizeBytes: "1000000000000",
      volumes: [
        {
          id: "volume-e",
          mountLabel: "E:",
          label: "Archive",
          fileSystem: "fat32",
          sizeBytes: "999000000000",
          freeBytes: "450000000000",
          isSystem: false,
          scanSupported: true,
          folderScopeSupported: false,
          warnings: [],
        },
      ],
    },
  ],
};

const folder: FolderSelection = {
  schemaVersion: 1,
  scopeId: "scope-documents",
  volumeId: "volume-c",
  label: "Documents",
};

const summary: ScanSummary = {
  schemaVersion: 1,
  scanId: "scan-1",
  sourceLabel: "System (C:)",
  scope: { kind: "folder", label: "Documents" },
  fileSystem: "ntfs",
  scanStatus: "partial",
  totalCandidates: "14",
  matchedCandidates: "9",
  unknownCandidates: "2",
  warnings: ["The active volume changed while it was being read."],
};

const firstPage: CandidatePage = {
  schemaVersion: 1,
  scanId: "scan-1",
  cursor: null,
  nextCursor: "cursor-2",
  candidates: [
    {
      id: "candidate-1",
      displayPath: "Documents/deleted invoice.pdf",
      kind: "file",
      state: "likelyComplete",
      sizeBytes: "18446744073709551615",
      metadataConfidence: "high",
      recoverabilityScore: 88,
      pathState: "reconstructed",
      warnings: [],
    },
  ],
};

const secondPage: CandidatePage = {
  schemaVersion: 1,
  scanId: "scan-1",
  cursor: "cursor-2",
  nextCursor: null,
  candidates: [
    {
      id: "candidate-2",
      displayPath: "Documents/old notes.txt",
      kind: "file",
      state: "partial",
      sizeBytes: "4096",
      metadataConfidence: "medium",
      recoverabilityScore: 54,
      pathState: "incomplete",
      warnings: ["Some data extents could not be proven."],
    },
  ],
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function commandName(call: unknown[]): string {
  return String(call[0]);
}

function mockRealFlow() {
  tauri.invoke.mockImplementation(
    (command: string, payload: Record<string, unknown>) => {
      switch (command) {
        case "list_storage_sources":
          return Promise.resolve(inventory);
        case "select_scan_folder":
          return Promise.resolve(folder);
        case "scan_storage_volume":
          return Promise.resolve(summary);
        case "get_candidate_page":
          return Promise.resolve(
            payload.cursor === null ? firstPage : secondPage,
          );
        default:
          return Promise.reject(new Error(`Unexpected command: ${command}`));
      }
    },
  );
}

describe("real connected-storage desktop workflow", () => {
  beforeEach(() => {
    window.localStorage.clear();
    tauri.isTauri.mockReturnValue(true);
    tauri.invoke.mockReset();
  });

  it("WIN-BROWSER-FAIL-CLOSED-001 shows no inventory outside Tauri", () => {
    tauri.isTauri.mockReturnValue(false);

    render(<App />);

    expect(
      screen.getByRole("heading", { name: "Aplicativo desktop necessário" }),
    ).toBeTruthy();
    expect(
      screen.queryByRole("button", { name: "Atualizar unidades" }),
    ).toBeNull();
    expect(screen.queryByRole("radio")).toBeNull();
    expect(tauri.invoke).not.toHaveBeenCalled();
  });

  it("WIN-REAL-INVENTORY-001 loads and groups observed disks and volumes on open", async () => {
    tauri.invoke.mockResolvedValue(inventory);

    render(<App />);

    expect(
      await screen.findByRole("heading", { name: /Internal NVMe/u }),
    ).toBeTruthy();
    expect(screen.getByRole("heading", { name: /Portable SSD/u })).toBeTruthy();
    expect(screen.getByRole("radio", { name: /System.*C:/u })).toBeEnabled();
    expect(screen.getByRole("radio", { name: /Recovery.*R:/u })).toBeDisabled();
    expect(screen.getByText("NVMe")).toBeTruthy();
    expect(screen.getByText("USB")).toBeTruthy();
    expect(screen.getByText("NTFS")).toBeTruthy();
    expect(screen.getByText("700 GiB livres")).toBeTruthy();
    expect(tauri.invoke).toHaveBeenCalledTimes(1);
    expect(tauri.invoke).toHaveBeenCalledWith("list_storage_sources", {
      requestId: expect.stringMatching(/^inventory-[0-9a-z-]+$/u),
    });
    expect(JSON.stringify(tauri.invoke.mock.calls)).not.toContain(
      rawDeviceMarker,
    );
  });

  it("WIN-INVENTORY-EMPTY-001 shows a truthful empty state and refreshes", async () => {
    tauri.invoke
      .mockResolvedValueOnce({
        schemaVersion: 1,
        generation: "inventory-empty",
        disks: [],
      })
      .mockResolvedValueOnce(inventory);

    render(<App />);

    expect(
      await screen.findByText(
        "Nenhum volume local montado e compatível foi encontrado.",
      ),
    ).toBeTruthy();
    fireEvent.click(
      screen.getByRole("button", { name: "Atualizar unidades" }),
    );

    expect(
      await screen.findByRole("heading", { name: /Internal NVMe/u }),
    ).toBeTruthy();
    expect(
      tauri.invoke.mock.calls.map(commandName),
    ).toEqual(["list_storage_sources", "list_storage_sources"]);
  });

  it("WIN-FOLDER-CANCEL-001 treats native folder cancellation as unchanged scope", async () => {
    tauri.invoke.mockImplementation((command: string) => {
      if (command === "list_storage_sources") {
        return Promise.resolve(inventory);
      }
      if (command === "select_scan_folder") {
        return Promise.resolve(null);
      }
      return Promise.reject(new Error(`Unexpected command: ${command}`));
    });

    render(<App />);
    fireEvent.click(
      await screen.findByRole("radio", { name: /System.*C:/u }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Escolher pasta" }));

    expect(
      await screen.findByText(
        "A seleção da pasta foi cancelada. O volume inteiro continua selecionado.",
      ),
    ).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
    expect(tauri.invoke).toHaveBeenLastCalledWith("select_scan_folder", {
      requestId: expect.stringMatching(/^folder-[0-9a-z-]+$/u),
      generation: "inventory-7",
      volumeId: "volume-c",
    });
  });

  it("WIN-REAL-SCAN-001 scans the selected volume/folder then renders the first real page", async () => {
    mockRealFlow();
    render(<App />);

    fireEvent.click(
      await screen.findByRole("radio", { name: /System.*C:/u }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Escolher pasta" }));
    expect(await screen.findByText("Documents")).toBeTruthy();
    fireEvent.click(
      screen.getByRole("button", { name: "Analisar volume selecionado" }),
    );

    const resultHeading = await screen.findByRole("heading", {
      level: 1,
      name: "System (C:)",
    });
    expect(resultHeading).toHaveFocus();
    expect(screen.getByText("Cobertura parcial")).toBeTruthy();
    expect(
      screen.getByRole("heading", {
        name: "Ancestralidade desconhecida",
      }),
    ).toBeTruthy();
    expect(screen.getByText("2 candidatos")).toBeTruthy();
    const candidateTable = screen.getByRole("table", {
      name: "Candidatos encontrados",
    });
    const recoveredPath = within(candidateTable).getByText(
      "Documents/deleted invoice.pdf",
    );
    expect(recoveredPath.closest("bdi")).toHaveAttribute("dir", "auto");
    expect(within(candidateTable).getByText("88/100")).toBeTruthy();
    expect(
      within(candidateTable).getByText("18.446.744.073.709.551.615 B"),
    ).toBeTruthy();
    expect(tauri.invoke).toHaveBeenCalledWith("scan_storage_volume", {
      requestId: expect.stringMatching(/^scan-[0-9a-z-]+$/u),
      generation: "inventory-7",
      volumeId: "volume-c",
      scopeId: "scope-documents",
    });
    expect(tauri.invoke).toHaveBeenCalledWith("get_candidate_page", {
      requestId: expect.stringMatching(/^page-[0-9a-z-]+$/u),
      scanId: "scan-1",
      cursor: null,
      limit: 100,
    });
  });

  it("WIN-CANDIDATE-PAGINATION-001 appends bounded pages without losing prior rows", async () => {
    mockRealFlow();
    render(<App />);

    fireEvent.click(
      await screen.findByRole("radio", { name: /System.*C:/u }),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Analisar volume selecionado" }),
    );
    expect(
      await screen.findByText("Documents/deleted invoice.pdf"),
    ).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Carregar mais" }));

    expect(await screen.findByText("Documents/old notes.txt")).toBeTruthy();
    expect(screen.getByText("Documents/deleted invoice.pdf")).toBeTruthy();
    expect(
      screen.queryByRole("button", { name: "Carregar mais" }),
    ).toBeNull();
    const pageCalls = tauri.invoke.mock.calls.filter(
      (call) => call[0] === "get_candidate_page",
    );
    expect(pageCalls).toHaveLength(2);
    expect(pageCalls[1]?.[1]).toMatchObject({
      scanId: "scan-1",
      cursor: "cursor-2",
      limit: 100,
    });
  });

  it("WIN-DUPLICATE-SCAN-001 admits only one scan while the native request is pending", async () => {
    const pending = deferred<ScanSummary>();
    tauri.invoke.mockImplementation((command: string) => {
      if (command === "list_storage_sources") {
        return Promise.resolve(inventory);
      }
      if (command === "scan_storage_volume") {
        return pending.promise;
      }
      return Promise.reject(new Error(`Unexpected command: ${command}`));
    });

    render(<App />);
    fireEvent.click(
      await screen.findByRole("radio", { name: /System.*C:/u }),
    );
    const scanButton = screen.getByRole("button", {
      name: "Analisar volume selecionado",
    });
    fireEvent.click(scanButton);
    fireEvent.click(scanButton);

    expect(
      screen.getByRole("progressbar", { name: "Análise em andamento" }),
    ).toBeTruthy();
    expect(
      tauri.invoke.mock.calls.filter(
        (call) => call[0] === "scan_storage_volume",
      ),
    ).toHaveLength(1);

    await act(async () => {
      pending.reject({ code: "SCAN_INTERNAL" });
      await pending.promise.catch(() => undefined);
    });
  });

  it("WIN-ERROR-PRIVACY-001 sanitizes inventory failures and never renders native details", async () => {
    tauri.invoke.mockRejectedValue({
      code: "BROKER_UNAVAILABLE",
      message: String.raw`\\.\${rawDeviceMarker} C:\private-marker access denied`,
    });

    render(<App />);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "O serviço de leitura segura não pôde ser iniciado.",
    );
    expect(document.body.textContent).not.toContain("private-marker");
    expect(document.body.textContent).not.toContain(rawDeviceMarker);
    expect(document.body.textContent).not.toContain("access denied");
  });

  it("WIN-STALE-UNMOUNT-001 ignores a late inventory response after unmount", async () => {
    const pending = deferred<StorageInventory>();
    tauri.invoke.mockReturnValue(pending.promise);
    const errors = vi.spyOn(console, "error").mockImplementation(() => {});
    const mounted = render(<App />);

    mounted.unmount();
    await act(async () => {
      pending.resolve(inventory);
      await pending.promise;
    });

    expect(errors).not.toHaveBeenCalled();
    errors.mockRestore();
  });

  it("WIN-NAVIGATION-FOCUS-001 focuses each view heading", async () => {
    tauri.isTauri.mockReturnValue(false);
    render(<App />);
    const navigation = screen.getByRole("navigation", {
      name: "Undelete Master",
    });
    const [analysisButton, settingsButton, helpButton] =
      within(navigation).getAllByRole("button");

    fireEvent.click(settingsButton!);
    await waitFor(() => {
      expect(
        screen.getByRole("heading", { level: 1, name: "Configurações" }),
      ).toHaveFocus();
    });

    fireEvent.click(helpButton!);
    await waitFor(() => {
      expect(
        screen.getByRole("heading", { level: 1, name: "Ajuda e limites" }),
      ).toHaveFocus();
    });

    fireEvent.click(analysisButton!);
    await waitFor(() => {
      expect(
        screen.getByRole("heading", {
          level: 1,
          name: "Unidades conectadas",
        }),
      ).toHaveFocus();
    });
  });
});

describe("effective preferences", () => {
  beforeEach(() => {
    window.localStorage.clear();
    tauri.isTauri.mockReturnValue(false);
    tauri.invoke.mockReset();
  });

  it("DESKTOP-EFFECTIVE-PREFS-001 applies and persists language, theme, and reduced motion only", async () => {
    const first = render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "Configurações" }));

    fireEvent.change(screen.getByLabelText("Idioma"), {
      target: { value: "en-US" },
    });
    expect(
      await screen.findByRole("heading", { name: "Settings" }),
    ).toBeTruthy();

    fireEvent.change(screen.getByLabelText("Theme"), {
      target: { value: "light" },
    });
    fireEvent.click(
      screen.getByRole("checkbox", { name: /^Reduce motion/u }),
    );

    await waitFor(() => {
      expect(document.documentElement.dataset.theme).toBe("light");
      expect(document.documentElement.dataset.reducedMotion).toBe("true");
      expect(document.documentElement.lang).toBe("en-US");
    });

    const stored = JSON.parse(
      window.localStorage.getItem("undelete-master.preferences.v1") ?? "{}",
    ) as Record<string, unknown>;
    expect(stored).toEqual({
      locale: "en-US",
      theme: "light",
      reducedMotion: true,
    });

    first.unmount();
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.getByLabelText("Theme")).toHaveValue("light");
    expect(
      screen.getByRole("checkbox", { name: /^Reduce motion/u }),
    ).toBeChecked();
  });
});
