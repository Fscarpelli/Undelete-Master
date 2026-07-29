import {
  AlertTriangle,
  CheckCircle2,
  FileSearch2,
  FolderOpen,
  LoaderCircle,
  ShieldCheck,
} from "lucide-react";
import { useEffect, useRef } from "react";
import {
  sumCandidateCounts,
  type DesktopScanReport,
  type DesktopVolume,
  type FileSystem,
  type PartitionTable,
  type VolumeScanStatus,
} from "../api/report";
import { formatBytes, formatInteger } from "../format";
import type { Locale, MessageKey } from "../i18n/messages";
import type { ScanState } from "../state/imageScan";

interface AnalysisViewProps {
  runtimeAvailable: boolean;
  locale: Locale;
  t: (key: MessageKey) => string;
  scanState: ScanState;
  startScan: () => Promise<void>;
}

function partitionLabel(
  value: PartitionTable,
  t: (key: MessageKey) => string,
): string {
  return t(`partition.${value}`);
}

function fileSystemLabel(
  value: FileSystem,
  t: (key: MessageKey) => string,
): string {
  return t(`filesystem.${value}`);
}

function scanStatusLabel(
  value: VolumeScanStatus,
  t: (key: MessageKey) => string,
): string {
  return t(`scanStatus.${value}`);
}

function VolumeRow({
  volume,
  locale,
  t,
}: {
  volume: DesktopVolume;
  locale: Locale;
  t: (key: MessageKey) => string;
}) {
  return (
    <tr>
      <th scope="row">{`${t("analysis.report.volume")} ${formatInteger(BigInt(volume.index + 1), locale)}`}</th>
      <td>{fileSystemLabel(volume.fileSystem, t)}</td>
      <td>
        <span className="scan-status" data-status={volume.scanStatus}>
          {scanStatusLabel(volume.scanStatus, t)}
        </span>
      </td>
      <td className="number-cell">
        <span>{formatInteger(volume.offsetBytes, locale)}</span>
        <span className="cell-secondary">
          {formatBytes(volume.offsetBytes, locale)}
        </span>
      </td>
      <td className="number-cell">
        <span>{formatInteger(volume.lengthBytes, locale)}</span>
        <span className="cell-secondary">
          {formatBytes(volume.lengthBytes, locale)}
        </span>
      </td>
      <td className="number-cell">
        {formatInteger(volume.candidateCount, locale)}
      </td>
    </tr>
  );
}

