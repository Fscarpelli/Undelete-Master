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
import type { DesktopScanReport } from "./api/report";

const tauri = vi.hoisted(() => ({
  isTauri: vi.fn<() => boolean>(),
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  isTauri: tauri.isTauri,
  invoke: tauri.invoke,
}));

const report: DesktopScanReport = {
  schemaVersion: 2,
  source: {
    label: "evidence.img",
    sizeBytes: "9007199254740992",
  },
  partitionTable: "gpt",
  volumes: [
    {
      index: 7,
      offsetBytes: "9007199254740992",
      lengthBytes: "18446744073709551615",
      fileSystem: "ntfs",
      scanStatus: "complete",
      candidateCount: "42",
      warnings: ["The bitmap ended before the declared volume boundary."],
    },
  ],
  warnings: ["The backup partition header was not available."],
  warningCount: "2",
  warningsOmitted: "0",
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

describe("real-only desktop workflow", () => {
  beforeEach(() => {
    tauri.isTauri.mockReturnValue(true);
    tauri.invoke.mockReset();
  });

  it("DESKTOP-TAURI-ONLY-001 fails closed in a browser and never invokes the scanner", () => {
    tauri.isTauri.mockReturnValue(false);

    render(<App />);

    expect(
      screen.getByRole("heading", { name: "Aplicativo desktop necessário" }),
    ).toBeTruthy();
    expect(
      screen.queryByRole("button", {
        name: "Selecionar e analisar imagem",
      }),
    ).toBeNull();
    expect(tauri.invoke).not.toHaveBeenCalled();
  });

  it("DESKTOP-NAVIGATION-FOCUS-001 focuses each new view heading", async () => {
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
        screen.getByRole("heading", { level: 1, name: /Config/u }),
      ).toHaveFocus();
    });

    fireEvent.click(helpButton!);
    await waitFor(() => {
      expect(
        screen.getByRole("heading", { level: 1, name: /Ajuda/u }),
      ).toHaveFocus();
    });

    fireEvent.click(analysisButton!);
    await waitFor(() => {
      expect(
        screen.getByRole("heading", { level: 1, name: /An/u }),
      ).toHaveFocus();
    });
  });

  it("DESKTOP-PICKER-CANCEL-001 treats native picker cancellation as an idle outcome", async () => {
    tauri.invoke.mockResolvedValue(null);
    render(<App />);

    fireEvent.click(
      screen.getByRole("button", { name: "Selecionar e analisar imagem" }),
    );

    expect(
      await screen.findByText(
        "A seleção foi cancelada. Nenhuma imagem foi aberta.",
      ),
    ).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
    expect(tauri.invoke).toHaveBeenCalledWith("select_and_scan_image", {
      requestId: expect.stringMatching(/^scan-[0-9a-z]+$/),
    });
    expect(Object.keys(tauri.invoke.mock.calls[0]?.[1] ?? {})).toEqual([
      "requestId",
    ]);
  });

  it("DESKTOP-DOUBLE-SUBMIT-001 admits only one invoke when activation is repeated synchronously", () => {
    const pending = deferred<DesktopScanReport | null>();
    tauri.invoke.mockReturnValue(pending.promise);
    render(<App />);

    const button = screen.getByRole("button", {
      name: "Selecionar e analisar imagem",
    });
    fireEvent.click(button);
    fireEvent.click(button);

    expect(tauri.invoke).toHaveBeenCalledTimes(1);
    expect(
      screen.getByRole("progressbar", { name: "Análise em andamento" }),
    ).toBeTruthy();
  });

  it("DESKTOP-NAVIGATION-SINGLE-SCAN-001 keeps one scan in flight across navigation", async () => {
    const pending = deferred<DesktopScanReport | null>();
    tauri.invoke.mockReturnValue(pending.promise);
    render(<App />);

    fireEvent.click(
      screen.getByRole("button", { name: "Selecionar e analisar imagem" }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Configurações" }));
    fireEvent.click(screen.getByRole("button", { name: "Análise" }));

    expect(
      screen.getByRole("progressbar", { name: "Análise em andamento" }),
    ).toBeTruthy();
    expect(
      screen.queryByRole("button", {
        name: "Selecionar e analisar imagem",
      }),
    ).toBeNull();
    expect(tauri.invoke).toHaveBeenCalledTimes(1);

    await act(async () => {
      pending.resolve(report);
      await pending.promise;
    });

    expect(
      await screen.findByRole("heading", { name: "evidence.img" }),
    ).toHaveFocus();
    expect(tauri.invoke).toHaveBeenCalledTimes(1);
  });

  it("DESKTOP-STALE-RESPONSE-001 ignores a response after the app has been replaced", async () => {
    const pending = deferred<DesktopScanReport | null>();
    tauri.invoke.mockReturnValue(pending.promise);
    const mounted = render(<App />);

    fireEvent.click(
      screen.getByRole("button", { name: "Selecionar e analisar imagem" }),
    );
    mounted.unmount();

    await act(async () => {
      pending.resolve(report);
      await pending.promise;
    });

    render(<App />);
    expect(
      screen.getByRole("button", { name: "Selecionar e analisar imagem" }),
    ).toBeTruthy();
    expect(screen.queryByText("evidence.img")).toBeNull();
  });

  it("DESKTOP-REAL-SUMMARY-001 renders only the real bounded report and preserves large integers", async () => {
    tauri.invoke.mockResolvedValue(report);
    render(<App />);

    fireEvent.click(
      screen.getByRole("button", { name: "Selecionar e analisar imagem" }),
    );

    const sourceHeading = await screen.findByRole("heading", {
      name: "evidence.img",
    });
    expect(sourceHeading).toHaveFocus();
    expect(sourceHeading.querySelector("bdi")).toHaveAttribute("dir", "auto");
    expect(screen.getByText("Origem validada · somente leitura")).toBeTruthy();
    expect(screen.getAllByText("9.007.199.254.740.992").length).toBeGreaterThan(
      0,
    );
    expect(
      screen.getByText("18.446.744.073.709.551.615"),
    ).toBeTruthy();
    expect(screen.getByText("NTFS")).toBeTruthy();
    expect(screen.getByText("Completa")).toBeTruthy();
    expect(screen.getAllByText("42")).toHaveLength(2);
    expect(
      screen.getByText(
        "A contagem indica metadados identificados pelo scanner. Ela não garante que os arquivos possam ser recuperados.",
      ),
    ).toBeTruthy();
    expect(
      screen.getByText("The backup partition header was not available."),
    ).toBeTruthy();
  });

  it("DESKTOP-ERROR-PRIVACY-001 maps structured errors without exposing the backend message", async () => {
    tauri.invoke.mockRejectedValue({
      code: "SOURCE_IO",
      message: String.raw`C:\private-marker\source.img: access denied`,
    });
    render(<App />);

    fireEvent.click(
      screen.getByRole("button", { name: "Selecionar e analisar imagem" }),
    );

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Não foi possível abrir ou ler a imagem em modo somente leitura.",
    );
    expect(document.body.textContent).not.toContain("private-marker");
    expect(document.body.textContent).not.toContain("access denied");
  });
});

