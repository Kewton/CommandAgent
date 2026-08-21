"use client";

import { useCallback, useEffect, useRef, useState, type Dispatch, type SetStateAction } from "react";

import { describeError } from "../lib/errors";
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
    if (stage !== "terminal" || session === null) return;
    const previousTitle = document.title;
    document.title = `✔ ${terminalHeading(session)} — CommandAgent`;
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
