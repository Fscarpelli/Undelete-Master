import {
  AlertTriangle,
  CheckCircle2,
  File,
  Folder,
  FolderOpen,
  HardDrive,
  LoaderCircle,
  RefreshCw,
  ScanSearch,
  ShieldCheck,
  X,
} from "lucide-react";
import { useEffect, useRef } from "react";
import type {
  BusType,
  CandidateKind,
  CandidateRow,
  CandidateState,
  MetadataConfidence,
  StorageDisk,
  StorageVolume,
} from "../api/storage";
import { formatBytes, formatInteger } from "../format";
import type { Locale, MessageKey } from "../i18n/messages";
import type { StorageWorkflowState } from "../state/storageScan";

export interface AnalysisViewProps {
  runtimeAvailable: boolean;
  locale: Locale;
  t: (key: MessageKey) => string;
  state: StorageWorkflowState;
  refreshInventory: () => Promise<void>;
  selectVolume: (volumeId: string) => void;
  selectFolder: () => Promise<void>;
  clearFolder: () => void;
  startScan: () => Promise<void>;
  loadMore: () => Promise<void>;
  resetScan: () => void;
}

const busLabels: Record<BusType, MessageKey> = {
  unknown: "bus.unknown",
  ata: "bus.ata",
  sata: "bus.sata",
  scsi: "bus.scsi",
  usb: "bus.usb",
  nvme: "bus.nvme",
  virtual: "bus.virtual",
};

const kindLabels: Record<CandidateKind, MessageKey> = {
  file: "candidate.kind.file",
  directory: "candidate.kind.directory",
};

