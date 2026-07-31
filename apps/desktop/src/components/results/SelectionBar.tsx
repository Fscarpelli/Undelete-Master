import {
  ArchiveRestore,
  Eraser,
  ListX,
  ShieldAlert,
} from "lucide-react";
import type { RefObject } from "react";
import { formatBytes, formatInteger } from "../../format";
import type { Locale, MessageKey } from "../../i18n/messages";
import type { RestoreWorkflowController } from "../../state/restoreWorkflow";
import type { ResultsWorkspaceController } from "../../state/resultsWorkspace";

export interface SelectionBarProps {
  controller: ResultsWorkspaceController;
  restore: RestoreWorkflowController;
  locale: Locale;
  t: (key: MessageKey) => string;
  recoverButtonRef: RefObject<HTMLButtonElement>;
}

function text(t: SelectionBarProps["t"], key: string): string {
  return t(key as MessageKey);
}

export function SelectionBar({
  controller,
  restore,
  locale,
  t,
  recoverButtonRef,
}: SelectionBarProps) {
  const selection = controller.state.selection;
  if (selection === null) {
    return null;
  }

  const selected = BigInt(selection.selectedCandidates);
  const matchingSelected = BigInt(selection.matchingSelectedCandidates);
  const ineligible = BigInt(selection.ineligibleCandidates);
  const updating =
    controller.state.selectionPhase === "updating";
  const recoveryBlocked =
    selected === 0n ||
    ineligible > 0n ||
    updating ||
    restore.state.phase !== "idle";

  return (
    <aside
      className="results-selection-bar"
      aria-labelledby="results-selection-summary"
      aria-live="polite"
    >
      <div className="results-selection-copy">
        <div className="results-selection-heading">
          <ArchiveRestore size={20} aria-hidden="true" />
          <h3 id="results-selection-summary">
            {text(t, "results.selection.summary")}:{" "}
            {formatInteger(selected, locale)}
          </h3>
        </div>
        <dl className="results-selection-facts">
          <div>
            <dt>{text(t, "results.selection.files")}</dt>
            <dd>{formatInteger(selection.selectedFiles, locale)}</dd>
          </div>
          <div>
            <dt>{text(t, "results.selection.directories")}</dt>
            <dd>{formatInteger(selection.selectedDirectories, locale)}</dd>
          </div>
          <div>
            <dt>{text(t, "results.selection.bytes")}</dt>
            <dd>{formatBytes(selection.selectedLogicalBytes, locale)}</dd>
          </div>
          <div>
            <dt>{text(t, "results.selection.bestEffort")}</dt>
            <dd>{formatInteger(selection.bestEffortCandidates, locale)}</dd>
          </div>
          <div>
            <dt>{text(t, "results.selection.conflicts")}</dt>
            <dd>{formatInteger(selection.conflictedCandidates, locale)}</dd>
          </div>
          <div>
            <dt>{text(t, "results.selection.ineligible")}</dt>
            <dd>{formatInteger(selection.ineligibleCandidates, locale)}</dd>
          </div>
        </dl>
        {ineligible > 0n ? (
          <p className="results-selection-blocked" role="alert">
            <ShieldAlert size={16} aria-hidden="true" />
            {text(t, "results.selection.ineligibleBlocked")}
          </p>
        ) : null}
        {updating ? (
          <p className="results-selection-updating" role="status">
            {text(t, "results.selection.updating")}
          </p>
        ) : null}
      </div>

      <div className="results-selection-actions">
        <button
          type="button"
          className="button"
          disabled={matchingSelected === 0n || updating}
          onClick={controller.clearMatching}
        >
          <Eraser size={16} aria-hidden="true" />
          {text(t, "results.selection.clearMatching")}
        </button>
        <button
          type="button"
          className="button"
          disabled={selected === 0n || updating}
          onClick={controller.clearAll}
        >
          <ListX size={16} aria-hidden="true" />
          {text(t, "results.selection.clearAll")}
        </button>
        <button
          ref={recoverButtonRef}
          type="button"
          className="button button-primary"
          disabled={recoveryBlocked}
          onClick={restore.chooseDestination}
        >
          <ArchiveRestore size={17} aria-hidden="true" />
          {text(t, "results.selection.recover")}
        </button>
      </div>
    </aside>
  );
}
