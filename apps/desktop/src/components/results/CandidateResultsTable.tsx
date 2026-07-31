import {
  AlertTriangle,
  ArrowUpDown,
  File,
  Folder,
} from "lucide-react";
import {
  useLayoutEffect,
  useMemo,
  useRef,
  type AriaAttributes,
  type RefObject,
} from "react";
import type {
  ActionableCandidateRow,
  CandidateSortField,
  CandidateState,
  DiscoveryMethod,
  MetadataConfidence,
  RecoveryEligibility,
} from "../../api/storage";
import { formatBytes, formatInteger } from "../../format";
import type { Locale, MessageKey } from "../../i18n/messages";
import type { ResultsWorkspaceController } from "../../state/resultsWorkspace";

export interface CandidateResultsTableProps {
  controller: ResultsWorkspaceController;
  locale: Locale;
  t: (key: MessageKey) => string;
  summaryRef: RefObject<HTMLDivElement>;
}

const stateKeys: Record<CandidateState, MessageKey> = {
  exactEvidence: "candidate.state.exactEvidence",
  likelyComplete: "candidate.state.likelyComplete",
  completeUnvalidated: "candidate.state.completeUnvalidated",
  structurallyValid: "candidate.state.structurallyValid",
  partial: "candidate.state.partial",
  conflicted: "candidate.state.conflicted",
  readError: "candidate.state.readError",
  zeroedOrTrimmed: "candidate.state.zeroedOrTrimmed",
  overwritten: "candidate.state.overwritten",
  metadataOnly: "candidate.state.metadataOnly",
  unknown: "candidate.state.unknown",
};
const confidenceKeys: Record<MetadataConfidence, MessageKey> = {
  high: "candidate.confidence.high",
  medium: "candidate.confidence.medium",
  low: "candidate.confidence.low",
};
const methodKeys: Record<DiscoveryMethod, MessageKey> = {
  ntfsMetadata: "candidate.method.ntfsMetadata",
  fatMetadata: "candidate.method.fatMetadata",
  exfatMetadata: "candidate.method.exfatMetadata",
  carving: "candidate.method.carving",
  recycleBin: "candidate.method.recycleBin",
};

function text(t: CandidateResultsTableProps["t"], key: string): string {
  return t(key as MessageKey);
}

function sortValue(
  activeField: CandidateSortField,
  activeDirection: "ascending" | "descending",
  field: CandidateSortField,
): AriaAttributes["aria-sort"] {
  return activeField === field ? activeDirection : "none";
}

function SortHeader({
  field,
  label,
  controller,
}: {
  field: CandidateSortField;
  label: string;
  controller: ResultsWorkspaceController;
}) {
  return (
    <th
      scope="col"
      aria-sort={sortValue(
        controller.state.sort.field,
        controller.state.sort.direction,
        field,
      )}
    >
      <button
        type="button"
        className="results-sort-button"
        onClick={() => controller.sortBy(field)}
      >
        <span>{label}</span>
        <ArrowUpDown size={14} aria-hidden="true" />
      </button>
    </th>
  );
}

function eligibilityLabel(
  t: CandidateResultsTableProps["t"],
  eligibility: RecoveryEligibility,
): string {
  return text(t, `results.eligibility.${eligibility}`);
}

