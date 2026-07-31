import { createRef } from "react";
import {
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import {
  afterAll,
  beforeAll,
  describe,
  expect,
  it,
  vi,
} from "vitest";
import type { CandidateSelectionSummary } from "../../api/storage";
import { translate } from "../../i18n/messages";
import type { RestoreWorkflowController } from "../../state/restoreWorkflow";
import { RestoreWorkflowDialog } from "./RestoreWorkflowDialog";

const selection: CandidateSelectionSummary = {
  selectionRevision: "9",
  selectedCandidates: "3",
  selectedFiles: "3",
  selectedDirectories: "0",
  selectedLogicalBytes: "8192",
  bestEffortCandidates: "1",
  conflictedCandidates: "1",
  ineligibleCandidates: "0",
  matchingSelectedCandidates: "3",
};

function controller(
  overrides: Partial<RestoreWorkflowController["state"]> = {},
): RestoreWorkflowController {
  return {
    state: {
      phase: "setup",
      scanId: "scan-1",
      selectionRevision: "9",
      destination: {
        schemaVersion: 1,
        destinationId: "destination-1",
        label: "Recovery drive",
        volumeLabel: "Backup",
        fileSystem: "NTFS",
        freeBytes: "10485760",
        relation: "different",
      },
      collisionPolicy: "rename",
      partialFilePolicy: "completeOnly",
      bestEffortConsent: false,
      plan: null,
      job: null,
      operationPhase: "idle",
      progressBasisPoints: 0,
      cancelRequested: false,
      error: null,
      ...overrides,
    },
    chooseDestination: vi.fn(),
    setPartialFilePolicy: vi.fn(),
    setBestEffortConsent: vi.fn(),
    createPlan: vi.fn(),
    start: vi.fn(),
    cancel: vi.fn(),
    openDestination: vi.fn(),
    close: vi.fn(),
  };
}

const t = (key: string) => key;

const originalShowModal = Object.getOwnPropertyDescriptor(
  HTMLDialogElement.prototype,
  "showModal",
);
const originalClose = Object.getOwnPropertyDescriptor(
  HTMLDialogElement.prototype,
  "close",
);

beforeAll(() => {
  Object.defineProperty(HTMLDialogElement.prototype, "showModal", {
    configurable: true,
    value(this: HTMLDialogElement) {
      this.setAttribute("open", "");
    },
  });
  Object.defineProperty(HTMLDialogElement.prototype, "close", {
    configurable: true,
    value(this: HTMLDialogElement) {
      this.removeAttribute("open");
    },
  });
});

afterAll(() => {
  if (originalShowModal === undefined) {
    Reflect.deleteProperty(HTMLDialogElement.prototype, "showModal");
  } else {
    Object.defineProperty(
      HTMLDialogElement.prototype,
      "showModal",
      originalShowModal,
    );
  }
  if (originalClose === undefined) {
    Reflect.deleteProperty(HTMLDialogElement.prototype, "close");
  } else {
    Object.defineProperty(
      HTMLDialogElement.prototype,
      "close",
      originalClose,
    );
  }
});

describe("transactional restore dialog", () => {
  it("DESKTOP-RESTORE-FLOW-024 explains that complete-only requires removing best-effort items", () => {
    expect(translate("en-US", "restore.policy.completeOnlyBody")).toBe(
      "Remove every best-effort item from the selection, or choose “Fill gaps with zeros and create a map” to continue.",
    );
    expect(translate("pt-BR", "restore.policy.completeOnlyBody")).toBe(
      "Remova da seleção todos os itens que exigem recuperação parcial ou escolha “Preencher lacunas com zeros e gerar mapa” para continuar.",
    );
  });

  it("keeps the generic internal-error copy honest when a job outcome is not known", () => {
    expect(translate("en-US", "error.RESTORE_INTERNAL")).toBe(
      "An internal failure prevented the requested operation from being confirmed or completed.",
    );
    expect(translate("pt-BR", "error.RESTORE_INTERNAL")).toBe(
      "Uma falha interna impediu confirmar ou concluir a operação solicitada.",
    );
  });

  it("DESKTOP-RESTORE-A11Y-025 requires zero-fill policy and explicit consent in the native dialog", () => {
    const workflow = controller();
    const rendered = render(
      <RestoreWorkflowDialog
        controller={workflow}
        selection={selection}
        locale="en-US"
        t={t}
        recoverButtonRef={createRef<HTMLButtonElement>()}
      />,
    );

    expect(
      screen.getByRole("button", { name: "restore.setup.review" }),
    ).toBeDisabled();
    fireEvent.click(
      screen.getByRole("radio", {
        name: /^restore\.policy\.zeroFillAndMap/u,
      }),
    );
    expect(workflow.setPartialFilePolicy).toHaveBeenCalledWith(
      "zeroFillAndMap",
    );

    const zeroFillWorkflow = controller({
      partialFilePolicy: "zeroFillAndMap",
    });
    rendered.rerender(
      <RestoreWorkflowDialog
        controller={zeroFillWorkflow}
        selection={selection}
        locale="en-US"
        t={t}
        recoverButtonRef={createRef<HTMLButtonElement>()}
      />,
    );
    const consent = screen.getByRole("checkbox", {
      name: "restore.policy.consent",
    });
    expect(consent).toBeTruthy();
    fireEvent.click(consent);
    expect(zeroFillWorkflow.setBestEffortConsent).toHaveBeenCalledWith(true);
  });

  it("DESKTOP-RESTORE-A11Y-025 renders only native progress and sends cancel through the controller", () => {
    const workflow = controller({
      phase: "active",
      operationPhase: "idle",
      progressBasisPoints: 6250,
      error: "RESTORE_INTERNAL",
      job: {
        schemaVersion: 1,
        jobId: "job-1",
        planId: "plan-1",
        status: "running",
        itemsTotal: "4",
        itemsCompleted: "3",
        itemsFailed: "0",
        itemsCancelled: "0",
        bytesTotal: "8192",
        bytesCompleted: "5120",
        currentItem: {
          ordinal: "4",
          candidateId: "candidate-4",
          kind: "file",
        },
        warnings: [],
        manifest: null,
      },
    });
    const rendered = render(
      <RestoreWorkflowDialog
        controller={workflow}
        selection={selection}
        locale="en-US"
        t={t}
        recoverButtonRef={createRef<HTMLButtonElement>()}
      />,
    );

    const progress = screen.getByRole("progressbar", {
      name: "restore.progress.label",
    });
    expect(progress).toHaveAttribute("max", "10000");
    expect(progress).toHaveAttribute("value", "6250");
    expect(screen.getByRole("alert")).toHaveTextContent(
      "error.RESTORE_INTERNAL",
    );
    expect(screen.getByText("5 KiB / 8 KiB")).toBeTruthy();
    fireEvent.click(
      screen.getByRole("button", { name: "restore.progress.cancel" }),
    );
    expect(workflow.cancel).toHaveBeenCalledOnce();

    rendered.rerender(
      <RestoreWorkflowDialog
        controller={controller({
          ...workflow.state,
          operationPhase: "cancelRequest",
        })}
        selection={selection}
        locale="en-US"
        t={t}
        recoverButtonRef={createRef<HTMLButtonElement>()}
      />,
    );
    expect(
      screen.getByRole("button", {
        name: "restore.progress.cancelling",
      }),
    ).toBeDisabled();
  });

  it("DESKTOP-RESTORE-A11Y-025 disables setup controls while native planning is pending", () => {
    const workflow = controller({ phase: "planning" });
    render(
      <RestoreWorkflowDialog
        controller={workflow}
        selection={selection}
        locale="en-US"
        t={t}
        recoverButtonRef={createRef<HTMLButtonElement>()}
      />,
    );

    for (const radio of screen.getAllByRole("radio")) {
      expect(radio).toBeDisabled();
    }
    expect(
      screen.getByRole("button", { name: "restore.destination.change" }),
    ).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "restore.setup.review" }),
    ).toBeDisabled();
  });

  it("DESKTOP-RESTORE-A11Y-025 announces setup validation from controller state", () => {
    render(
      <RestoreWorkflowDialog
        controller={controller({
          phase: "error",
          error: "RESTORE_PARTIAL_POLICY_REQUIRED",
        })}
        selection={selection}
        locale="en-US"
        t={t}
        recoverButtonRef={createRef<HTMLButtonElement>()}
      />,
    );

    expect(screen.getByRole("alert")).toHaveTextContent(
      "error.RESTORE_PARTIAL_POLICY_REQUIRED",
    );
    expect(
      screen.getByRole("button", { name: "restore.setup.review" }),
    ).toBeDisabled();
  });

  it("DESKTOP-RESTORE-FLOW-024 preserves a non-terminal job and offers cancel after an error", () => {
    const workflow = controller({
      phase: "error",
      error: "RESTORE_INTERNAL",
      job: {
        schemaVersion: 1,
        jobId: "job-active",
        planId: "plan-1",
        status: "running",
        itemsTotal: "3",
        itemsCompleted: "1",
        itemsFailed: "0",
        itemsCancelled: "0",
        bytesTotal: "8192",
        bytesCompleted: "2048",
        currentItem: {
          ordinal: "2",
          candidateId: "candidate-2",
          kind: "file",
        },
        warnings: [],
        manifest: null,
      },
    });
    render(
      <RestoreWorkflowDialog
        controller={workflow}
        selection={selection}
        locale="en-US"
        t={t}
        recoverButtonRef={createRef<HTMLButtonElement>()}
      />,
    );

    expect(
      screen.queryByRole("button", { name: "restore.dialog.close" }),
    ).toBeNull();
    fireEvent.click(
      screen.getByRole("button", { name: "restore.progress.cancel" }),
    );
    expect(workflow.cancel).toHaveBeenCalledOnce();
    expect(workflow.close).not.toHaveBeenCalled();
  });

  it("DESKTOP-RESTORE-A11Y-025 focuses the native dialog step and returns focus after close", () => {
    const recoverButtonRef = createRef<HTMLButtonElement>();
    const workflow = controller();
    const rendered = render(
      <>
        <button ref={recoverButtonRef} type="button">
          results.selection.recover
        </button>
        <RestoreWorkflowDialog
          controller={workflow}
          selection={selection}
          locale="en-US"
          t={t}
          recoverButtonRef={recoverButtonRef}
        />
      </>,
    );

    expect(screen.getByRole("dialog")).toBeInstanceOf(HTMLDialogElement);
    expect(
      screen.getByRole("heading", { name: "restore.setup.title" }),
    ).toHaveFocus();

    rendered.rerender(
      <>
        <button ref={recoverButtonRef} type="button">
          results.selection.recover
        </button>
        <RestoreWorkflowDialog
          controller={controller({
            phase: "idle",
            destination: null,
          })}
          selection={selection}
          locale="en-US"
          t={t}
          recoverButtonRef={recoverButtonRef}
        />
      </>,
    );

    expect(recoverButtonRef.current).toHaveFocus();
  });

  it("DESKTOP-RESTORE-OPEN-DESTINATION-026 shows terminal manifest evidence and opens only through the native action", () => {
    const workflow = controller({
      phase: "finished",
      operationPhase: "idle",
      progressBasisPoints: 10000,
      job: {
        schemaVersion: 1,
        jobId: "job-1",
        planId: "plan-1",
        status: "completed",
        itemsTotal: "3",
        itemsCompleted: "3",
        itemsFailed: "0",
        itemsCancelled: "0",
        bytesTotal: "8192",
        bytesCompleted: "8192",
        currentItem: null,
        warnings: ["One item requires inspection."],
        manifest: {
          manifestSha256:
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          completionStatus: "needsReconciliation",
          publishedItems: "3",
          partialItems: "1",
        },
      },
    });
    const rendered = render(
      <RestoreWorkflowDialog
        controller={workflow}
        selection={selection}
        locale="en-US"
        t={t}
        recoverButtonRef={createRef<HTMLButtonElement>()}
      />,
    );

    expect(screen.getByText("restore.finish.completed")).toBeTruthy();
    expect(screen.queryByText("restore.finish.failed")).toBeNull();
    expect(screen.queryByText("restore.finish.cancelled")).toBeNull();
    expect(screen.getByText("restore.finish.itemsCompleted")).toBeTruthy();
    expect(screen.getByText("restore.finish.itemsFailed")).toBeTruthy();
    expect(screen.getByText("restore.finish.itemsCancelled")).toBeTruthy();
    expect(screen.getByText("restore.finish.needsReconciliation")).toBeTruthy();
    const partialFact = screen
      .getByText("restore.finish.partial")
      .closest("div");
    expect(partialFact).not.toBeNull();
    expect(within(partialFact!).getByText("1")).toBeTruthy();
    expect(
      screen.getByText(
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      ),
    ).toBeTruthy();
    fireEvent.click(
      screen.getByRole("button", { name: "restore.finish.openDestination" }),
    );
    expect(workflow.openDestination).toHaveBeenCalledOnce();

    rendered.rerender(
      <RestoreWorkflowDialog
        controller={controller({
          ...workflow.state,
          operationPhase: "opening",
        })}
        selection={selection}
        locale="en-US"
        t={t}
        recoverButtonRef={createRef<HTMLButtonElement>()}
      />,
    );
    expect(
      screen.getByRole("button", {
        name: "restore.finish.openDestination",
      }),
    ).toBeDisabled();
  });

  it("DESKTOP-RESTORE-OPEN-DESTINATION-026 never offers open destination for a failed job", () => {
    const workflow = controller({
      phase: "finished",
      job: {
        schemaVersion: 1,
        jobId: "job-failed",
        planId: "plan-1",
        status: "failed",
        itemsTotal: "3",
        itemsCompleted: "1",
        itemsFailed: "2",
        itemsCancelled: "0",
        bytesTotal: "8192",
        bytesCompleted: "2048",
        currentItem: null,
        warnings: [],
        manifest: null,
      },
    });
    render(
      <RestoreWorkflowDialog
        controller={workflow}
        selection={selection}
        locale="en-US"
        t={t}
        recoverButtonRef={createRef<HTMLButtonElement>()}
      />,
    );

    expect(screen.getByText("restore.finish.failed")).toBeTruthy();
    expect(screen.queryByText("restore.finish.completed")).toBeNull();
    expect(screen.queryByText("restore.finish.cancelled")).toBeNull();
    expect(
      screen.queryByRole("button", {
        name: "restore.finish.openDestination",
      }),
    ).toBeNull();
  });

  it("DESKTOP-RESTORE-FLOW-024 reports lost tracking as an unknown outcome with close instead of cancel", () => {
    const workflow = controller({
      phase: "trackingLost",
      error: "RESTORE_INTERNAL",
      job: {
        schemaVersion: 1,
        jobId: "job-untracked",
        planId: "plan-1",
        status: "running",
        itemsTotal: "3",
        itemsCompleted: "1",
        itemsFailed: "0",
        itemsCancelled: "0",
        bytesTotal: "8192",
        bytesCompleted: "2048",
        currentItem: {
          ordinal: "2",
          candidateId: "candidate-2",
          kind: "file",
        },
        warnings: [],
        manifest: null,
      },
    });
    render(
      <RestoreWorkflowDialog
        controller={workflow}
        selection={selection}
        locale="en-US"
        t={t}
        recoverButtonRef={createRef<HTMLButtonElement>()}
      />,
    );

    expect(
      screen.getByRole("heading", {
        name: "restore.trackingLost.title",
      }),
    ).toBeTruthy();
    expect(screen.getByRole("alert")).toHaveTextContent(
      "restore.trackingLost.body",
    );
    fireEvent.click(
      screen.getAllByRole("button", {
        name: "restore.dialog.close",
      })[0]!,
    );
    expect(workflow.close).toHaveBeenCalledOnce();
    expect(
      screen.queryByRole("button", {
        name: "restore.progress.cancel",
      }),
    ).toBeNull();
  });

  it("DESKTOP-RESTORE-OPEN-DESTINATION-026 keeps completed evidence and retry available after opening fails", () => {
    const workflow = controller({
      phase: "finished",
      error: "RESTORE_INTERNAL",
      job: {
        schemaVersion: 1,
        jobId: "job-completed",
        planId: "plan-1",
        status: "completed",
        itemsTotal: "3",
        itemsCompleted: "3",
        itemsFailed: "0",
        itemsCancelled: "0",
        bytesTotal: "8192",
        bytesCompleted: "8192",
        currentItem: null,
        warnings: [],
        manifest: {
          manifestSha256:
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          completionStatus: "completedDurable",
          publishedItems: "3",
          partialItems: "0",
        },
      },
    });
    render(
      <RestoreWorkflowDialog
        controller={workflow}
        selection={selection}
        locale="en-US"
        t={t}
        recoverButtonRef={createRef<HTMLButtonElement>()}
      />,
    );

    expect(screen.getByRole("alert")).toHaveTextContent(
      "error.RESTORE_INTERNAL",
    );
    fireEvent.click(
      screen.getByRole("button", {
        name: "restore.finish.openDestination",
      }),
    );
    expect(workflow.openDestination).toHaveBeenCalledOnce();
  });
});
