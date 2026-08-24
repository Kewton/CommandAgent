"use client";

import { useCallback, useEffect, useRef, useState, type Dispatch, type SetStateAction } from "react";

import { describeError } from "../lib/errors";
import { elapsedLabel } from "../lib/format";
import {
  confirmDirective as confirmSessionDirective,
  createDirective as requestDirective,
  fetchSessionArtifact,
  fetchSessionArtifacts,
  fetchSessionEvents,
} from "../lib/trial-api";
import type {
  CreatedSession,
  DirectiveProposal,
  DocumentRecord,
  DocumentSummary,
  PolledSession,
  TrialWorkspaceLease,
} from "../lib/types";
import type { ScreenStage } from "./use-trial-compose";

type UseTrialTerminalProps = {
  created: CreatedSession | null;
  recordError: (reason: unknown) => void;
  rejectTrialToken: (reason: unknown, rejectedValue: string) => boolean;
  resumeForDirective: () => void;
  session: PolledSession | null;
  setBusy: Dispatch<SetStateAction<boolean>>;
  setError: Dispatch<SetStateAction<string | null>>;
  setErrorReconnectSessionId: Dispatch<SetStateAction<string | null>>;
  setStage: Dispatch<SetStateAction<ScreenStage>>;
  setWorkspaceLease: Dispatch<SetStateAction<TrialWorkspaceLease | null>>;
  stage: ScreenStage;
  terminalHeading: (session: PolledSession) => string;
  trialToken: string;
};

export function useTrialTerminal(props: UseTrialTerminalProps) {
  const {
    created, recordError, rejectTrialToken, resumeForDirective, session, setBusy,
    setError, setErrorReconnectSessionId, setStage, setWorkspaceLease, stage,
    terminalHeading, trialToken,
  } = props;
  const terminalRef = useRef<HTMLElement>(null);
  const priorStageRef = useRef<ScreenStage>(stage);
  const preTerminalTitleRef = useRef<string | null>(null);
  const notifiedTerminalRef = useRef<string | null>(null);
  const [directiveText, setDirectiveText] = useState("");
  const [directive, setDirective] = useState<DirectiveProposal | null>(null);
  const [artifacts, setArtifacts] = useState<DocumentSummary[]>([]);
  const [evidenceDocument, setEvidenceDocument] = useState<DocumentRecord | null>(null);
  const [evidenceOpen, setEvidenceOpen] = useState(false);
  const [evidenceLoading, setEvidenceLoading] = useState(false);
  const [evidenceError, setEvidenceError] = useState<string | null>(null);

  const loadArtifacts = useCallback(async () => {
    if (created === null) return;
    setEvidenceOpen(true);
    setEvidenceLoading(true);
    setEvidenceError(null);
    try {
      setArtifacts(await fetchSessionArtifacts(trialToken, created.id));
    } catch (reason) {
      rejectTrialToken(reason, trialToken);
      setEvidenceError(describeError(reason));
    } finally {
      setEvidenceLoading(false);
    }
  }, [created, rejectTrialToken, trialToken]);

  useEffect(() => {
    const priorStage = priorStageRef.current;
    priorStageRef.current = stage;
    if (stage !== "terminal" || session === null) {
      preTerminalTitleRef.current = document.title;
      return;
    }
    const heading = terminalHeading(session);
    const marker = session.gate === "gate_3" ? "✔" : "✗";
    const previousTitle = preTerminalTitleRef.current ?? document.title;
    document.title = `${marker} ${heading} | CommandAgent`;

    const notificationKey = `${session.id}:${session.event_count}`;
    if (priorStage === "gate_2" && notifiedTerminalRef.current !== notificationKey) {
      notifiedTerminalRef.current = notificationKey;
      notifyCompletion(session, heading);
    }
    return () => { document.title = previousTitle; };
  }, [session, stage, terminalHeading]);

  useEffect(() => {
    if (stage === "terminal") void loadArtifacts();
  }, [loadArtifacts, stage]);

  async function persistDirective() {
    if (created === null || directiveText.trim() === "") return;
    setBusy(true);
    setError(null);
    setErrorReconnectSessionId(null);
    try {
      setDirective(await requestDirective(trialToken, created.id, directiveText));
    } catch (reason) {
      recordError(reason);
    } finally {
      setBusy(false);
    }
  }

  async function confirmDirective() {
    if (created === null || directive === null) return;
    setBusy(true);
    setError(null);
    setErrorReconnectSessionId(null);
    try {
      await confirmSessionDirective(trialToken, created.id, directive.directive_hash);
      setDirective(null);
      setDirectiveText("");
      setWorkspaceLease(null);
      resumeForDirective();
    } catch (reason) {
      recordError(reason);
    } finally {
      setBusy(false);
    }
  }

  async function readEvents() {
    if (created === null) return;
    await readEvidence(() => fetchSessionEvents(trialToken, created.id));
  }

  async function readArtifact(path: string) {
    if (created === null) return;
    await readEvidence(() => fetchSessionArtifact(trialToken, created.id, path));
  }

  async function readEvidence(load: () => Promise<DocumentRecord>) {
    setEvidenceOpen(true);
    setEvidenceLoading(true);
    setEvidenceError(null);
    try {
      setEvidenceDocument(await load());
    } catch (reason) {
      rejectTrialToken(reason, trialToken);
      setEvidenceError(describeError(reason));
    } finally {
      setEvidenceLoading(false);
    }
  }

  function resetForLaunch() {
    setArtifacts([]);
    setEvidenceDocument(null);
    setEvidenceOpen(false);
    setEvidenceError(null);
  }

  function resetForNewRun() {
    resetForLaunch();
    setDirectiveText("");
    setDirective(null);
  }

  return {
    artifacts, confirmDirective, directive, directiveText, evidenceDocument,
    evidenceError, evidenceLoading, evidenceOpen, loadArtifacts, persistDirective,
    readArtifact, readEvents, resetForLaunch, resetForNewRun, setDirective,
    setDirectiveText, setStage, terminalRef,
  };
}

function notifyCompletion(session: PolledSession, heading: string) {
  if (
    !document.hidden || typeof window.Notification === "undefined" ||
    window.Notification.permission !== "granted"
  ) return;
  const elapsedSeconds = Math.max(
    0,
    Math.floor(Date.now() / 1_000 - session.started_epoch_seconds),
  );
  try {
    new window.Notification(`CommandAgent: ${session.gate === "gate_3" ? "Gate 3" : "Gate 4"}`, {
      body: `${heading} 所要時間 ${elapsedLabel(elapsedSeconds)}`,
    });
  } catch {
    // Notification support can disappear with browser or OS policy changes.
  }
}
