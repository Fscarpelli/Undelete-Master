import {
  AlertTriangle,
  Ban,
  CheckCircle2,
  FolderOpen,
  LoaderCircle,
  Play,
  ShieldCheck,
  X,
} from "lucide-react";
import {
  useEffect,
  useRef,
  type RefObject,
} from "react";
import type {
  CandidateSelectionSummary,
  RestoreDestinationSummary,
  RestoreJobSnapshot,
  RestorePlanSummary,
} from "../../api/storage";
import { formatBytes, formatInteger } from "../../format";
import type { Locale, MessageKey } from "../../i18n/messages";
import type { RestoreWorkflowController } from "../../state/restoreWorkflow";

export interface RestoreWorkflowDialogProps {
  controller: RestoreWorkflowController;
  selection: CandidateSelectionSummary | null;
  locale: Locale;
  t: (key: MessageKey) => string;
  recoverButtonRef: RefObject<HTMLButtonElement>;
}

function text(t: RestoreWorkflowDialogProps["t"], key: string): string {
  return t(key as MessageKey);
}

function isNonTerminalJob(job: RestoreJobSnapshot | null): boolean {
  return (
    job !== null &&
    (job.status === "queued" ||
      job.status === "running" ||
      job.status === "cancelling")
  );
}

const setupErrorCodes = new Set([
  "RESTORE_ITEM_INELIGIBLE",
  "RESTORE_PARTIAL_POLICY_REQUIRED",
  "RESTORE_SELECTION_STALE",
]);

function DestinationFacts({
  destination,
  locale,
  t,
}: {
  destination: RestoreDestinationSummary;
  locale: Locale;
  t: RestoreWorkflowDialogProps["t"];
}) {
  return (
    <dl className="restore-destination-summary">
      <div>
        <dt>{text(t, "restore.destination.selected")}</dt>
        <dd>
          <bdi dir="auto">{destination.label}</bdi>
        </dd>
      </div>
      <div>
        <dt>{text(t, "restore.destination.volume")}</dt>
        <dd>
          <bdi dir="auto">{destination.volumeLabel}</bdi>
        </dd>
      </div>
      <div>
        <dt>{text(t, "restore.destination.filesystem")}</dt>
        <dd>{destination.fileSystem}</dd>
      </div>
      <div>
        <dt>{text(t, "restore.destination.freeBytes")}</dt>
        <dd>{formatBytes(destination.freeBytes, locale)}</dd>
      </div>
    </dl>
  );
}

