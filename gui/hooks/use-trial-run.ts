"use client";

import { useEffect, useMemo, useState } from "react";

import type { PolledSession } from "../lib/types";
import { useTrialCompose, type ScreenStage } from "./use-trial-compose";
import { useTrialMonitor } from "./use-trial-monitor";
import { useTrialTerminal } from "./use-trial-terminal";

export type { ScreenStage } from "./use-trial-compose";

export function useTrialRun(terminalHeading: (session: PolledSession) => string) {
  const [stage, setStage] = useState<ScreenStage>("compose");
  const compose = useTrialCompose({ stage, setStage });
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
    if (!window.matchMedia("(max-width: 720px)").matches) return;
    const target =
      stage === "gate_1"
        ? compose.gateOneRef.current
        : stage === "gate_2"
          ? monitor.executionRef.current
          : stage === "terminal"
            ? terminal.terminalRef.current
            : null;
    if (target === null) return;
    const frame = window.requestAnimationFrame(() => {
      target.scrollIntoView({ behavior: "smooth", block: "start" });
    });
    return () => window.cancelAnimationFrame(frame);
  }, [compose.gateOneRef, monitor.executionRef, stage, terminal.terminalRef]);

  const priceDuration = useMemo(() => {
    const seconds =
      monitor.session?.average_duration_seconds ?? compose.proposal?.price.average_duration_seconds;
    return seconds === null || seconds === undefined
      ? "未記録"
      : `平均 ${(seconds / 60).toFixed(1)} 分`;
  }, [compose.proposal, monitor.session]);
  const priceCost = useMemo(() => {
    const cost = compose.proposal?.price.average_cost_usd;
    return cost === null || cost === undefined ? "未記録" : `平均 $${cost.toFixed(4)}`;
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
  }

  return {
    artifacts: terminal.artifacts,
    busy: compose.busy,
    checkContract: compose.checkContract,
    confirmDirective: terminal.confirmDirective,
    confirmed: compose.confirmed,
    created: monitor.created,
    currentPhase: monitor.currentPhase,
    directive: terminal.directive,
    directiveText: terminal.directiveText,
    elapsedSeconds: monitor.elapsedSeconds,
    error: compose.error,
    errorReconnectSessionId: compose.errorReconnectSessionId,
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