function WarningList({
  report,
  locale,
  t,
}: {
  report: DesktopScanReport;
  locale: Locale;
  t: (key: MessageKey) => string;
}) {
  const warnings = [
    ...report.warnings.map((message) => ({ scope: null, message })),
    ...report.volumes.flatMap((volume) =>
      volume.warnings.map((message) => ({
        scope: `${t("analysis.report.volume")} ${formatInteger(BigInt(volume.index + 1), locale)}`,
        message,
      })),
    ),
  ];

  return (
    <section className="report-section" aria-labelledby="warning-heading">
      <div className="section-heading">
        <div>
          <h2 id="warning-heading">{t("analysis.report.warnings")}</h2>
          <p>
            {formatInteger(report.warningCount, locale)}{" "}
            {t("analysis.report.warnings").toLocaleLowerCase(locale)}
          </p>
        </div>
        {report.warningsOmitted !== "0" && (
          <span className="warning-omitted">
            {t("analysis.report.omitted")}:{" "}
            {formatInteger(report.warningsOmitted, locale)}
          </span>
        )}
      </div>
      {warnings.length === 0 ? (
        <div className="quiet-state">
          <CheckCircle2 size={18} aria-hidden="true" />
          {t("analysis.report.noWarnings")}
        </div>
      ) : (
        <ul className="warning-list">
          {warnings.map((item, index) => (
            <li key={`${item.scope ?? "image"}-${index.toString()}`}>
              <AlertTriangle size={17} aria-hidden="true" />
              <span>
                {item.scope && <strong>{item.scope}: </strong>}
                {item.message}
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function ReportView({
  report,
  locale,
  t,
  onNewScan,
}: {
  report: DesktopScanReport;
  locale: Locale;
  t: (key: MessageKey) => string;
  onNewScan: () => void;
}) {
  const candidateCount = sumCandidateCounts(report);
  const hasPartialVolume = report.volumes.some(
    (volume) => volume.scanStatus === "partial",
  );
  const sourceHeading = useRef<HTMLHeadingElement>(null);

  useEffect(() => {
    sourceHeading.current?.focus();
  }, []);

  return (
    <div className="report">
      <header className="report-header">
        <div>
          <div className="source-title">
            <FileSearch2 size={22} aria-hidden="true" />
            <h1
              ref={sourceHeading}
              data-view-heading
              tabIndex={-1}
            >
              <bdi dir="auto">{report.source.label}</bdi>
            </h1>
          </div>
          <span className="read-only-seal">
            <ShieldCheck size={15} aria-hidden="true" />
            {t("analysis.report.readOnly")}
          </span>
        </div>
        <button type="button" className="button" onClick={onNewScan}>
          <FolderOpen size={17} aria-hidden="true" />
          {t("analysis.report.new")}
        </button>
      </header>

      <div className="metric-strip">
        <div className="metric">
          <span>{t("analysis.report.sourceSize")}</span>
          <strong>{formatBytes(report.source.sizeBytes, locale)}</strong>
          <small>{formatInteger(report.source.sizeBytes, locale)} B</small>
        </div>
        <div className="metric">
          <span>{t("analysis.report.partition")}</span>
          <strong>{partitionLabel(report.partitionTable, t)}</strong>
        </div>
        <div className="metric">
          <span>{t("analysis.report.volumes")}</span>
          <strong>{formatInteger(BigInt(report.volumes.length), locale)}</strong>
        </div>
        <div className="metric">
          <span>{t("analysis.report.candidates")}</span>
          <strong>{formatInteger(candidateCount, locale)}</strong>
        </div>
      </div>

      <div className="truth-note" role="note">
        <AlertTriangle size={18} aria-hidden="true" />
        {t(
          hasPartialVolume
            ? "analysis.report.partialCaveat"
            : "analysis.report.candidateCaveat",
        )}
      </div>

      <section className="report-section" aria-labelledby="volume-heading">
        <div className="section-heading">
          <h2 id="volume-heading">{t("analysis.report.volumes")}</h2>
        </div>
        {report.volumes.length === 0 ? (
          <div className="quiet-state">{t("analysis.report.noVolumes")}</div>
        ) : (
          <div
            className="table-scroll"
            role="region"
            aria-label={t("analysis.report.volumeTable")}
            tabIndex={0}
          >
            <table>
              <caption className="visually-hidden">
                {t("analysis.report.volumeTable")}
              </caption>
              <thead>
                <tr>
                  <th scope="col">{t("analysis.report.volume")}</th>
                  <th scope="col">{t("analysis.report.fileSystem")}</th>
                  <th scope="col">{t("analysis.report.coverage")}</th>
                  <th scope="col">{t("analysis.report.offset")}</th>
                  <th scope="col">{t("analysis.report.length")}</th>
                  <th scope="col">{t("analysis.report.candidates")}</th>
                </tr>
              </thead>
              <tbody>
                {report.volumes.map((volume) => (
                  <VolumeRow
                    key={volume.index}
                    volume={volume}
                    locale={locale}
                    t={t}
                  />
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>

      <WarningList report={report} locale={locale} t={t} />
    </div>
  );
}

export function AnalysisView({
  runtimeAvailable,
  locale,
  t,
  scanState: state,
  startScan: start,
}: AnalysisViewProps) {
  if (!runtimeAvailable) {
    return (
      <div className="page narrow-page">
        <header className="page-header">
          <h1 data-view-heading tabIndex={-1}>
            {t("analysis.title")}
          </h1>
          <p>{t("analysis.subtitle")}</p>
        </header>
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

  if (state.phase === "success") {
    return (
      <ReportView
        report={state.report}
        locale={locale}
        t={t}
        onNewScan={() => void start()}
      />
    );
  }

  return (
    <div className="page narrow-page">
      <header className="page-header">
        <h1 data-view-heading tabIndex={-1}>
          {t("analysis.title")}
        </h1>
        <p>{t("analysis.subtitle")}</p>
      </header>

      {state.phase === "pending" ? (
        <section
          className="task-panel pending-panel"
          role="status"
          aria-live="polite"
        >
          <LoaderCircle className="spinner" size={30} aria-hidden="true" />
          <div>
            <h2>{t("analysis.pending.title")}</h2>
            <p>{t("analysis.pending.body")}</p>
          </div>
          <div
            className="indeterminate-track"
            role="progressbar"
            aria-label={t("analysis.pending.title")}
          >
            <span />
          </div>
        </section>
      ) : (
        <section className="task-panel">
          <FolderOpen size={30} aria-hidden="true" />
          <div>
            <h2>{t("analysis.idle.title")}</h2>
            <p>{t("analysis.idle.body")}</p>
          </div>
          {state.phase === "idle" && state.cancelled && (
            <p className="inline-notice" role="status">
              {t("analysis.cancelled")}
            </p>
          )}
          {state.phase === "error" && (
            <div className="error-message" role="alert">
              <strong>{t("analysis.error.title")}</strong>
              <span>{t(`error.${state.code}`)}</span>
            </div>
          )}
          <button
            type="button"
            className="button button-primary"
            onClick={() => void start()}
          >
            <FolderOpen size={18} aria-hidden="true" />
            {state.phase === "error"
              ? t("analysis.tryAgain")
              : t("analysis.select")}
          </button>
        </section>
      )}
    </div>
  );
}