function CandidateRow({
  candidate,
  controller,
  locale,
  t,
}: {
  candidate: ActionableCandidateRow;
  controller: ResultsWorkspaceController;
  locale: Locale;
  t: CandidateResultsTableProps["t"];
}) {
  const KindIcon = candidate.kind === "directory" ? Folder : File;
  const selectionDisabled =
    controller.state.selectionPhase === "updating";
  return (
    <tr
      data-candidate-id={candidate.id}
      data-eligibility={candidate.eligibility}
      data-selected={candidate.selected ? "true" : "false"}
    >
      <td className="results-selection-column">
        <input
          type="checkbox"
          checked={candidate.selected}
          disabled={selectionDisabled}
          aria-label={`${text(t, "results.selection.row")}: ${
            candidate.displayPath
          }`}
          onChange={(event) =>
            controller.setCandidateSelected(
              candidate.id,
              event.currentTarget.checked,
            )
          }
        />
      </td>
      <th scope="row">
        <div className="candidate-path results-path">
          <KindIcon size={17} aria-hidden="true" />
          <bdi dir="auto">{candidate.displayPath}</bdi>
        </div>
      </th>
      <td>
        <bdi dir="auto">
          {candidate.extension.length === 0
            ? text(t, "results.filters.noExtension")
            : candidate.extension}
        </bdi>
      </td>
      <td>{t(`candidate.kind.${candidate.kind}` as MessageKey)}</td>
      <td>{t(methodKeys[candidate.method])}</td>
      <td className="number-cell">
        <span>{formatInteger(candidate.sizeBytes, locale)} B</span>
        <span className="cell-secondary">
          {formatBytes(candidate.sizeBytes, locale)}
        </span>
      </td>
      <td>
        <span className="candidate-state" data-state={candidate.state}>
          {t(stateKeys[candidate.state])}
        </span>
      </td>
      <td>{t(confidenceKeys[candidate.metadataConfidence])}</td>
      <td className="number-cell">
        {candidate.recoverabilityScore === null
          ? "—"
          : `${formatInteger(
              BigInt(candidate.recoverabilityScore),
              locale,
            )}/100`}
      </td>
      <td>
        <span
          className="results-eligibility"
          data-eligibility={candidate.eligibility}
        >
          {eligibilityLabel(t, candidate.eligibility)}
        </span>
      </td>
      <td>
        {candidate.warnings.length === 0 ? (
          <span className="cell-secondary">—</span>
        ) : (
          <ul className="candidate-warnings">
            {candidate.warnings.map((warning, index) => (
              <li key={`${candidate.id}-${index.toString()}`}>
                <AlertTriangle size={14} aria-hidden="true" />
                <bdi dir="auto">{warning}</bdi>
              </li>
            ))}
          </ul>
        )}
      </td>
    </tr>
  );
}