function DestinationSetup({
  controller,
  selection,
  locale,
  t,
  headingRef,
  busy,
}: {
  controller: RestoreWorkflowController;
  selection: CandidateSelectionSummary | null;
  locale: Locale;
  t: RestoreWorkflowDialogProps["t"];
  headingRef: RefObject<HTMLHeadingElement>;
  busy: boolean;
}) {
  const { destination, partialFilePolicy, bestEffortConsent } =
    controller.state;
  if (destination === null) {
    return (
      <div className="restore-dialog-step" role="status">
        <LoaderCircle className="spinner" size={24} aria-hidden="true" />
        {text(t, "restore.destination.selecting")}
      </div>
    );
  }

  const requiresBestEffort =
    BigInt(selection?.bestEffortCandidates ?? "0") > 0n;
  const hasIneligible =
    BigInt(selection?.ineligibleCandidates ?? "0") > 0n;
  const selectionCurrent =
    selection !== null &&
    BigInt(selection.selectedCandidates) > 0n &&
    controller.state.selectionRevision === selection.selectionRevision;
  const partialChoiceComplete =
    !requiresBestEffort ||
    (partialFilePolicy === "zeroFillAndMap" && bestEffortConsent);

  return (
    <div className="restore-dialog-step">
      <h2 ref={headingRef} tabIndex={-1}>
        {text(t, "restore.setup.title")}
      </h2>
      <p>{text(t, "restore.setup.body")}</p>
      {controller.state.error === null ? null : (
        <p className="error-message" role="alert">
          {text(t, `error.${controller.state.error}`)}
        </p>
      )}
      <DestinationFacts destination={destination} locale={locale} t={t} />
      <button
        type="button"
        className="button"
        disabled={busy || controller.state.phase === "error"}
        onClick={controller.chooseDestination}
      >
        <FolderOpen size={16} aria-hidden="true" />
        {text(t, "restore.destination.change")}
      </button>

      <fieldset className="restore-policy-options">
        <legend>{text(t, "restore.policy.title")}</legend>
        <label className="restore-policy-option">
          <input
            type="radio"
            name="restore-partial-policy"
            value="completeOnly"
            checked={partialFilePolicy === "completeOnly"}
            disabled={busy}
            onChange={() =>
              controller.setPartialFilePolicy("completeOnly")
            }
          />
          <span>
            <strong>{text(t, "restore.policy.completeOnly")}</strong>
            <small>{text(t, "restore.policy.completeOnlyBody")}</small>
          </span>
        </label>
        <label className="restore-policy-option">
          <input
            type="radio"
            name="restore-partial-policy"
            value="zeroFillAndMap"
            checked={partialFilePolicy === "zeroFillAndMap"}
            disabled={busy}
            onChange={() =>
              controller.setPartialFilePolicy("zeroFillAndMap")
            }
          />
          <span>
            <strong>{text(t, "restore.policy.zeroFillAndMap")}</strong>
            <small>{text(t, "restore.policy.zeroFillAndMapBody")}</small>
          </span>
        </label>
      </fieldset>

      {partialFilePolicy === "zeroFillAndMap" ? (
        <label className="restore-best-effort-consent">
          <input
            type="checkbox"
            checked={bestEffortConsent}
            disabled={busy}
            onChange={(event) =>
              controller.setBestEffortConsent(event.currentTarget.checked)
            }
          />
          <span>{text(t, "restore.policy.consent")}</span>
        </label>
      ) : null}

      <div className="restore-dialog-actions">
        <button
          type="button"
          className="button button-primary"
          disabled={
            !partialChoiceComplete ||
            hasIneligible ||
            !selectionCurrent ||
            busy ||
            controller.state.phase === "error"
          }
          onClick={controller.createPlan}
        >
          {busy ? (
            <LoaderCircle className="spinner" size={17} aria-hidden="true" />
          ) : (
            <ShieldCheck size={17} aria-hidden="true" />
          )}
          {text(t, "restore.setup.review")}
        </button>
      </div>
    </div>
  );
}

function PlanReview({
  controller,
  plan,
  destination,
  locale,
  t,
  headingRef,
}: {
  controller: RestoreWorkflowController;
  plan: RestorePlanSummary;
  destination: RestoreDestinationSummary;
  locale: Locale;
  t: RestoreWorkflowDialogProps["t"];
  headingRef: RefObject<HTMLHeadingElement>;
}) {
  return (
    <div className="restore-dialog-step">
      <h2 ref={headingRef} tabIndex={-1}>
        {text(t, "restore.review.title")}
      </h2>
      <DestinationFacts destination={destination} locale={locale} t={t} />
      <dl className="restore-plan-summary">
        <div>
          <dt>{text(t, "restore.review.items")}</dt>
          <dd>{formatInteger(plan.itemsTotal, locale)}</dd>
        </div>
        <div>
          <dt>{text(t, "restore.review.files")}</dt>
          <dd>{formatInteger(plan.filesTotal, locale)}</dd>
        </div>
        <div>
          <dt>{text(t, "restore.review.directories")}</dt>
          <dd>{formatInteger(plan.directoriesTotal, locale)}</dd>
        </div>
        <div>
          <dt>{text(t, "restore.review.bytes")}</dt>
          <dd>{formatBytes(plan.logicalBytes, locale)}</dd>
        </div>
        <div>
          <dt>{text(t, "restore.review.bestEffort")}</dt>
          <dd>{formatInteger(plan.bestEffortItems, locale)}</dd>
        </div>
        <div>
          <dt>{text(t, "restore.review.collision")}</dt>
          <dd>{text(t, "restore.review.collisionRename")}</dd>
        </div>
        <div className="restore-plan-digest">
          <dt>{text(t, "restore.review.digest")}</dt>
          <dd>
            <bdi dir="ltr">{plan.planDigest}</bdi>
          </dd>
        </div>
      </dl>
      <div className="restore-dialog-actions">
        <button
          type="button"
          className="button button-primary"
          onClick={controller.start}
        >
          <Play size={17} aria-hidden="true" />
          {text(t, "restore.review.start")}
        </button>
      </div>
    </div>
  );
}

