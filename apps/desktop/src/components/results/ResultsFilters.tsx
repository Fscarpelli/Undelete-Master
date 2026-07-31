import { Search, SlidersHorizontal, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import type {
  CandidateKind,
  CandidateState,
  DiscoveryMethod,
  MetadataConfidence,
  RecoveryEligibility,
} from "../../api/storage";
import { formatInteger } from "../../format";
import type { Locale, MessageKey } from "../../i18n/messages";
import type { ResultsWorkspaceController } from "../../state/resultsWorkspace";

export interface ResultsFiltersProps {
  controller: ResultsWorkspaceController;
  locale: Locale;
  t: (key: MessageKey) => string;
}

const candidateKinds: readonly CandidateKind[] = ["file", "directory"];
const metadataConfidences: readonly MetadataConfidence[] = [
  "high",
  "medium",
  "low",
];
const discoveryMethods: readonly DiscoveryMethod[] = [
  "ntfsMetadata",
  "fatMetadata",
  "exfatMetadata",
  "carving",
  "recycleBin",
];
const candidateStates: readonly CandidateState[] = [
  "exactEvidence",
  "likelyComplete",
  "completeUnvalidated",
  "structurallyValid",
  "partial",
  "conflicted",
  "readError",
  "zeroedOrTrimmed",
  "overwritten",
  "metadataOnly",
  "unknown",
];
const recoveryEligibilities: readonly RecoveryEligibility[] = [
  "complete",
  "bestEffort",
  "ineligible",
];

const kindKeys: Record<CandidateKind, MessageKey> = {
  file: "candidate.kind.file",
  directory: "candidate.kind.directory",
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

function text(t: ResultsFiltersProps["t"], key: string): string {
  return t(key as MessageKey);
}

function nullableScore(value: string): number | null {
  if (value.length === 0) {
    return null;
  }
  const parsed = Number(value);
  return Number.isInteger(parsed) && parsed >= 0 && parsed <= 100
    ? parsed
    : null;
}

function FacetGroup<T extends string>({
  legend,
  values,
  selected,
  label,
  toggle,
}: {
  legend: string;
  values: readonly T[];
  selected: readonly T[];
  label: (value: T) => string;
  toggle: (value: T) => void;
}) {
  return (
    <fieldset className="results-filter-group">
      <legend>{legend}</legend>
      <div className="results-filter-options">
        {values.map((value) => (
          <label key={value} className="results-filter-option">
            <input
              type="checkbox"
              checked={selected.includes(value)}
              onChange={() => toggle(value)}
            />
            <span>{label(value)}</span>
          </label>
        ))}
      </div>
    </fieldset>
  );
}

export function ResultsFilters({
  controller,
  locale,
  t,
}: ResultsFiltersProps) {
  const { query } = controller.state;
  const [search, setSearch] = useState(query.search);
  const [extensionSearch, setExtensionSearch] = useState("");

  useEffect(() => {
    setSearch(query.search);
  }, [query.search]);

  const visibleExtensionFacets = useMemo(() => {
    const normalized = extensionSearch.trim().toLocaleLowerCase();
    return controller.state.extensionFacets
      .filter((facet) => {
        const displayExtension =
          facet.extension.length === 0
            ? text(t, "results.filters.noExtension")
            : facet.extension;
        return displayExtension.toLocaleLowerCase().includes(normalized);
      })
      .slice(0, 100);
  }, [controller.state.extensionFacets, extensionSearch, t]);

  return (
    <aside className="results-filters" aria-labelledby="results-filter-title">
      <div className="results-filter-heading">
        <SlidersHorizontal size={18} aria-hidden="true" />
        <h3 id="results-filter-title">{text(t, "results.filters.title")}</h3>
      </div>

      <form
        className="results-search"
        role="search"
        onSubmit={(event) => {
          event.preventDefault();
          controller.submitSearch(search);
        }}
      >
        <label htmlFor="results-search-input">
          {text(t, "results.search.label")}
        </label>
        <div className="results-search-control">
          <input
            id="results-search-input"
            type="search"
            maxLength={512}
            value={search}
            placeholder={text(t, "results.search.placeholder")}
            onChange={(event) => setSearch(event.currentTarget.value)}
          />
          <button type="submit" className="button button-primary">
            <Search size={16} aria-hidden="true" />
            {text(t, "results.search.submit")}
          </button>
        </div>
      </form>

      <fieldset className="results-filter-group results-extension-filter">
        <legend>{text(t, "results.filters.extensions")}</legend>
        <label htmlFor="results-extension-search">
          {text(t, "results.filters.extensionSearch")}
        </label>
        <input
          id="results-extension-search"
          type="search"
          maxLength={128}
          value={extensionSearch}
          onChange={(event) => setExtensionSearch(event.currentTarget.value)}
        />
        {query.extensions.length === 0 ? null : (
          <div
            className="results-filter-chips"
            aria-label={text(t, "results.filters.selectedExtensions")}
          >
            {query.extensions.map((extension) => {
              const displayExtension =
                extension.length === 0
                  ? text(t, "results.filters.noExtension")
                  : extension;
              return (
                <button
                  key={extension}
                  type="button"
                  className="results-filter-chip"
                  aria-label={`${text(
                    t,
                    "results.filters.removeExtension",
                  )}: ${displayExtension}`}
                  onClick={() => controller.toggleExtension(extension)}
                >
                  <bdi dir="auto">{displayExtension}</bdi>
                  <X size={14} aria-hidden="true" />
                </button>
              );
            })}
          </div>
        )}
        <div className="results-filter-options results-extension-options">
          {visibleExtensionFacets.map((facet) => {
            const displayExtension =
              facet.extension.length === 0
                ? text(t, "results.filters.noExtension")
                : facet.extension;
            return (
              <label
                key={facet.extension}
                className="results-filter-option"
              >
                <input
                  type="checkbox"
                  checked={query.extensions.includes(facet.extension)}
                  disabled={
                    query.extensions.length >= 128 &&
                    !query.extensions.includes(facet.extension)
                  }
                  onChange={() =>
                    controller.toggleExtension(facet.extension)
                  }
                />
                <span>
                  <bdi dir="auto">{displayExtension}</bdi>{" "}
                  <span className="results-facet-count">
                    {formatInteger(facet.count, locale)}
                  </span>
                </span>
              </label>
            );
          })}
        </div>
      </fieldset>

      <FacetGroup
        legend={text(t, "results.filters.kinds")}
        values={candidateKinds}
        selected={query.kinds}
        label={(value) => t(kindKeys[value])}
        toggle={controller.toggleKind}
      />
      <FacetGroup
        legend={text(t, "results.filters.confidences")}
        values={metadataConfidences}
        selected={query.metadataConfidences}
        label={(value) => t(confidenceKeys[value])}
        toggle={controller.toggleMetadataConfidence}
      />
      <FacetGroup
        legend={text(t, "results.filters.methods")}
        values={discoveryMethods}
        selected={query.methods}
        label={(value) => t(methodKeys[value])}
        toggle={controller.toggleMethod}
      />
      <FacetGroup
        legend={text(t, "results.filters.states")}
        values={candidateStates}
        selected={query.states}
        label={(value) => t(stateKeys[value])}
        toggle={controller.toggleCandidateState}
      />
      <FacetGroup
        legend={text(t, "results.filters.eligibilities")}
        values={recoveryEligibilities}
        selected={query.eligibilities}
        label={(value) => text(t, `results.eligibility.${value}`)}
        toggle={controller.toggleEligibility}
      />

      <fieldset className="results-filter-group results-score-filter">
        <legend>{text(t, "results.filters.score")}</legend>
        <div className="results-filter-options">
          <label>
            <span>{text(t, "results.filters.minScore")}</span>
            <input
              type="number"
              min={0}
              max={100}
              value={query.minRecoverabilityScore ?? ""}
              onChange={(event) =>
                controller.setScoreRange(
                  nullableScore(event.currentTarget.value),
                  query.maxRecoverabilityScore,
                )
              }
            />
          </label>
          <label>
            <span>{text(t, "results.filters.maxScore")}</span>
            <input
              type="number"
              min={0}
              max={100}
              value={query.maxRecoverabilityScore ?? ""}
              onChange={(event) =>
                controller.setScoreRange(
                  query.minRecoverabilityScore,
                  nullableScore(event.currentTarget.value),
                )
              }
            />
          </label>
        </div>
      </fieldset>

      <label className="results-filter-option results-selected-only">
        <input
          type="checkbox"
          checked={query.selectedOnly}
          onChange={(event) =>
            controller.setSelectedOnly(event.currentTarget.checked)
          }
        />
        <span>{text(t, "results.filters.selectedOnly")}</span>
      </label>
    </aside>
  );
}