describe("effective preferences", () => {
  beforeEach(() => {
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
    expect(Object.keys(stored).sort()).toEqual([
      "locale",
      "reducedMotion",
      "theme",
    ]);

    first.unmount();
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.getByLabelText("Theme")).toHaveValue("light");
    expect(
      screen.getByRole("checkbox", { name: /^Reduce motion/u }),
    ).toBeChecked();
  });

  it("DESKTOP-PREFERENCE-STORAGE-001 reports session-only behavior when persistence fails", async () => {
    const setItem = vi
      .spyOn(Storage.prototype, "setItem")
      .mockImplementation(() => {
        throw new Error("storage unavailable");
      });

    try {
      render(<App />);
      fireEvent.click(
        screen.getByRole("button", { name: "Configurações" }),
      );

      expect(
        await screen.findByText(
          "As alterações valem nesta sessão, mas não puderam ser salvas neste dispositivo.",
        ),
      ).toBeTruthy();

      fireEvent.change(screen.getByLabelText("Tema"), {
        target: { value: "light" },
      });

      await waitFor(() => {
        expect(document.documentElement.dataset.theme).toBe("light");
      });
      expect(
        screen.getByText(
          "As alterações valem nesta sessão, mas não puderam ser salvas neste dispositivo.",
        ),
      ).toBeTruthy();
    } finally {
      setItem.mockRestore();
    }
  });
});