function ProgressFacts({
  job,
  locale,
  t,
}: {
  job: RestoreJobSnapshot;
  locale: Locale;
  t: RestoreWorkflowDialogProps["t"];
}) {
  return (
    <dl className="restore-progress-facts">
      <div>
        <dt>{text(t, "restore.progress.completed")}</dt>
        <dd>
          {formatInteger(job.itemsCompleted, locale)} /{" "}
          {formatInteger(job.itemsTotal, locale)}
        </dd>
      </div>
      <div>
        <dt>{text(t, "restore.progress.bytes")}</dt>
        <dd>
          {formatBytes(job.bytesCompleted, locale)} /{" "}
          {formatBytes(job.bytesTotal, locale)}
        </dd>
      </div>
      <div>
        <dt>{text(t, "restore.progress.failed")}</dt>
        <dd>{formatInteger(job.itemsFailed, locale)}</dd>
      </div>
      <div>
        <dt>{text(t, "restore.progress.cancelled")}</dt>
        <dd>{formatInteger(job.itemsCancelled, locale)}</dd>
      </div>
      {job.currentItem === null ? null : (
        <div>
          <dt>{text(t, "restore.progress.current")}</dt>
          <dd>{formatInteger(job.currentItem.ordinal, locale)}</dd>
        </div>
      )}
    </dl>
  );
}

function RestoreProgress({
  controller,
  locale,
  t,
  headingRef,
}: {
  controller: RestoreWorkflowController;
  locale: Locale;
  t: RestoreWorkflowDialogProps["t"];
  headingRef: RefObject<HTMLHeadingElement>;
}) {
  const {
    job,
    progressBasisPoints,
    cancelRequested,
    operationPhase,
  } = controller.state;
  const cancelling =
    cancelRequested ||
    job?.status === "cancelling" ||
    operationPhase === "cancelRequest";

  return (
    <div className="restore-dialog-step">
      <h2 ref={headingRef} tabIndex={-1}>
        {text(t, "restore.progress.title")}
      </h2>
      {controller.state.error === null ? null : (
        <p className="error-message" role="alert">
          {text(t, `error.${controller.state.error}`)}
        </p>
      )}
      <progress
        className="restore-progress"
        aria-label={text(t, "restore.progress.label")}
        max={10_000}
        {...(job === null ? {} : { value: progressBasisPoints })}
      />
      {job === null ? (
        <p role="status">{text(t, "restore.progress.title")}</p>
      ) : (
        <>
          <ProgressFacts job={job} locale={locale} t={t} />
          {job.warnings.length === 0 ? null : (
            <section
              className="restore-progress-warnings"
              aria-labelledby="restore-progress-warnings"
            >
              <h3 id="restore-progress-warnings">
                {text(t, "restore.progress.warnings")}
              </h3>
              <ul>
                {job.warnings.map((warning, index) => (
                  <li key={`${index.toString()}-${warning}`}>
                    <AlertTriangle size={14} aria-hidden="true" />
                    <bdi dir="auto">{warning}</bdi>
                  </li>
                ))}
              </ul>
            </section>
          )}
        </>
      )}
      <div className="restore-dialog-actions">
        <button
          type="button"
          className="button"
          disabled={job === null || cancelling}
          onClick={controller.cancel}
        >
          {cancelling ? (
            <LoaderCircle className="spinner" size={16} aria-hidden="true" />
          ) : (
            <Ban size={16} aria-hidden="true" />
          )}
          {text(
            t,
            cancelling
              ? "restore.progress.cancelling"
              : "restore.progress.cancel",
          )}
        </button>
      </div>
    </div>
  );
}