export function CandidateResultsTable({
  controller,
  locale,
  t,
  summaryRef,
}: CandidateResultsTableProps) {
  const page = controller.state.page;
  const candidates = useMemo(
    () => page?.candidates.slice(0, 100) ?? [],
    [page],
  );
  const candidateIdentity = candidates.map((candidate) => candidate.id).join(
    "\u001f",
  );
  const focusedCandidate = useRef<string | null>(null);

  useLayoutEffect(() => {
    const previousFocusedCandidate = focusedCandidate.current;
    if (
      previousFocusedCandidate !== null &&
      !candidates.some(
        (candidate) => candidate.id === previousFocusedCandidate,
      ) &&
      (document.activeElement === document.body ||
        document.activeElement === null)
    ) {
      summaryRef.current?.focus();
      focusedCandidate.current = null;
    }
  }, [candidateIdentity, candidates, summaryRef]);

  useLayoutEffect(
    () => () => {
      if (focusedCandidate.current !== null) {
        summaryRef.current?.focus();
      }
    },
    [summaryRef],
  );

  if (page === null) {
    return (
      <div
        className="results-table-region"
        role="status"
        aria-live="polite"
      >
        {controller.state.phase === "loading"
          ? text(t, "results.summary.loading")
          : text(t, "results.summary.empty")}
      </div>
    );
  }

  const filteredTotal = BigInt(page.filteredTotal);
  const matchingSelected = BigInt(
    controller.state.selection?.matchingSelectedCandidates ?? "0",
  );
  const allMatchingSelected =
    filteredTotal > 0n && matchingSelected === filteredTotal;
  const someMatchingSelected =
    matchingSelected > 0n && matchingSelected < filteredTotal;
  const selectionDisabled =
    filteredTotal === 0n ||
    controller.state.selectionPhase === "updating";

  return (
    <div className="results-table-region">
      <div
        className="results-table-scroll"
        role="region"
        aria-label={t("analysis.results.table")}
        tabIndex={0}
      >
        <table
          className="results-table candidate-table"
          aria-label={t("analysis.results.table")}
          aria-rowcount={Number(filteredTotal)}
          aria-busy={controller.state.phase === "loading"}
          onFocusCapture={(event) => {
            const row = event.target.closest<HTMLElement>(
              "[data-candidate-id]",
            );
            focusedCandidate.current = row?.dataset.candidateId ?? null;
          }}
          onBlurCapture={(event) => {
            if (!(event.relatedTarget instanceof Element)) {
              return;
            }
            const nextRow = event.relatedTarget.closest<HTMLElement>(
              "[data-candidate-id]",
            );
            focusedCandidate.current =
              nextRow?.dataset.candidateId ?? null;
          }}
        >
          <caption className="visually-hidden">
            {t("analysis.results.table")}
          </caption>
          <thead>
            <tr>
              <th
                scope="col"
                className="results-selection-column"
              >
                <input
                  type="checkbox"
                  checked={allMatchingSelected}
                  disabled={selectionDisabled}
                  aria-checked={
                    someMatchingSelected ? "mixed" : allMatchingSelected
                  }
                  aria-label={text(t, "results.selection.filtered")}
                  ref={(input) => {
                    if (input !== null) {
                      input.indeterminate = someMatchingSelected;
                    }
                  }}
                  onChange={() => {
                    if (allMatchingSelected) {
                      controller.clearMatching();
                    } else {
                      controller.selectAllMatching();
                    }
                  }}
                />
              </th>
              <SortHeader
                field="path"
                label={t("analysis.results.path")}
                controller={controller}
              />
              <SortHeader
                field="extension"
                label={text(t, "results.table.extension")}
                controller={controller}
              />
              <th scope="col">{t("analysis.results.kind")}</th>
              <SortHeader
                field="method"
                label={t("analysis.results.method")}
                controller={controller}
              />
              <SortHeader
                field="size"
                label={t("analysis.results.size")}
                controller={controller}
              />
              <SortHeader
                field="state"
                label={t("analysis.results.state")}
                controller={controller}
              />
              <SortHeader
                field="confidence"
                label={t("analysis.results.confidence")}
                controller={controller}
              />
              <SortHeader
                field="recoverabilityScore"
                label={t("analysis.results.score")}
                controller={controller}
              />
              <th scope="col">{text(t, "results.table.eligibility")}</th>
              <th scope="col">{text(t, "results.table.warnings")}</th>
            </tr>
          </thead>
          <tbody>
            {candidates.map((candidate) => (
              <CandidateRow
                key={candidate.id}
                candidate={candidate}
                controller={controller}
                locale={locale}
                t={t}
              />
            ))}
          </tbody>
        </table>
      </div>

      <nav
        className="results-pagination"
        aria-label={text(t, "results.pagination.page")}
      >
        <button
          type="button"
          className="button"
          disabled={
            controller.state.currentPageIndex === 0 ||
            controller.state.phase === "loading" ||
            controller.state.selectionPhase === "updating"
          }
          onClick={controller.previousPage}
        >
          {text(t, "results.pagination.previous")}
        </button>
        <span>
          {text(t, "results.pagination.page")}{" "}
          {formatInteger(
            BigInt(controller.state.currentPageIndex + 1),
            locale,
          )}
        </span>
        <button
          type="button"
          className="button"
          disabled={
            page.nextCursor === null ||
            controller.state.phase === "loading" ||
            controller.state.selectionPhase === "updating"
          }
          onClick={controller.nextPage}
        >
          {text(t, "results.pagination.next")}
        </button>
      </nav>
    </div>
  );
}
