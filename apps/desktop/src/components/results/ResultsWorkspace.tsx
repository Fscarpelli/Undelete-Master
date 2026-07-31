import { AlertTriangle, LoaderCircle, RefreshCw } from "lucide-react";
import {
  useRef,
  type RefObject,
} from "react";
import type { ScanSummary } from "../../api/storage";
import { formatInteger } from "../../format";
import type { Locale, MessageKey } from "../../i18n/messages";
import type { RestoreWorkflowController } from "../../state/restoreWorkflow";
import type { ResultsWorkspaceController } from "../../state/resultsWorkspace";
import { CandidateResultsTable } from "./CandidateResultsTable";
import { RestoreWorkflowDialog } from "./RestoreWorkflowDialog";
import { ResultsFilters } from "./ResultsFilters";
import { SelectionBar } from "./SelectionBar";

export interface ResultsWorkspaceProps {
  summary: ScanSummary;
  controller: ResultsWorkspaceController;
  restore: RestoreWorkflowController;
  locale: Locale;
  t: (key: MessageKey) => string;
  recoverButtonRef: RefObject<HTMLButtonElement>;
}

function text(t: ResultsWorkspaceProps["t"], key: string): string {
  return t(key as MessageKey);
}

function noCandidatesKey(summary: ScanSummary): MessageKey {
  if (summary.scanMode === "deepJpeg") {
    return summary.scanStatus === "partial"
      ? "analysis.results.noCandidatesDeepPartial"
      : "analysis.results.noCandidatesDeepComplete";
  }
  return summary.scanStatus === "partial"
    ? "analysis.results.noCandidatesPartial"
    : "analysis.results.noCandidatesComplete";
}

export function ResultsWorkspace({
  summary,
  controller,
  restore,
  locale,
  t,
  recoverButtonRef,
}: ResultsWorkspaceProps) {
  const summaryRef = useRef<HTMLDivElement>(null);
  const page = controller.state.page;
  const visibleRows = page?.candidates.slice(0, 100).length ?? 0;
  const empty =
    page !== null &&
    BigInt(page.filteredTotal) === 0n &&
    controller.state.phase !== "loading";
  const initialPageError =
    page === null && controller.state.phase === "error";
  const emptyMessageKey =
    BigInt(summary.totalCandidates) === 0n
      ? noCandidatesKey(summary)
      : "results.summary.empty";

  return (
    <div className="results-workspace">
      <div className="section-heading results-workspace-heading">
        <div>
          <h2 id="results-workspace-heading">
            {t("analysis.results.table")}
          </h2>
          <p>
            <bdi dir="auto">{summary.sourceLabel}</bdi>
          </p>
        </div>
      </div>

      <div
        ref={summaryRef}
        className="results-summary"
        role="status"
        aria-live="polite"
        tabIndex={-1}
      >
        {page === null ? null : (
          <>
            <span>
              {text(t, "results.summary.filtered")}:{" "}
              <strong>
                {formatInteger(page.filteredTotal, locale)}
              </strong>
            </span>
            <span>
              {text(t, "results.summary.visible")}:{" "}
              <strong>
                {formatInteger(BigInt(visibleRows), locale)}
              </strong>
            </span>
          </>
        )}
        {controller.state.phase === "loading" ? (
          <span className="results-summary-loading">
            <LoaderCircle className="spinner" size={15} aria-hidden="true" />
            {text(t, "results.summary.loading")}
          </span>
        ) : null}
      </div>

      {controller.state.phase === "error" &&
      controller.state.error !== null ? (
        <div className="results-error error-message" role="alert">
          <AlertTriangle size={18} aria-hidden="true" />
          <span>{text(t, `error.${controller.state.error}`)}</span>
          <button
            type="button"
            className="button"
            onClick={controller.retry}
          >
            <RefreshCw size={16} aria-hidden="true" />
            {text(t, "results.summary.retry")}
          </button>
        </div>
      ) : null}

      <div className="results-layout">
        <ResultsFilters controller={controller} locale={locale} t={t} />
        <div className="results-table-column">
          {initialPageError ? null : empty ? (
            <p className="quiet-state">
              {t(emptyMessageKey)}
            </p>
          ) : (
            <CandidateResultsTable
              controller={controller}
              locale={locale}
              t={t}
              summaryRef={summaryRef}
            />
          )}
        </div>
      </div>

      <SelectionBar
        controller={controller}
        restore={restore}
        locale={locale}
        t={t}
        recoverButtonRef={recoverButtonRef}
      />
      <RestoreWorkflowDialog
        controller={restore}
        selection={controller.state.selection}
        locale={locale}
        t={t}
        recoverButtonRef={recoverButtonRef}
      />
    </div>
  );
}
