import { useCallback, useEffect, useRef, useState } from "react";
import type {
  CandidateSelectionSummary,
  PartialFilePolicy,
  RestoreCollisionPolicy,
  RestoreDestinationSummary,
  RestoreJobSnapshot,
  RestorePlanSummary,
} from "../api/storage";
import {
  cancelRestore,
  createRestorePlan,
  getRestoreJob,
  normalizeRestoreError,
  openRestoreDestination,
  RestoreCommandError,
  type RestoreErrorCode,
  selectRestoreDestination,
  startRestore,
} from "../api/storageDesktop";

const RESTORE_POLL_INTERVAL_MS = 1_000;
const MAX_CONSECUTIVE_INTERNAL_POLL_FAILURES = 3;
const DESTINATION_ERROR_CODES = new Set<RestoreErrorCode>([
  "RESTORE_DESTINATION_INVALID",
  "RESTORE_DESTINATION_LIMIT",
  "RESTORE_DESTINATION_EXPIRED",
  "RESTORE_DIFFERENT_DISK_REQUIRED",
]);

export type RestoreWorkflowPhase =
  | "idle"
  | "selectingDestination"
  | "setup"
  | "planning"
  | "review"
  | "starting"
  | "active"
  | "trackingLost"
  | "finished"
  | "error";

export type RestoreOperationPhase =
  | "idle"
  | "polling"
  | "cancelRequest"
  | "opening";

export interface RestoreWorkflowState {
  phase: RestoreWorkflowPhase;
  scanId: string | null;
  selectionRevision: string | null;
  destination: RestoreDestinationSummary | null;
  collisionPolicy: RestoreCollisionPolicy;
  partialFilePolicy: PartialFilePolicy;
  bestEffortConsent: boolean;
  plan: RestorePlanSummary | null;
  job: RestoreJobSnapshot | null;
  operationPhase: RestoreOperationPhase;
  progressBasisPoints: number;
  cancelRequested: boolean;
  error: RestoreErrorCode | null;
}

export interface RestoreWorkflowController {
  state: RestoreWorkflowState;
  chooseDestination(): void;
  setPartialFilePolicy(policy: PartialFilePolicy): void;
  setBestEffortConsent(consent: boolean): void;
  createPlan(): void;
  start(): void;
  cancel(): void;
  openDestination(): void;
  close(): void;
}

interface PollOperation {
  kind: "poll";
  generation: number;
  intent: number;
  jobId: string;
  planId: string;
}

interface CancelOperation {
  kind: "cancel";
  generation: number;
  intent: number;
  jobId: string;
  planId: string;
}

interface OpenOperation {
  kind: "open";
  generation: number;
  intent: number;
  jobId: string;
}

type RestoreOperation = PollOperation | CancelOperation | OpenOperation;

let requestSequence = 0;

function nextRequestId(): string {
  requestSequence += 1;
  return `restore-${requestSequence.toString(36)}`;
}

function initialState(
  scanId: string | null,
  selectionRevision: string | null,
): RestoreWorkflowState {
  return {
    phase: "idle",
    scanId,
    selectionRevision,
    destination: null,
    collisionPolicy: "rename",
    partialFilePolicy: "completeOnly",
    bestEffortConsent: false,
    plan: null,
    job: null,
    operationPhase: "idle",
    progressBasisPoints: 0,
    cancelRequested: false,
    error: null,
  };
}

function isTerminal(snapshot: RestoreJobSnapshot): boolean {
  return (
    snapshot.status === "completed" ||
    snapshot.status === "failed" ||
    snapshot.status === "cancelled"
  );
}

export function restoreProgressBasisPoints(
  snapshot: RestoreJobSnapshot,
): number {
  const bytesTotal = BigInt(snapshot.bytesTotal);
  if (bytesTotal > 0n) {
    const value = (BigInt(snapshot.bytesCompleted) * 10_000n) / bytesTotal;
    return Number(value > 10_000n ? 10_000n : value);
  }
  const itemsTotal = BigInt(snapshot.itemsTotal);
  if (itemsTotal === 0n) {
    return 0;
  }
  const completed =
    BigInt(snapshot.itemsCompleted) +
    BigInt(snapshot.itemsFailed) +
    BigInt(snapshot.itemsCancelled);
  const value = (completed * 10_000n) / itemsTotal;
  return Number(value > 10_000n ? 10_000n : value);
}