const stateLabels: Record<CandidateState, MessageKey> = {
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

const confidenceLabels: Record<MetadataConfidence, MessageKey> = {
  high: "candidate.confidence.high",
  medium: "candidate.confidence.medium",
  low: "candidate.confidence.low",
};

function selectedVolume(state: StorageWorkflowState): StorageVolume | null {
  if (state.inventory === null || state.selectedVolumeId === null) {
    return null;
  }
  for (const disk of state.inventory.disks) {
    const volume = disk.volumes.find(
      (item) => item.id === state.selectedVolumeId,
    );
    if (volume !== undefined) {
      return volume;
    }
  }
  return null;
}

function PageHeader({
  t,
}: {
  t: (key: MessageKey) => string;
}) {
  return (
    <header className="page-header storage-page-header">
      <div>
        <h1 data-view-heading tabIndex={-1}>
          {t("analysis.title")}
        </h1>
        <p>{t("analysis.subtitle")}</p>
      </div>
    </header>
  );
}

function RuntimeUnavailable({
  t,
}: {
  t: (key: MessageKey) => string;
}) {
  return (
    <div className="page storage-page">
      <PageHeader t={t} />
      <section className="unavailable-panel" role="status">
        <AlertTriangle size={26} aria-hidden="true" />
        <div>
          <h2>{t("analysis.runtime.title")}</h2>
          <p>{t("analysis.runtime.body")}</p>
        </div>
      </section>
    </div>
  );
}

function InventoryLoading({
  t,
}: {
  t: (key: MessageKey) => string;
}) {
  return (
    <section className="task-panel pending-panel" role="status">
      <LoaderCircle className="spinner" size={30} aria-hidden="true" />
      <div>
        <h2>{t("analysis.inventory.loadingTitle")}</h2>
        <p>{t("analysis.inventory.loadingBody")}</p>
      </div>
      <div
        className="indeterminate-track"
        role="progressbar"
        aria-label={t("analysis.inventory.loadingTitle")}
      >
        <span />
      </div>
    </section>
  );
}

function RefreshButton({
  refreshing,
  refreshInventory,
  t,
}: {
  refreshing: boolean;
  refreshInventory: () => Promise<void>;
  t: (key: MessageKey) => string;
}) {
  return (
    <button
      type="button"
      className="button"
      disabled={refreshing}
      onClick={() => void refreshInventory()}
    >
      <RefreshCw
        className={refreshing ? "spinner" : undefined}
        size={17}
        aria-hidden="true"
      />
      {refreshing
        ? t("analysis.inventory.refreshing")
        : t("analysis.inventory.refresh")}
    </button>
  );
}

function InventoryError({
  code,
  refreshInventory,
  t,
}: {
  code: NonNullable<StorageWorkflowState["inventoryError"]>;
  refreshInventory: () => Promise<void>;
  t: (key: MessageKey) => string;
}) {
  return (
    <section className="task-panel">
      <AlertTriangle size={30} aria-hidden="true" />
      <div>
        <h2>{t("analysis.inventory.errorTitle")}</h2>
        <div className="error-message" role="alert">
          <span>{t(`error.${code}`)}</span>
        </div>
      </div>
      <RefreshButton
        refreshing={false}
        refreshInventory={refreshInventory}
        t={t}
      />
    </section>
  );
}

function VolumeWarnings({ warnings }: { warnings: string[] }) {
  if (warnings.length === 0) {
    return null;
  }
  return (
    <ul className="volume-warning-list">
      {warnings.map((warning, index) => (
        <li key={`${index.toString()}-${warning}`}>{warning}</li>
      ))}
    </ul>
  );
}

function VolumeOption({
  volume,
  selected,
  disabled,
  locale,
  t,
  selectVolume,
}: {
  volume: StorageVolume;
  selected: boolean;
  disabled: boolean;
  locale: Locale;
  t: (key: MessageKey) => string;
  selectVolume: (volumeId: string) => void;
}) {
  const displayLabel =
    volume.label.length > 0 ? volume.label : t("analysis.volume.unnamed");
  return (
    <label
      className="volume-option"
      data-selected={selected ? "true" : "false"}
      data-disabled={disabled ? "true" : "false"}
    >
      <input
        type="radio"
        name="storage-volume"
        value={volume.id}
        checked={selected}
        disabled={disabled}
        onChange={() => selectVolume(volume.id)}
      />
      <span className="volume-main">
        <span className="volume-title">
          <strong>
            <bdi dir="auto">{displayLabel}</bdi>{" "}
            <span className="mount-label">{volume.mountLabel}</span>
          </strong>
          {volume.isSystem ? (
            <span className="volume-badge">{t("analysis.volume.system")}</span>
          ) : null}
          {!volume.scanSupported ? (
            <span className="volume-badge volume-badge-warning">
              {t("analysis.volume.unsupported")}
            </span>
          ) : null}
        </span>
        <span className="volume-facts">
          <span>{volume.fileSystem.toLocaleUpperCase("en-US")}</span>
          <span>{formatBytes(volume.sizeBytes, locale)}</span>
          <span>
            {formatBytes(volume.freeBytes, locale)}{" "}
            {t("analysis.volume.free")}
          </span>
        </span>
        <VolumeWarnings warnings={volume.warnings} />
      </span>
    </label>
  );
}

function DiskCard({
  disk,
  state,
  locale,
  t,
  selectVolume,
}: {
  disk: StorageDisk;
  state: StorageWorkflowState;
  locale: Locale;
  t: (key: MessageKey) => string;
  selectVolume: (volumeId: string) => void;
}) {
  const interactionsDisabled =
    state.scanPhase === "scanning" || state.folderPhase === "selecting";
  return (
    <section className="disk-card" aria-labelledby={`disk-${disk.id}`}>
      <header className="disk-header">
        <div className="disk-heading">
          <HardDrive size={21} aria-hidden="true" />
          <div>
            <h2 id={`disk-${disk.id}`}>
              <bdi dir="auto">{disk.displayName}</bdi>
            </h2>
            <span>{formatBytes(disk.sizeBytes, locale)}</span>
          </div>
        </div>
        <span className="bus-badge">{t(busLabels[disk.busType])}</span>
      </header>
      {disk.volumes.length === 0 ? (
        <p className="quiet-state">{t("analysis.disk.noVolumes")}</p>
      ) : (
        <fieldset className="volume-list">
          <legend className="visually-hidden">
            {t("analysis.volume.choose")}
          </legend>
          {disk.volumes.map((volume) => (
            <VolumeOption
              key={volume.id}
              volume={volume}
              selected={state.selectedVolumeId === volume.id}
              disabled={!volume.scanSupported || interactionsDisabled}
              locale={locale}
              t={t}
              selectVolume={selectVolume}
            />
          ))}
        </fieldset>
      )}
    </section>
  );
}

function ScopeControls({
  state,
  volume,
  selectFolder,
  clearFolder,
  startScan,
  t,
}: {
  state: StorageWorkflowState;
  volume: StorageVolume;
  selectFolder: () => Promise<void>;
  clearFolder: () => void;
  startScan: () => Promise<void>;
  t: (key: MessageKey) => string;
}) {
  const selecting = state.folderPhase === "selecting";
  return (
    <section className="scope-panel" aria-labelledby="scope-heading">
      <div className="scope-copy">
        <h2 id="scope-heading">{t("analysis.scope.title")}</h2>
        <p>
          {state.folderSelection === null
            ? t("analysis.scope.wholeVolume")
            : t("analysis.scope.folderBody")}
        </p>
      </div>

      {state.folderSelection === null ? null : (
        <div className="selected-scope">
          <Folder size={18} aria-hidden="true" />
          <span>
            {t("analysis.scope.folderLabel")}:{" "}
            <strong>
              <bdi dir="auto">{state.folderSelection.label}</bdi>
            </strong>
          </span>
          <button
            type="button"
            className="icon-button"
            aria-label={t("analysis.scope.clearFolder")}
            onClick={clearFolder}
          >
            <X size={17} aria-hidden="true" />
          </button>
        </div>
      )}

      {state.folderCancelled ? (
        <p className="inline-notice" role="status">
          {t(
            state.folderSelection === null
              ? "analysis.scope.folderCancelled"
              : "analysis.scope.folderChangeCancelled",
          )}
        </p>
      ) : null}

      {!volume.folderScopeSupported ? (
        <p className="scope-limitation">
          <AlertTriangle size={16} aria-hidden="true" />
          {t("analysis.scope.unsupported")}
        </p>
      ) : null}

      {state.scanPhase === "error" && state.scanError !== null ? (
        <div className="error-message" role="alert">
          <strong>{t("analysis.scan.errorTitle")}</strong>
          <span>{t(`error.${state.scanError}`)}</span>
        </div>
      ) : null}

      <div className="scope-actions">
        {volume.folderScopeSupported ? (
          <button
            type="button"
            className="button"
            disabled={selecting}
            onClick={() => void selectFolder()}
          >
            {selecting ? (
              <LoaderCircle className="spinner" size={17} aria-hidden="true" />
            ) : (
              <FolderOpen size={17} aria-hidden="true" />
            )}
            {state.folderSelection === null
              ? t("analysis.scope.chooseFolder")
              : t("analysis.scope.changeFolder")}
          </button>
        ) : null}
        <button
          type="button"
          className="button button-primary"
          disabled={selecting}
          onClick={() => void startScan()}
        >
          <ScanSearch size={18} aria-hidden="true" />
          {t("analysis.scan.start")}
        </button>
      </div>
    </section>
  );
}

function InventoryView({
  state,
  locale,
  t,
  refreshInventory,
  selectVolume,
  selectFolder,
  clearFolder,
  startScan,
}: Omit<
  AnalysisViewProps,
  "runtimeAvailable" | "loadMore" | "resetScan"
>) {
  const volume = selectedVolume(state);
  const refreshing = state.inventoryPhase === "refreshing";
  const disks = state.inventory?.disks ?? [];
  return (
    <div className="page storage-page">
      <PageHeader t={t} />

      <section className="safety-banner" aria-labelledby="safety-heading">
        <ShieldCheck size={21} aria-hidden="true" />
        <div>
          <h2 id="safety-heading">{t("analysis.safety.title")}</h2>
          <p>{t("analysis.safety.body")}</p>
        </div>
      </section>

      <div className="inventory-toolbar">
        <div>
          <h2>{t("analysis.inventory.title")}</h2>
          <p>{t("analysis.inventory.body")}</p>
        </div>
        <RefreshButton
          refreshing={refreshing}
          refreshInventory={refreshInventory}
          t={t}
        />
      </div>

      {disks.length === 0 ? (
        <section className="empty-inventory" role="status">
          <HardDrive size={28} aria-hidden="true" />
          <div>
            <h2>{t("analysis.inventory.emptyTitle")}</h2>
            <p>{t("analysis.inventory.emptyBody")}</p>
          </div>
        </section>
      ) : (
        <div className="disk-list">
          {disks.map((disk) => (
            <DiskCard
              key={disk.id}
              disk={disk}
              state={state}
              locale={locale}
              t={t}
              selectVolume={selectVolume}
            />
          ))}
        </div>
      )}

      {volume === null ? null : (
        <ScopeControls
          state={state}
          volume={volume}
          selectFolder={selectFolder}
          clearFolder={clearFolder}
          startScan={startScan}
          t={t}
        />
      )}
    </div>
  );
}

function ScanPending({
  t,
}: {
  t: (key: MessageKey) => string;
}) {
  return (
    <div className="page storage-page">
      <PageHeader t={t} />
      <section
        className="task-panel pending-panel"
        role="status"
        aria-live="polite"
      >
        <LoaderCircle className="spinner" size={30} aria-hidden="true" />
        <div>
          <h2>{t("analysis.scan.pendingTitle")}</h2>
          <p>{t("analysis.scan.pendingBody")}</p>
        </div>
        <div
          className="indeterminate-track"
          role="progressbar"
          aria-label={t("analysis.scan.pendingTitle")}
        >
          <span />
        </div>
      </section>
    </div>
  );
}

function CandidateRowView({
  candidate,
  locale,
  t,
}: {
  candidate: CandidateRow;
  locale: Locale;
  t: (key: MessageKey) => string;
}) {
  const KindIcon = candidate.kind === "directory" ? Folder : File;
  return (
    <tr>
      <th scope="row">
        <div className="candidate-path">
          <KindIcon size={17} aria-hidden="true" />
          <div>
            <bdi dir="auto">{candidate.displayPath}</bdi>
            {candidate.warnings.length === 0 ? null : (
              <ul className="candidate-warnings">
                {candidate.warnings.map((warning, index) => (
                  <li key={`${candidate.id}-${index.toString()}`}>{warning}</li>
                ))}
              </ul>
            )}
          </div>
        </div>
      </th>
      <td>{t(kindLabels[candidate.kind])}</td>
      <td className="number-cell">
        <span>{formatInteger(candidate.sizeBytes, locale)} B</span>
        <span className="cell-secondary">
          {formatBytes(candidate.sizeBytes, locale)}
        </span>
      </td>
      <td>
        <span className="candidate-state" data-state={candidate.state}>
          {t(stateLabels[candidate.state])}
        </span>
      </td>
      <td>{t(confidenceLabels[candidate.metadataConfidence])}</td>
      <td className="number-cell">
        {candidate.recoverabilityScore === null
          ? "—"
          : `${formatInteger(BigInt(candidate.recoverabilityScore), locale)}/100`}
      </td>
    </tr>
  );
}

function ResultWarnings({
  warnings,
  t,
}: {
  warnings: string[];
  t: (key: MessageKey) => string;
}) {
  return (
    <section className="report-section" aria-labelledby="result-warnings">
      <div className="section-heading">
        <h2 id="result-warnings">{t("analysis.results.warnings")}</h2>
      </div>
      {warnings.length === 0 ? (
        <div className="quiet-state">
          <CheckCircle2 size={18} aria-hidden="true" />
          {t("analysis.results.noWarnings")}
        </div>
      ) : (
        <ul className="warning-list">
          {warnings.map((warning, index) => (
            <li key={`${index.toString()}-${warning}`}>
              <AlertTriangle size={17} aria-hidden="true" />
              <span>{warning}</span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function ResultsView({
  state,
  locale,
  t,
  loadMore,
  resetScan,
}: Pick<
  AnalysisViewProps,
  "state" | "locale" | "t" | "loadMore" | "resetScan"
>) {
  const summary = state.summary;
  const heading = useRef<HTMLHeadingElement>(null);
  const scanId = summary?.scanId ?? "";

  useEffect(() => {
    if (scanId.length > 0) {
      heading.current?.focus();
    }
  }, [scanId]);

  if (summary === null) {
    return null;
  }

  return (
    <div className="report">
      <header className="report-header">
        <div>
          <div className="source-title">
            <HardDrive size={22} aria-hidden="true" />
            <h1 ref={heading} data-view-heading tabIndex={-1}>
              <bdi dir="auto">{summary.sourceLabel}</bdi>
            </h1>
          </div>
          <span className="read-only-seal">
            <ShieldCheck size={15} aria-hidden="true" />
            {t("analysis.results.readOnly")}
          </span>
        </div>
        <button type="button" className="button" onClick={resetScan}>
          <RefreshCw size={17} aria-hidden="true" />
          {t("analysis.results.newScan")}
        </button>
      </header>

      <div className="metric-strip result-metrics">
        <div className="metric">
          <span>{t("analysis.results.scope")}</span>
          <strong>
            <bdi dir="auto">{summary.scope.label}</bdi>
          </strong>
          <small>{t(`scope.${summary.scope.kind}`)}</small>
        </div>
        <div className="metric">
          <span>{t("analysis.results.fileSystem")}</span>
          <strong>{t(`filesystem.${summary.fileSystem}`)}</strong>
        </div>
        <div className="metric">
          <span>{t("analysis.results.total")}</span>
          <strong>{formatInteger(summary.totalCandidates, locale)}</strong>
        </div>
        <div className="metric">
          <span>{t("analysis.results.matched")}</span>
          <strong>{formatInteger(summary.matchedCandidates, locale)}</strong>
        </div>
        <div className="metric">
          <span>{t("analysis.results.unknown")}</span>
          <strong>{formatInteger(summary.unknownCandidates, locale)}</strong>
        </div>
      </div>

      {summary.scanStatus === "partial" ? (
        <section className="truth-note" role="note">
          <AlertTriangle size={18} aria-hidden="true" />
          <div>
            <strong>{t("analysis.results.partialTitle")}</strong>
            <p>{t("analysis.results.partialBody")}</p>
          </div>
        </section>
      ) : null}

      <section className="unknown-panel" aria-labelledby="unknown-heading">
        <AlertTriangle size={20} aria-hidden="true" />
        <div>
          <h2 id="unknown-heading">{t("analysis.results.unknownTitle")}</h2>
          <strong>
            {formatInteger(summary.unknownCandidates, locale)}{" "}
            {t("analysis.results.candidates")}
          </strong>
          <p>{t("analysis.results.unknownBody")}</p>
        </div>
      </section>

      <section className="report-section">
        <div className="section-heading">
          <div>
            <h2 id="candidate-heading">{t("analysis.results.table")}</h2>
            <p>
              {formatInteger(BigInt(state.candidates.length), locale)}{" "}
              {t("analysis.results.loaded")}
            </p>
          </div>
        </div>
        <p className="result-caveat" role="note">
          {t("analysis.results.caveat")}
        </p>
        {state.candidates.length === 0 ? (
          <div className="quiet-state">
            {t("analysis.results.noCandidates")}
          </div>
        ) : (
          <div
            className="table-scroll"
            role="region"
            aria-label={t("analysis.results.table")}
            tabIndex={0}
          >
            <table>
              <caption className="visually-hidden">
                {t("analysis.results.table")}
              </caption>
              <thead>
                <tr>
                  <th scope="col">{t("analysis.results.path")}</th>
                  <th scope="col">{t("analysis.results.kind")}</th>
                  <th scope="col">{t("analysis.results.size")}</th>
                  <th scope="col">{t("analysis.results.state")}</th>
                  <th scope="col">{t("analysis.results.confidence")}</th>
                  <th scope="col">{t("analysis.results.score")}</th>
                </tr>
              </thead>
              <tbody>
                {state.candidates.map((candidate) => (
                  <CandidateRowView
                    key={candidate.id}
                    candidate={candidate}
                    locale={locale}
                    t={t}
                  />
                ))}
              </tbody>
            </table>
          </div>
        )}
        {state.pageError === null ? null : (
          <div className="table-action-error error-message" role="alert">
            <span>{t(`error.${state.pageError}`)}</span>
          </div>
        )}
        {state.nextCursor === null ? null : (
          <div className="table-actions">
            <button
              type="button"
              className="button"
              disabled={state.pagePhase === "loading"}
              onClick={() => void loadMore()}
            >
              {state.pagePhase === "loading" ? (
                <LoaderCircle
                  className="spinner"
                  size={17}
                  aria-hidden="true"
                />
              ) : null}
              {state.pagePhase === "loading"
                ? t("analysis.results.loadingMore")
                : t("analysis.results.loadMore")}
            </button>
          </div>
        )}
      </section>

      <ResultWarnings warnings={summary.warnings} t={t} />
    </div>
  );
}

export function AnalysisView(props: AnalysisViewProps) {
  const { runtimeAvailable, state, t } = props;

  if (!runtimeAvailable || state.inventoryPhase === "unavailable") {
    return <RuntimeUnavailable t={t} />;
  }

  if (state.scanPhase === "success") {
    return <ResultsView {...props} />;
  }

  if (state.scanPhase === "scanning") {
    return <ScanPending t={t} />;
  }

  if (
    state.inventoryPhase === "loading" ||
    (state.inventoryPhase === "refreshing" && state.inventory === null)
  ) {
    return (
      <div className="page storage-page">
        <PageHeader t={t} />
        <InventoryLoading t={t} />
      </div>
    );
  }

  if (
    state.inventoryPhase === "error" &&
    state.inventoryError !== null
  ) {
    return (
      <div className="page storage-page">
        <PageHeader t={t} />
        <InventoryError
          code={state.inventoryError}
          refreshInventory={props.refreshInventory}
          t={t}
        />
      </div>
    );
  }

  return <InventoryView {...props} />;
}