function RestoreFinished({
  controller,
  job,
  locale,
  t,
  headingRef,
}: {
  controller: RestoreWorkflowController;
  job: RestoreJobSnapshot;
  locale: Locale;
  t: RestoreWorkflowDialogProps["t"];
  headingRef: RefObject<HTMLHeadingElement>;
}) {
  const manifest = job.manifest;
  const completed = job.status === "completed";
  const statusKey =
    job.status === "completed"
      ? "restore.finish.completed"
      : job.status === "failed"
        ? "restore.finish.failed"
        : job.status === "cancelled"
          ? "restore.finish.cancelled"
          : null;
  const opening = controller.state.operationPhase === "opening";
  return (
    <div className="restore-dialog-step">
      <div className="restore-finish-heading">
        {completed ? (
          <CheckCircle2 size={24} aria-hidden="true" />
        ) : (
          <AlertTriangle size={24} aria-hidden="true" />
        )}
        <h2 ref={headingRef} tabIndex={-1}>
          {text(t, "restore.finish.title")}
        </h2>
      </div>
      {controller.state.error === null ? null : (
        <p className="error-message" role="alert">
          {text(t, `error.${controller.state.error}`)}
        </p>
      )}
      <dl className="restore-finish-summary">
        {statusKey === null ? null : (
          <div>
            <dt>{text(t, "restore.finish.status")}</dt>
            <dd>{text(t, statusKey)}</dd>
          </div>
        )}
        <div>
          <dt>{text(t, "restore.finish.itemsCompleted")}</dt>
          <dd>{formatInteger(job.itemsCompleted, locale)}</dd>
        </div>
        <div>
          <dt>{text(t, "restore.finish.itemsFailed")}</dt>
          <dd>{formatInteger(job.itemsFailed, locale)}</dd>
        </div>
        <div>
          <dt>{text(t, "restore.finish.itemsCancelled")}</dt>
          <dd>{formatInteger(job.itemsCancelled, locale)}</dd>
        </div>
        {manifest === null ? null : (
          <>
            <div>
              <dt>{text(t, "restore.finish.published")}</dt>
              <dd>{formatInteger(manifest.publishedItems, locale)}</dd>
            </div>
            <div>
              <dt>{text(t, "restore.finish.partial")}</dt>
              <dd>{formatInteger(manifest.partialItems, locale)}</dd>
            </div>
            <div>
              <dt>{text(t, "restore.finish.manifestSha256")}</dt>
              <dd>
                <bdi dir="ltr">{manifest.manifestSha256}</bdi>
              </dd>
            </div>
            <div>
              <dt>{text(t, "restore.finish.reconciliation")}</dt>
              <dd>
                {text(
                  t,
                  manifest.completionStatus === "completedDurable"
                    ? "restore.finish.durable"
                    : "restore.finish.needsReconciliation",
                )}
              </dd>
            </div>
          </>
        )}
      </dl>
      {job.warnings.length === 0 ? null : (
        <ul className="restore-progress-warnings">
          {job.warnings.map((warning, index) => (
            <li key={`${index.toString()}-${warning}`}>
              <AlertTriangle size={14} aria-hidden="true" />
              <bdi dir="auto">{warning}</bdi>
            </li>
          ))}
        </ul>
      )}
      <div className="restore-dialog-actions">
        {completed && manifest !== null ? (
          <button
            type="button"
            className="button button-primary"
            disabled={opening}
            onClick={controller.openDestination}
          >
            {opening ? (
              <LoaderCircle
                className="spinner"
                size={17}
                aria-hidden="true"
              />
            ) : (
              <FolderOpen size={17} aria-hidden="true" />
            )}
            {text(t, "restore.finish.openDestination")}
          </button>
        ) : null}
        <button
          type="button"
          className="button"
          disabled={opening}
          onClick={controller.close}
        >
          {text(t, "restore.finish.done")}
        </button>
      </div>
    </div>
  );
}

function RestoreError({
  controller,
  t,
  headingRef,
}: {
  controller: RestoreWorkflowController;
  t: RestoreWorkflowDialogProps["t"];
  headingRef: RefObject<HTMLHeadingElement>;
}) {
  const destinationErrors = new Set([
    "RESTORE_DESTINATION_INVALID",
    "RESTORE_DESTINATION_LIMIT",
    "RESTORE_DESTINATION_EXPIRED",
    "RESTORE_DIFFERENT_DISK_REQUIRED",
  ]);
  const canReselectDestination =
    controller.state.error !== null &&
    destinationErrors.has(controller.state.error);
  const trackingLost = controller.state.phase === "trackingLost";
  const activeJob =
    !trackingLost && isNonTerminalJob(controller.state.job);
  const cancelling =
    controller.state.operationPhase === "cancelRequest" ||
    controller.state.cancelRequested ||
    controller.state.job?.status === "cancelling";
  return (
    <div className="restore-dialog-step">
      <div className="restore-error-heading">
        <AlertTriangle size={24} aria-hidden="true" />
        <h2 ref={headingRef} tabIndex={-1}>
          {text(
            t,
            trackingLost
              ? "restore.trackingLost.title"
              : "restore.error.title",
          )}
        </h2>
      </div>
      {trackingLost ? (
        <p className="error-message" role="alert">
          {text(t, "restore.trackingLost.body")}
        </p>
      ) : controller.state.error === null ? null : (
        <p className="error-message" role="alert">
          {text(t, `error.${controller.state.error}`)}
        </p>
      )}
      <div className="restore-dialog-actions">
        {activeJob ? (
          <button
            type="button"
            className="button"
            disabled={cancelling}
            onClick={controller.cancel}
          >
            {cancelling ? (
              <LoaderCircle
                className="spinner"
                size={16}
                aria-hidden="true"
              />
            ) : (
              <Ban size={16} aria-hidden="true" />
            )}
            {text(
              t,
              cancelling
                ? "restore.progress.cancelling"
                : "restore.progress.cancel",
            )}
          </button>
        ) : canReselectDestination ? (
          <button
            type="button"
            className="button"
            onClick={controller.chooseDestination}
          >
            <FolderOpen size={16} aria-hidden="true" />
            {text(t, "restore.error.retryDestination")}
          </button>
        ) : null}
        {activeJob ? null : (
          <button
            type="button"
            className="button"
            onClick={controller.close}
          >
            {text(t, "restore.dialog.close")}
          </button>
        )}
      </div>
    </div>
  );
}

