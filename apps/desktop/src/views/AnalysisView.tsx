import {
  AlertTriangle,
  CheckCircle2,
  Folder,
  FolderOpen,
  HardDrive,
  LoaderCircle,
  RefreshCw,
  ScanSearch,
  ShieldCheck,
  X,
} from "lucide-react";
import { useEffect, useRef, useState, type RefObject } from "react";
import type {
  BusType,
  ScanMode,
  StorageDisk,
  StorageVolume,
} from "../api/storage";
import { ResultsWorkspace } from "../components/results/ResultsWorkspace";
import { formatBytes, formatInteger } from "../format";
import type { Locale, MessageKey } from "../i18n/messages";
import type { RestoreWorkflowController } from "../state/restoreWorkflow";
import type { ResultsWorkspaceController } from "../state/resultsWorkspace";
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
  selectScanMode: (mode: ScanMode) => void;
  startScan: () => Promise<void>;
  resetScan: () => void;
  results: ResultsWorkspaceController;
  restore: RestoreWorkflowController;
  recoverButtonRef: RefObject<HTMLButtonElement>;
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
  selectScanMode,
  startScan,
  t,
}: {
  state: StorageWorkflowState;
  volume: StorageVolume;
  selectFolder: () => Promise<void>;
  clearFolder: () => void;
  selectScanMode: (mode: ScanMode) => void;
  startScan: () => Promise<void>;
  t: (key: MessageKey) => string;
}) {
  const selecting = state.folderPhase === "selecting";
  const deepJpegAvailable =
    state.folderSelection === null &&
    volume.fileSystem.toLocaleLowerCase("en-US") === "ntfs";
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

      <fieldset className="scan-mode-list">
        <legend>{t("analysis.mode.title")}</legend>
        <label
          className="scan-mode-option"
          data-selected={state.scanMode === "metadata" ? "true" : "false"}
        >
          <input
            type="radio"
            name="scan-mode"
            value="metadata"
            checked={state.scanMode === "metadata"}
            disabled={selecting}
            onChange={() => selectScanMode("metadata")}
          />
          <span>
            <strong>{t("analysis.mode.metadata.title")}</strong>
            <small>{t("analysis.mode.metadata.body")}</small>
          </span>
        </label>
        <label
          className="scan-mode-option"
          data-selected={state.scanMode === "deepJpeg" ? "true" : "false"}
          data-disabled={!deepJpegAvailable ? "true" : "false"}
        >
          <input
            type="radio"
            name="scan-mode"
            value="deepJpeg"
            checked={state.scanMode === "deepJpeg"}
            disabled={selecting || !deepJpegAvailable}
            onChange={() => selectScanMode("deepJpeg")}
          />
          <span>
            <strong>{t("analysis.mode.deepJpeg.title")}</strong>
            <small>{t("analysis.mode.deepJpeg.body")}</small>
          </span>
        </label>
      </fieldset>

      {!deepJpegAvailable ? (
        <p className="scope-limitation">
          <AlertTriangle size={16} aria-hidden="true" />
          {t(
            state.folderSelection === null
              ? "analysis.mode.ntfsBlocked"
              : "analysis.mode.folderBlocked",
          )}
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
  selectScanMode,
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
          selectScanMode={selectScanMode}
          startScan={startScan}
          t={t}
        />
      )}
    </div>
  );
}