function hasSelectedCandidates(
  selection: CandidateSelectionSummary | null,
): selection is CandidateSelectionSummary {
  return selection !== null && BigInt(selection.selectedCandidates) > 0n;
}

export function useRestoreWorkflow(
  runtimeAvailable: boolean,
  scanId: string | null,
  selection: CandidateSelectionSummary | null,
): RestoreWorkflowController {
  const [state, setState] = useState<RestoreWorkflowState>(() =>
    initialState(scanId, selection?.selectionRevision ?? null),
  );
  const stateRef = useRef(state);
  const selectionRef = useRef(selection);
  const mountedRef = useRef(false);
  const generationRef = useRef(0);
  const operationIntentRef = useRef(0);
  const operationsRef = useRef<RestoreOperation[]>([]);
  const operationRunningRef = useRef(false);
  const activeOperationRef = useRef<RestoreOperation["kind"] | null>(null);
  const pumpOperationsRef = useRef<() => void>(() => undefined);
  const executeOperationRef = useRef<
    (operation: RestoreOperation) => Promise<void>
  >(async () => undefined);
  const pollTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pollQueuedRef = useRef(false);
  const destinationInFlightRef = useRef(false);
  const planInFlightRef = useRef(false);
  const startInFlightRef = useRef(false);
  const cancelPendingRef = useRef(false);
  const openPendingRef = useRef(false);
  const consecutiveInternalPollFailuresRef = useRef(0);

  const commitState = useCallback(
    (
      update:
        | RestoreWorkflowState
        | ((current: RestoreWorkflowState) => RestoreWorkflowState),
    ) => {
      const next =
        typeof update === "function" ? update(stateRef.current) : update;
      stateRef.current = next;
      if (mountedRef.current) {
        setState(next);
      }
    },
    [],
  );

  const clearPollTimer = useCallback(() => {
    if (pollTimerRef.current !== null) {
      clearTimeout(pollTimerRef.current);
      pollTimerRef.current = null;
    }
  }, []);

  const pumpOperations = useCallback(() => {
    if (!mountedRef.current || operationRunningRef.current) {
      return;
    }
    const operation = operationsRef.current.shift();
    if (operation === undefined) {
      return;
    }
    operationRunningRef.current = true;
    activeOperationRef.current = operation.kind;
    void executeOperationRef
      .current(operation)
      .finally(() => {
        if (operation.kind === "poll") {
          pollQueuedRef.current = false;
        }
        operationRunningRef.current = false;
        activeOperationRef.current = null;
        pumpOperationsRef.current();
      });
  }, []);

  const enqueueOperation = useCallback(
    (operation: RestoreOperation) => {
      if (operation.kind === "poll") {
        if (
          pollQueuedRef.current ||
          activeOperationRef.current === "poll"
        ) {
          return;
        }
        pollQueuedRef.current = true;
      } else if (operation.kind === "cancel") {
        const retained = operationsRef.current.filter(
          (pending) => pending.kind !== "poll",
        );
        if (retained.length !== operationsRef.current.length) {
          pollQueuedRef.current = false;
        }
        operationsRef.current = retained;
      }
      operationsRef.current.push(operation);
      pumpOperations();
    },
    [pumpOperations],
  );

  const schedulePoll = useCallback(() => {
    clearPollTimer();
    const current = stateRef.current;
    const job = current.job;
    if (
      !mountedRef.current ||
      current.phase !== "active" ||
      job === null ||
      isTerminal(job) ||
      cancelPendingRef.current
    ) {
      return;
    }
    const generation = generationRef.current;
    const intent = operationIntentRef.current;
    pollTimerRef.current = setTimeout(() => {
      pollTimerRef.current = null;
      const latest = stateRef.current;
      if (
        !mountedRef.current ||
        generation !== generationRef.current ||
        intent !== operationIntentRef.current ||
        latest.phase !== "active" ||
        latest.job?.jobId !== job.jobId
      ) {
        return;
      }
      enqueueOperation({
        kind: "poll",
        generation,
        intent,
        jobId: job.jobId,
        planId: job.planId,
      });
    }, RESTORE_POLL_INTERVAL_MS);
  }, [clearPollTimer, enqueueOperation]);

  const operationIsCurrent = useCallback(
    (operation: RestoreOperation): boolean =>
      mountedRef.current &&
      operation.generation === generationRef.current &&
      operation.intent === operationIntentRef.current,
    [],
  );

  const failWorkflow = useCallback(
    (code: RestoreErrorCode) => {
      clearPollTimer();
      consecutiveInternalPollFailuresRef.current = 0;
      commitState((current) => ({
        ...current,
        phase: "error",
        operationPhase: "idle",
        error: code,
      }));
    },
    [clearPollTimer, commitState],
  );

  const loseJobTracking = useCallback(
    (code: RestoreErrorCode) => {
      clearPollTimer();
      cancelPendingRef.current = false;
      commitState((current) => ({
        ...current,
        phase: "trackingLost",
        operationPhase: "idle",
        error: code,
      }));
    },
    [clearPollTimer, commitState],
  );

  const failJobOperation = useCallback(
    (code: RestoreErrorCode, operationKind: "poll" | "cancel") => {
      const current = stateRef.current;
      if (current.job !== null && !isTerminal(current.job)) {
        if (code !== "RESTORE_INTERNAL") {
          loseJobTracking(code);
          return;
        }
        if (operationKind === "poll") {
          consecutiveInternalPollFailuresRef.current += 1;
          if (
            consecutiveInternalPollFailuresRef.current >=
            MAX_CONSECUTIVE_INTERNAL_POLL_FAILURES
          ) {
            loseJobTracking(code);
            return;
          }
        }
        commitState({
          ...current,
          phase: "active",
          operationPhase: "idle",
          error: code,
        });
        schedulePoll();
        return;
      }
      failWorkflow(code);
    },
    [commitState, failWorkflow, loseJobTracking, schedulePoll],
  );

  const applyNativeJob = useCallback(
    (
      snapshot: RestoreJobSnapshot,
      expectedJobId: string | null,
      expectedPlanId: string,
      cancelConfirmed: boolean,
    ) => {
      if (
        (expectedJobId !== null && snapshot.jobId !== expectedJobId) ||
        snapshot.planId !== expectedPlanId
      ) {
        throw new RestoreCommandError("REPORT_INCOMPATIBLE");
      }
      consecutiveInternalPollFailuresRef.current = 0;
      const terminal = isTerminal(snapshot);
      commitState((current) => ({
        ...current,
        phase: terminal ? "finished" : "active",
        job: snapshot,
        operationPhase: "idle",
        progressBasisPoints: restoreProgressBasisPoints(snapshot),
        cancelRequested: current.cancelRequested || cancelConfirmed,
        error: null,
      }));
      if (!terminal) {
        schedulePoll();
      } else {
        clearPollTimer();
      }
    },
    [clearPollTimer, commitState, schedulePoll],
  );

  const runPoll = useCallback(
    async (operation: PollOperation) => {
      if (!operationIsCurrent(operation)) {
        return;
      }
      commitState((current) => ({
        ...current,
        operationPhase: "polling",
      }));
      try {
        const snapshot = await getRestoreJob(
          nextRequestId(),
          operation.jobId,
        );
        if (!operationIsCurrent(operation)) {
          return;
        }
        applyNativeJob(
          snapshot,
          operation.jobId,
          operation.planId,
          false,
        );
      } catch (error) {
        if (operationIsCurrent(operation)) {
          failJobOperation(normalizeRestoreError(error).code, "poll");
        }
      }
    },
    [applyNativeJob, commitState, failJobOperation, operationIsCurrent],
  );

  const runCancel = useCallback(
    async (operation: CancelOperation) => {
      try {
        const snapshot = await cancelRestore(
          nextRequestId(),
          operation.jobId,
        );
        if (!operationIsCurrent(operation)) {
          return;
        }
        cancelPendingRef.current = false;
        applyNativeJob(
          snapshot,
          operation.jobId,
          operation.planId,
          true,
        );
      } catch (error) {
        if (operationIsCurrent(operation)) {
          cancelPendingRef.current = false;
          failJobOperation(normalizeRestoreError(error).code, "cancel");
        }
      } finally {
        cancelPendingRef.current = false;
        const current = stateRef.current;
        if (
          operationIsCurrent(operation) &&
          current.phase === "active" &&
          current.job !== null &&
          !isTerminal(current.job)
        ) {
          schedulePoll();
        }
      }
    },
    [
      applyNativeJob,
      failJobOperation,
      operationIsCurrent,
      schedulePoll,
    ],
  );

  const runOpen = useCallback(
    async (operation: OpenOperation) => {
      try {
        await openRestoreDestination(nextRequestId(), operation.jobId);
        if (operationIsCurrent(operation)) {
          commitState((current) => ({
            ...current,
            operationPhase: "idle",
            error: null,
          }));
        }
      } catch (error) {
        if (operationIsCurrent(operation)) {
          commitState((current) => ({
            ...current,
            phase:
              current.job?.status === "completed"
                ? "finished"
                : current.phase,
            operationPhase: "idle",
            error: normalizeRestoreError(error).code,
          }));
        }
      } finally {
        openPendingRef.current = false;
      }
    },
    [commitState, operationIsCurrent],
  );

  useEffect(() => {
    selectionRef.current = selection;
  }, [selection]);

  useEffect(() => {
    pumpOperationsRef.current = pumpOperations;
  }, [pumpOperations]);

  useEffect(() => {
    executeOperationRef.current = async (operation) => {
      if (operation.kind === "poll") {
        await runPoll(operation);
      } else if (operation.kind === "cancel") {
        await runCancel(operation);
      } else {
        await runOpen(operation);
      }
    };
  }, [runCancel, runOpen, runPoll]);

  const clearOperations = useCallback(() => {
    clearPollTimer();
    operationsRef.current = [];
    pollQueuedRef.current = false;
    cancelPendingRef.current = false;
    openPendingRef.current = false;
    consecutiveInternalPollFailuresRef.current = 0;
  }, [clearPollTimer]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      generationRef.current += 1;
      operationIntentRef.current += 1;
      clearOperations();
    };
  }, [clearOperations]);

  useEffect(() => {
    generationRef.current += 1;
    operationIntentRef.current += 1;
    destinationInFlightRef.current = false;
    planInFlightRef.current = false;
    startInFlightRef.current = false;
    clearOperations();
    commitState(
      initialState(
        scanId,
        selectionRef.current?.selectionRevision ?? null,
      ),
    );
  }, [
    clearOperations,
    commitState,
    runtimeAvailable,
    scanId,
  ]);

  useEffect(() => {
    const revision = selection?.selectionRevision ?? null;
    const current = stateRef.current;
    if (current.selectionRevision === revision) {
      return;
    }
    if (
      current.phase === "setup" ||
      current.phase === "planning" ||
      current.phase === "review"
    ) {
      commitState({
        ...current,
        phase: current.destination === null ? "idle" : "setup",
        selectionRevision: revision,
        partialFilePolicy: "completeOnly",
        bestEffortConsent: false,
        plan: null,
        job: null,
        progressBasisPoints: 0,
        error: null,
      });
      return;
    }
    commitState({
      ...current,
      selectionRevision: revision,
    });
  }, [commitState, selection?.selectionRevision]);

  const chooseDestination = useCallback(() => {
    const current = stateRef.current;
    const authoritativeSelection = selectionRef.current;
    if (
      !runtimeAvailable ||
      current.scanId === null ||
      !hasSelectedCandidates(authoritativeSelection) ||
      destinationInFlightRef.current ||
      !(
        current.phase === "idle" ||
        current.phase === "setup" ||
        (current.phase === "error" &&
          current.error !== null &&
          DESTINATION_ERROR_CODES.has(current.error))
      )
    ) {
      return;
    }
    destinationInFlightRef.current = true;
    const generation = generationRef.current;
    const selectedScanId = current.scanId;
    commitState({
      ...current,
      phase: "selectingDestination",
      destination: null,
      partialFilePolicy: "completeOnly",
      bestEffortConsent: false,
      plan: null,
      job: null,
      progressBasisPoints: 0,
      cancelRequested: false,
      error: null,
    });
    void selectRestoreDestination(nextRequestId(), selectedScanId)
      .then((destination) => {
        if (
          !mountedRef.current ||
          generation !== generationRef.current ||
          stateRef.current.scanId !== selectedScanId
        ) {
          return;
        }
        if (destination === null) {
          commitState(
            initialState(
              selectedScanId,
              selectionRef.current?.selectionRevision ?? null,
            ),
          );
          return;
        }
        commitState((latest) => ({
          ...latest,
          phase: "setup",
          selectionRevision:
            selectionRef.current?.selectionRevision ?? null,
          destination,
          error: null,
        }));
      })
      .catch((error: unknown) => {
        if (
          mountedRef.current &&
          generation === generationRef.current &&
          stateRef.current.scanId === selectedScanId
        ) {
          failWorkflow(normalizeRestoreError(error).code);
        }
      })
      .finally(() => {
        destinationInFlightRef.current = false;
      });
  }, [commitState, failWorkflow, runtimeAvailable]);

  const setPartialFilePolicy = useCallback(
    (policy: PartialFilePolicy) => {
      const current = stateRef.current;
      if (
        current.destination === null ||
        (current.phase !== "setup" && current.phase !== "error")
      ) {
        return;
      }
      commitState({
        ...current,
        phase: "setup",
        partialFilePolicy: policy,
        bestEffortConsent:
          policy === "zeroFillAndMap"
            ? current.bestEffortConsent
            : false,
        plan: null,
        error: null,
      });
    },
    [commitState],
  );

  const setBestEffortConsent = useCallback(
    (consent: boolean) => {
      const current = stateRef.current;
      if (
        current.destination === null ||
        current.partialFilePolicy !== "zeroFillAndMap" ||
        (current.phase !== "setup" && current.phase !== "error")
      ) {
        return;
      }
      commitState({
        ...current,
        phase: "setup",
        bestEffortConsent: consent,
        plan: null,
        error: null,
      });
    },
    [commitState],
  );

  const createPlanCommand = useCallback(() => {
    const current = stateRef.current;
    const authoritativeSelection = selectionRef.current;
    if (
      current.phase !== "setup" ||
      current.scanId === null ||
      current.destination === null ||
      !hasSelectedCandidates(authoritativeSelection) ||
      planInFlightRef.current
    ) {
      return;
    }
    if (BigInt(authoritativeSelection.ineligibleCandidates) > 0n) {
      commitState({
        ...current,
        phase: "error",
        error: "RESTORE_ITEM_INELIGIBLE",
      });
      return;
    }
    if (
      BigInt(authoritativeSelection.bestEffortCandidates) > 0n &&
      (current.partialFilePolicy !== "zeroFillAndMap" ||
        !current.bestEffortConsent)
    ) {
      commitState({
        ...current,
        phase: "error",
        error: "RESTORE_PARTIAL_POLICY_REQUIRED",
      });
      return;
    }
    if (
      current.selectionRevision !==
      authoritativeSelection.selectionRevision
    ) {
      commitState({
        ...current,
        phase: "error",
        error: "RESTORE_SELECTION_STALE",
      });
      return;
    }

    planInFlightRef.current = true;
    const generation = generationRef.current;
    const selectedScanId = current.scanId;
    const selectionRevision = authoritativeSelection.selectionRevision;
    const destinationId = current.destination.destinationId;
    const collisionPolicy = current.collisionPolicy;
    const partialFilePolicy = current.partialFilePolicy;
    commitState({
      ...current,
      phase: "planning",
      plan: null,
      error: null,
    });
    void createRestorePlan(
      nextRequestId(),
      selectedScanId,
      selectionRevision,
      destinationId,
      collisionPolicy,
      partialFilePolicy,
    )
      .then((plan) => {
        if (
          !mountedRef.current ||
          generation !== generationRef.current ||
          stateRef.current.scanId !== selectedScanId ||
          selectionRef.current?.selectionRevision !== selectionRevision
        ) {
          return;
        }
        if (
          plan.scanId !== selectedScanId ||
          plan.selectionRevision !== selectionRevision ||
          plan.destinationId !== destinationId ||
          plan.collisionPolicy !== collisionPolicy ||
          plan.partialFilePolicy !== partialFilePolicy
        ) {
          throw new RestoreCommandError("REPORT_INCOMPATIBLE");
        }
        commitState((latest) => ({
          ...latest,
          phase: "review",
          plan,
          error: null,
        }));
      })
      .catch((error: unknown) => {
        if (
          mountedRef.current &&
          generation === generationRef.current &&
          stateRef.current.scanId === selectedScanId &&
          selectionRef.current?.selectionRevision === selectionRevision
        ) {
          failWorkflow(normalizeRestoreError(error).code);
        }
      })
      .finally(() => {
        planInFlightRef.current = false;
      });
  }, [commitState, failWorkflow]);

  const start = useCallback(() => {
    const current = stateRef.current;
    const authoritativeSelection = selectionRef.current;
    const plan = current.plan;
    if (current.phase !== "review" || startInFlightRef.current) {
      return;
    }
    if (plan === null) {
      failWorkflow("REPORT_INCOMPATIBLE");
      return;
    }
    if (
      authoritativeSelection === null ||
      plan.selectionRevision !== authoritativeSelection.selectionRevision
    ) {
      commitState({
        ...current,
        phase: "error",
        plan: null,
        error: "RESTORE_SELECTION_STALE",
      });
      return;
    }
    startInFlightRef.current = true;
    const generation = generationRef.current;
    commitState({
      ...current,
      phase: "starting",
      error: null,
    });
    void startRestore(nextRequestId(), plan.planId)
      .then((snapshot) => {
        if (
          !mountedRef.current ||
          generation !== generationRef.current
        ) {
          return;
        }
        applyNativeJob(snapshot, null, plan.planId, false);
      })
      .catch((error: unknown) => {
        if (
          mountedRef.current &&
          generation === generationRef.current
        ) {
          failWorkflow(normalizeRestoreError(error).code);
        }
      })
      .finally(() => {
        startInFlightRef.current = false;
      });
  }, [applyNativeJob, commitState, failWorkflow]);

  const cancel = useCallback(() => {
    const current = stateRef.current;
    const job = current.job;
    if (
      current.phase !== "active" ||
      job === null ||
      isTerminal(job) ||
      cancelPendingRef.current ||
      current.cancelRequested
    ) {
      return;
    }
    cancelPendingRef.current = true;
    clearPollTimer();
    operationIntentRef.current += 1;
    const intent = operationIntentRef.current;
    commitState({
      ...current,
      operationPhase: "cancelRequest",
      error: null,
    });
    enqueueOperation({
      kind: "cancel",
      generation: generationRef.current,
      intent,
      jobId: job.jobId,
      planId: job.planId,
    });
  }, [clearPollTimer, commitState, enqueueOperation]);

  const openDestination = useCallback(() => {
    const current = stateRef.current;
    const job = current.job;
    if (
      current.phase !== "finished" ||
      job?.status !== "completed" ||
      openPendingRef.current ||
      current.operationPhase !== "idle"
    ) {
      return;
    }
    openPendingRef.current = true;
    operationIntentRef.current += 1;
    const intent = operationIntentRef.current;
    commitState({
      ...current,
      operationPhase: "opening",
      error: null,
    });
    enqueueOperation({
      kind: "open",
      generation: generationRef.current,
      intent,
      jobId: job.jobId,
    });
  }, [commitState, enqueueOperation]);

  const close = useCallback(() => {
    const current = stateRef.current;
    if (
      current.phase === "selectingDestination" ||
      current.phase === "planning" ||
      current.phase === "starting" ||
      current.phase === "active"
    ) {
      return;
    }
    generationRef.current += 1;
    operationIntentRef.current += 1;
    clearOperations();
    commitState(
      initialState(
        current.scanId,
        selectionRef.current?.selectionRevision ?? null,
      ),
    );
  }, [clearOperations, commitState]);

  return {
    state,
    chooseDestination,
    setPartialFilePolicy,
    setBestEffortConsent,
    createPlan: createPlanCommand,
    start,
    cancel,
    openDestination,
    close,
  };
}