export function RestoreWorkflowDialog({
  controller,
  selection,
  locale,
  t,
  recoverButtonRef,
}: RestoreWorkflowDialogProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const headingRef = useRef<HTMLHeadingElement>(null);
  const wasOpen = useRef(false);
  const open = controller.state.phase !== "idle";

  useEffect(() => {
    const dialog = dialogRef.current;
    if (dialog === null) {
      return;
    }

    if (open) {
      if (!dialog.open) {
        dialog.showModal();
      }
      headingRef.current?.focus();
    } else {
      if (dialog.open) {
        dialog.close();
      }
      if (wasOpen.current) {
        recoverButtonRef.current?.focus();
      }
    }
    wasOpen.current = open;
  }, [controller.state.phase, open, recoverButtonRef]);

  const phase = controller.state.phase;
  const recoverableSetupError =
    phase === "error" &&
    controller.state.destination !== null &&
    controller.state.error !== null &&
    setupErrorCodes.has(controller.state.error);
  const busy =
    phase === "selectingDestination" ||
    phase === "planning" ||
    phase === "starting" ||
    phase === "active" ||
    controller.state.operationPhase === "opening" ||
    (phase !== "trackingLost" &&
      isNonTerminalJob(controller.state.job));

  return (
    <dialog
      ref={dialogRef}
      className="restore-dialog"
      aria-labelledby="restore-dialog-title"
      onCancel={(event) => {
        event.preventDefault();
        if (phase === "active" || phase === "starting") {
          controller.cancel();
        } else if (!busy) {
          controller.close();
        }
      }}
    >
      <div className="restore-dialog-panel">
        <header className="restore-dialog-header">
          <h1 id="restore-dialog-title">
            {text(t, "restore.dialog.title")}
          </h1>
          {busy ? null : (
            <button
              type="button"
              className="icon-button"
              aria-label={text(t, "restore.dialog.close")}
              onClick={controller.close}
            >
              <X size={18} aria-hidden="true" />
            </button>
          )}
        </header>

        {phase === "selectingDestination" ||
        phase === "setup" ||
        phase === "planning" ||
        recoverableSetupError ? (
          <DestinationSetup
            controller={controller}
            selection={selection}
            locale={locale}
            t={t}
            headingRef={headingRef}
            busy={phase === "planning"}
          />
        ) : null}
        {phase === "review" &&
        controller.state.plan !== null &&
        controller.state.destination !== null ? (
          <PlanReview
            controller={controller}
            plan={controller.state.plan}
            destination={controller.state.destination}
            locale={locale}
            t={t}
            headingRef={headingRef}
          />
        ) : null}
        {phase === "starting" || phase === "active" ? (
          <RestoreProgress
            controller={controller}
            locale={locale}
            t={t}
            headingRef={headingRef}
          />
        ) : null}
        {phase === "finished" && controller.state.job !== null ? (
          <RestoreFinished
            controller={controller}
            job={controller.state.job}
            locale={locale}
            t={t}
            headingRef={headingRef}
          />
        ) : null}
        {(phase === "error" && !recoverableSetupError) ||
        phase === "trackingLost" ? (
          <RestoreError
            controller={controller}
            t={t}
            headingRef={headingRef}
          />
        ) : null}
      </div>
    </dialog>
  );
}