function ScanPending({
  mode,
  scanProgress,
  scanStartedAt,
  t,
}: {
  mode: ScanMode;
  scanProgress: StorageWorkflowState["scanProgress"];
  scanStartedAt: number | null;
  t: (key: MessageKey) => string;
}) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    let active = true;
    let frame: number;
    let lastTick = Date.now();
    const tick = (timestamp: number) => {
      if (!active) {
        return;
      }
      if (timestamp - lastTick >= 1000) {
        lastTick = timestamp;
        setNow(Date.now());
      }
      frame = window.requestAnimationFrame(tick);
    };
    frame = window.requestAnimationFrame(tick);
    return () => {
      active = false;
      window.cancelAnimationFrame(frame);
    };
  }, []);
  const elapsedSeconds =
    scanStartedAt === null ? 0 : Math.max(0, Math.floor((now - scanStartedAt) / 1000));
  const completed = scanProgress === null ? 0n : BigInt(scanProgress.completed);
  const total = scanProgress === null ? 0n : BigInt(scanProgress.total);
  const determinate =
    scanProgress?.phase === "mftRecords" && total > 0n && completed <= total;
  const percent = determinate
    ? Math.min(99, Number((completed * 100n) / total))
    : 0;
  const etaSeconds =
    determinate && completed > 0n && elapsedSeconds > 0
      ? Math.max(0, Math.round((elapsedSeconds * Number(total - completed)) / Number(completed)))
      : null;
  const formatDuration = (seconds: number) => {
    const minutes = Math.floor(seconds / 60);
    const remainder = seconds % 60;
    return `${minutes.toString().padStart(2, "0")}m ${remainder.toString().padStart(2, "0")}s`;
  };
  const phaseKey = scanProgress === null
    ? "analysis.scan.phase.bootstrap"
    : `analysis.scan.phase.${scanProgress.phase}`;
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
          <p>
            {t(
              mode === "deepJpeg"
                ? "analysis.scan.pendingDeepBody"
                : "analysis.scan.pendingBody",
            )}
          </p>
          <p className="scan-phase-label">{t(phaseKey as MessageKey)}</p>
        </div>
        {determinate ? (
          <progress
            className="scan-progress-native"
            max={100}
            value={percent}
            aria-label={t("analysis.scan.pendingTitle")}
          />
        ) : (
          <div
            className="indeterminate-track"
            role="progressbar"
            aria-label={t("analysis.scan.pendingTitle")}
          >
            <span />
          </div>
        )}
        <div className="scan-progress-facts" aria-live="polite">
          <span>{t("analysis.scan.elapsed")}: <strong>{formatDuration(elapsedSeconds)}</strong></span>
          <span>
            {t("analysis.scan.eta")}: <strong>{etaSeconds === null ? t("analysis.scan.etaCalculating") : formatDuration(etaSeconds)}</strong>
          </span>
          {determinate ? <span>{percent}%</span> : <span>{t("analysis.scan.progressUnknown")}</span>}
        </div>
      </section>
    </div>
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
  results,
  restore,
  recoverButtonRef,
  resetScan,
}: Pick<
  AnalysisViewProps,
  | "state"
  | "locale"
  | "t"
  | "results"
  | "restore"
  | "recoverButtonRef"
  | "resetScan"
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
          <span>{t("analysis.results.mode")}</span>
          <strong>{t(`scanMode.${summary.scanMode}`)}</strong>
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
        {summary.mftCoverage === null ? null : (
          <div className="metric">
            <span>{t("analysis.results.mftRecordsExamined")}</span>
            <strong>
              {formatInteger(summary.mftCoverage.recordsExamined, locale)} /{" "}
              {formatInteger(summary.mftCoverage.recordsDeclared, locale)}
            </strong>
          </div>
        )}
        {summary.jpegCarveCoverage === null ? null : (
          <div className="metric">
            <span>{t("analysis.results.jpegBytesExamined")}</span>
            <strong>
              {formatBytes(summary.jpegCarveCoverage.bytesScanned, locale)} /{" "}
              {formatBytes(summary.jpegCarveCoverage.bytesRequested, locale)}
            </strong>
          </div>
        )}
      </div>

      {summary.jpegCarveCoverage === null ? null : (
        <section className="truth-note" role="note">
          <ScanSearch size={18} aria-hidden="true" />
          <div>
            <strong>{t("analysis.results.jpegCoverageTitle")}</strong>
            <p>{t("analysis.results.jpegCoverageBody")}</p>
            <ul className="coverage-facts">
              <li>
                {t("analysis.results.jpegCoverageStatus")}:{" "}
                {t(
                  summary.jpegCarveCoverage.partial
                    ? "analysis.results.jpegCoveragePartial"
                    : "analysis.results.jpegCoverageComplete",
                )}
              </li>
              <li>
                {t("analysis.results.jpegRegions")}:{" "}
                {formatInteger(
                  summary.jpegCarveCoverage.regionsSubmitted,
                  locale,
                )}
              </li>
              <li>
                {t("analysis.results.jpegSignaturesAttempted")}:{" "}
                {formatInteger(
                  summary.jpegCarveCoverage.signaturesAttempted,
                  locale,
                )}
              </li>
              <li>
                {t("analysis.results.jpegValidationBytes")}:{" "}
                {formatBytes(
                  summary.jpegCarveCoverage.validationBytesRead,
                  locale,
                )}
              </li>
              <li>
                {t("analysis.results.jpegSignatureLimit")}:{" "}
                {t(
                  summary.jpegCarveCoverage.signatureAttemptLimitReached
                    ? "analysis.results.limitReached"
                    : "analysis.results.limitNotReached",
                )}
              </li>
              <li>
                {t("analysis.results.jpegValidationLimit")}:{" "}
                {t(
                  summary.jpegCarveCoverage.validationByteLimitReached
                    ? "analysis.results.limitReached"
                    : "analysis.results.limitNotReached",
                )}
              </li>
              <li>
                {t("analysis.results.jpegRejected")}:{" "}
                {formatInteger(
                  summary.jpegCarveCoverage.rejectedSignatures,
                  locale,
                )}
              </li>
              <li>
                {t("analysis.results.jpegTruncated")}:{" "}
                {formatInteger(
                  summary.jpegCarveCoverage.truncatedSignatures,
                  locale,
                )}
              </li>
            </ul>
          </div>
        </section>
      )}

      {summary.scanStatus === "partial" ? (
        <section className="truth-note" role="note">
          <AlertTriangle size={18} aria-hidden="true" />
          <div>
            <strong>{t("analysis.results.partialTitle")}</strong>
            <p>{t("analysis.results.partialBody")}</p>
          </div>
        </section>
      ) : null}

      {summary.scope.kind === "folder" ? (
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
      ) : null}

      <p className="result-caveat" role="note">
        {t("analysis.results.caveat")}
      </p>
      <ResultsWorkspace
        summary={summary}
        controller={results}
        restore={restore}
        locale={locale}
        t={t}
        recoverButtonRef={recoverButtonRef}
      />

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
    return (
      <ScanPending
        mode={state.scanMode}
        scanProgress={state.scanProgress}
        scanStartedAt={state.scanStartedAt}
        t={t}
      />
    );
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
