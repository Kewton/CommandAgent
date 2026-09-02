"use client";

import { useEffect, useMemo, useRef, useState } from "react";

import type { PolledSession } from "../lib/types";
import { trialRoutePath, withBasePath } from "../lib/base-path";
import { useTrialCompose, type ScreenStage } from "./use-trial-compose";
import { useTrialMonitor } from "./use-trial-monitor";
import { useTrialTerminal } from "./use-trial-terminal";

export type { ScreenStage } from "./use-trial-compose";

export function useTrialRun(
  terminalHeading: (session: PolledSession) => string,
  { loadComposeOptions = true }: { loadComposeOptions?: boolean } = {},
) {
  const [stage, setStage] = useState<ScreenStage>("compose");
  const priorStageRef = useRef<ScreenStage>(stage);
  const compose = useTrialCompose({ loadOptions: loadComposeOptions, stage, setStage });
  const monitor = useTrialMonitor({
    reconnectSessionId: compose.reconnectSessionId,
    rejectTrialToken: compose.rejectTrialToken,
    setBusy: compose.setBusy,
    setError: compose.setError,
    setErrorReconnectSessionId: compose.setErrorReconnectSessionId,
    setReconnectSessionId: compose.setReconnectSessionId,
    setStage,
    stage,
    trialAccessReady: compose.trialAccessReady,
    trialToken: compose.trialToken,
  });
  const terminal = useTrialTerminal({
    created: monitor.created,
    recordError: compose.recordError,
    rejectTrialToken: compose.rejectTrialToken,
    resumeForDirective: monitor.resumeForDirective,
    session: monitor.session,
    setBusy: compose.setBusy,
    setError: compose.setError,
    setErrorReconnectSessionId: compose.setErrorReconnectSessionId,
    setStage,
    setWorkspaceLease: compose.setWorkspaceLease,
    stage,
    terminalHeading,
    trialToken: compose.trialToken,
  });

  useEffect(() => {
    if (priorStageRef.current === stage) return;
    priorStageRef.current = stage;
    const target =
      stage === "compose"
        ? compose.composeRef.current
        : stage === "gate_1"
        ? compose.gateOneRef.current
        : stage === "gate_2"
          ? monitor.executionRef.current
          : stage === "terminal"
            ? terminal.terminalRef.current
            : document.querySelector<HTMLElement>("[data-testid='closed-session']");
    if (target === null) return;
    const frame = window.requestAnimationFrame(() => {
      target.focus({ preventScroll: true });
      if (window.matchMedia("(max-width: 720px)").matches) {
        target.scrollIntoView({ behavior: "smooth", block: "start" });
      }
    });
    return () => window.cancelAnimationFrame(frame);
  }, [
    compose.composeRef,
    compose.gateOneRef,
    monitor.executionRef,
    stage,
    terminal.terminalRef,
  ]);

  const priceDuration = useMemo(() => {
    const seconds =
      monitor.session?.average_duration_seconds ?? compose.proposal?.price.average_duration_seconds;
    const unavailable = compose.proposal?.price.source === "未計測" ? "未計測" : "未記録";
    return seconds === null || seconds === undefined
      ? unavailable
      : `平均 ${(seconds / 60).toFixed(1)} 分`;
  }, [compose.proposal, monitor.session]);
  const priceCost = useMemo(() => {
    const cost = compose.proposal?.price.average_cost_usd;
    const unavailable = compose.proposal?.price.source === "未計測" ? "未計測" : "未記録";
    return cost === null || cost === undefined ? unavailable : `平均 $${cost.toFixed(4)}`;
  }, [compose.proposal]);

  function launchConfirmed() {
    return compose.launchConfirmed((value) => {
      terminal.resetForLaunch();
      monitor.acceptLaunch(value);
    });
  }

  function startNewRun() {
    compose.resetForNewRun();
    monitor.resetForNewRun();
    terminal.resetForNewRun();
    setStage("compose");
    window.location.assign(withBasePath(trialRoutePath("compose")));
  }

  return {
    artifacts: terminal.artifacts,
    busy: compose.busy,
    checkContract: compose.checkContract,
    cancelStop: monitor.cancelStop,
    composeRef: compose.composeRef,
    confirmDirective: terminal.confirmDirective,
    confirmStop: monitor.confirmStop,
    confirmed: compose.confirmed,
    created: monitor.created,
    currentPhase: monitor.currentPhase,
    directive: terminal.directive,
    directiveText: terminal.directiveText,
    elapsedSeconds: monitor.elapsedSeconds,
    editProposal: compose.editProposal,
    error: compose.error,
    errorReconnectSessionId: compose.errorReconnectSessionId,
    evidenceAnnouncement: terminal.evidenceAnnouncement,
    evidenceDocument: terminal.evidenceDocument,
    evidenceError: terminal.evidenceError,
    evidenceLoading: terminal.evidenceLoading,
    evidenceOpen: terminal.evidenceOpen,
    executionRef: monitor.executionRef,
    gateOneRef: compose.gateOneRef,
    inspectWorkspaceLease: compose.inspectWorkspaceLease,
    launchBlockReason: compose.launchBlockReason,
    launchConfirmed,
    launchIdentityLocked: compose.launchIdentityLocked,
    loadArtifacts: terminal.loadArtifacts,
    monitor: monitor.monitor,
    observedSession: monitor.observedSession,
    optionsError: compose.optionsError,
    compatiblePacks: compose.compatiblePacks,
    persistDirective: terminal.persistDirective,
    priceCost,
    priceDuration,
    proposal: compose.proposal,
    providerChanged: compose.providerChanged,
    readArtifact: terminal.readArtifact,
    readEvents: terminal.readEvents,
    readRecoveryDocument: terminal.readRecoveryDocument,
    reconnectExisting: monitor.reconnectExisting,
    reconnectSessionId: compose.reconnectSessionId,
    rejectTrialToken: compose.rejectTrialToken,
    selectedProfile: compose.selectedProfile,
    selectedProvider: compose.selectedProvider,
    selectedPack: compose.selectedPack,
    session: monitor.session,
    sessionIndexRevision: monitor.sessionIndexRevision,
    setConfirmed: compose.setConfirmed,
    setDirective: terminal.setDirective,
    setDirectiveText: terminal.setDirectiveText,
    setProposal: compose.setProposal,
    setProviderChanged: compose.setProviderChanged,
    setReconnectSessionId: compose.setReconnectSessionId,
    setStage,
    setWorkspaceLease: compose.setWorkspaceLease,
    spec: compose.spec,
    stage,
    startNewRun,
    stopActiveSession: monitor.stopActiveSession,
    stopError: monitor.stopError,
    stopState: monitor.stopState,
    terminalRef: terminal.terminalRef,
    trialAccessReady: compose.trialAccessReady,
    trialOptions: compose.trialOptions,
    trialToken: compose.trialToken,
    trialTokenAuthEnabled: compose.trialTokenAuthEnabled,
    update: compose.update,
    updateTrialToken: compose.updateTrialToken,
    workspaceLease: compose.workspaceLease,
  };
}

export type TrialRunState = ReturnType<typeof useTrialRun>;
