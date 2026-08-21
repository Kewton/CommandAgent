"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useShellRuntimeStatus } from "../components/shell";
import {
  describeError,
  GuiRequestError,
  isTrialTokenRejected,
  reconnectSessionId as reconnectIdFromError,
} from "../lib/errors";
import {
  persistTrialToken,
  removeRejectedTrialToken,
  restoreTrialToken,
} from "../lib/trial-token-storage";
import {
  CHANGED_POLL_INTERVAL_MS,
  TERMINAL_FAILURE_LIMIT,
  type MonitorStatus,
  monitorFailure,
  retryDelay,
  unchangedPollDelay,
} from "../lib/trial-monitor";
import {
  confirmDirective as confirmSessionDirective,
  createDirective as requestDirective,
  createSession as launchSession,
  fetchSession,
  fetchSessionArtifact,
  fetchSessionArtifacts,
  fetchSessionEvents,
  fetchSessionPoll,
  fetchPackOptions,
  fetchTrialOptions,
  fetchWorkspaceLease,
  proposeSession,
} from "../lib/trial-api";
import type {
  CreatedSession,
  DirectiveProposal,
  DocumentRecord,
  DocumentSummary,
  PolledSession,
  PackOptions,
  SessionProposal,
  SessionSpec,
  TrialOptions,
  TrialWorkspaceLease,
} from "../lib/types";

const initialSpec: SessionSpec = {
  goal: "",
  profile: "python-cli",
  provider: "ollama",
  model: "",
  planner_provider: "ollama",
  planner_model: "",
  pack: null,
};

const pythonCliSample: Pick<SessionSpec, "goal" | "profile" | "pack"> = {
  goal: "Create a CLI --pattern filter command",
  profile: "python-cli",
  pack: "cli-assist@1.0.0",
};

export type ScreenStage = "compose" | "gate_1" | "gate_2" | "terminal" | "closed";

type MonitorState = {
  attempt: number;
  guidance: string | null;
  lastSuccessAt: string | null;
  retryInMs: number | null;
  status: MonitorStatus;
  summary: string | null;
};

const initialMonitor: MonitorState = {
  attempt: 0,
  guidance: null,
  lastSuccessAt: null,
  retryInMs: null,
  status: "degraded",
  summary: null,
};

export function useTrialRun(
  terminalHeading: (session: PolledSession) => string,
) {
  const runtime = useShellRuntimeStatus();
  const gateOneRef = useRef<HTMLElement>(null);
  const executionRef = useRef<HTMLElement>(null);
  const terminalRef = useRef<HTMLElement>(null);
  const packPreselectionApplied = useRef(false);
  const [trialToken, setTrialToken] = useState("");
  const [reconnectSessionId, setReconnectSessionId] = useState("");
  const [spec, setSpec] = useState<SessionSpec>(initialSpec);
  const [proposal, setProposal] = useState<SessionProposal | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const [created, setCreated] = useState<CreatedSession | null>(null);
  const [session, setSession] = useState<PolledSession | null>(null);
  const [gateTwoStartedAt, setGateTwoStartedAt] = useState<number | null>(null);
  const [elapsedSeconds, setElapsedSeconds] = useState(0);
  const [stage, setStage] = useState<ScreenStage>("compose");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [errorReconnectSessionId, setErrorReconnectSessionId] = useState<string | null>(null);
  const [trialOptions, setTrialOptions] = useState<TrialOptions | null>(null);
  const [packOptions, setPackOptions] = useState<PackOptions | null>(null);
  const [optionsError, setOptionsError] = useState<string | null>(null);
  const [providerChanged, setProviderChanged] = useState(false);
  const [directiveText, setDirectiveText] = useState("");
  const [directive, setDirective] = useState<DirectiveProposal | null>(null);
  const [workspaceLease, setWorkspaceLease] = useState<TrialWorkspaceLease | null>(null);
  const [sessionIndexRevision, setSessionIndexRevision] = useState(0);
  const launchIdentityLocked =
    stage === "gate_2" || stage === "terminal" || stage === "closed";
  const [monitor, setMonitor] = useState<MonitorState>(initialMonitor);
  const [artifacts, setArtifacts] = useState<DocumentSummary[]>([]);

  useEffect(() => {
    if (new URLSearchParams(window.location.search).get("sample") !== "python-cli") return;
    setSpec((current) => ({
      ...current,
      ...pythonCliSample,
    }));
  }, []);
  const [evidenceDocument, setEvidenceDocument] = useState<DocumentRecord | null>(null);
  const [evidenceOpen, setEvidenceOpen] = useState(false);
  const [evidenceLoading, setEvidenceLoading] = useState(false);
  const [evidenceError, setEvidenceError] = useState<string | null>(null);
  const trialTokenAuthEnabled = runtime?.data?.trial_token_auth_enabled !== false;
  const trialAccessReady = !trialTokenAuthEnabled || trialToken.trim() !== "";

  useEffect(() => {
    setTrialToken(restoreTrialToken());
  }, []);

  useEffect(() => {
    if (runtime?.data?.trial_token_auth_enabled === false) {
      setTrialToken("");
      persistTrialToken("");
    }
  }, [runtime?.data?.trial_token_auth_enabled]);

  const updateTrialToken = useCallback((value: string) => {
    setTrialToken(value);
    persistTrialToken(value);
  }, []);

  const rejectTrialToken = useCallback((reason: unknown, rejectedValue: string) => {
    if (!isTrialTokenRejected(reason)) return false;
    removeRejectedTrialToken(rejectedValue);
    setTrialToken((current) =>
      current.trim() === rejectedValue.trim() ? "" : current,
    );
    return true;
  }, []);

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
    let cancelled = false;
    const loadOptions = async () => {
      try {
        const [value, packs] = await Promise.all([
          fetchTrialOptions(),
          fetchPackOptions(),
        ]);
        if (!cancelled) {
          setTrialOptions(value);
          setPackOptions(packs);
        }
      } catch (reason) {
        if (!cancelled) setOptionsError(describeError(reason));
      }
    };
    void loadOptions();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const id = new URLSearchParams(window.location.search).get("session");
    if (id !== null) setReconnectSessionId(id);
  }, []);

  useEffect(() => {
    if (packOptions === null || packPreselectionApplied.current) return;
    packPreselectionApplied.current = true;
    const selector = new URLSearchParams(window.location.search).get("pack");
    const option = packOptions.packs.find(
      (candidate) => `${candidate.id}@${candidate.version}` === selector,
    );
    if (selector !== null && option !== undefined) {
      setSpec((current) => ({ ...current, profile: option.profile, pack: selector }));
    }
  }, [packOptions]);

  useEffect(() => {
    if (
      created === null ||
      !trialAccessReady ||
      stage === "closed" ||
      stage === "terminal"
    ) {
      return;
    }
    let cancelled = false;
    let attempt = 0;
    let etag: string | null = null;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let unchangedResponses = 0;
    const poll = async () => {
      try {
        const result = await fetchSessionPoll(created.id, trialToken, etag);
        if (cancelled) return;
        attempt = 0;
        etag = result.etag;
        setMonitor({
          attempt: 0,
          guidance: null,
          lastSuccessAt: new Date().toISOString(),
          retryInMs: null,
          status: "connected",
          summary: null,
        });
        if (result.value === null) {
          unchangedResponses += 1;
          timer = setTimeout(() => void poll(), unchangedPollDelay(unchangedResponses));
          return;
        }
        unchangedResponses = 0;
        const value = result.value;
        setSession(value);
        const startedAt = epochMilliseconds(value.started_epoch_seconds);
        if (startedAt !== null) setGateTwoStartedAt(startedAt);
        if (value.gate === "gate_3" || value.gate === "gate_4") {
          setSessionIndexRevision((current) => current + 1);
          setStage("terminal");
          return;
        }
        setStage("gate_2");
        timer = setTimeout(() => void poll(), CHANGED_POLL_INTERVAL_MS);
      } catch (reason) {
        if (cancelled) return;
        attempt += 1;
        unchangedResponses = 0;
        const failure = monitorFailure(reason);
        const rejected = rejectTrialToken(reason, trialToken);
        const stop = rejected || (failure.terminal && attempt >= TERMINAL_FAILURE_LIMIT);
        const delay = retryDelay(attempt);
        setMonitor((current) => ({
          attempt,
          guidance: stop
            ? `${attempt} 回失敗したため監視を停止しました。${failure.guidance}`
            : failure.guidance,
          lastSuccessAt: current.lastSuccessAt,
          retryInMs: stop ? null : delay,
          status: stop || attempt >= TERMINAL_FAILURE_LIMIT ? "lost" : "degraded",
          summary: failure.summary,
        }));
        if (!stop) timer = setTimeout(() => void poll(), delay);
      }
    };
    void poll();
    return () => {
      cancelled = true;
      if (timer !== undefined) clearTimeout(timer);
    };
  }, [created, rejectTrialToken, stage, trialAccessReady, trialToken]);

  useEffect(() => {
    if (gateTwoStartedAt === null || stage !== "gate_2") return;
    const tick = () => {
      setElapsedSeconds(elapsedSince(gateTwoStartedAt));
    };
    tick();
    const timer = window.setInterval(tick, 1_000);
    return () => window.clearInterval(timer);
  }, [gateTwoStartedAt, stage]);

  useEffect(() => {
    if (stage !== "terminal" || session === null) return;
    const previousTitle = document.title;
    document.title = `✔ ${terminalHeading(session)} — CommandAgent`;
    return () => {
      document.title = previousTitle;
    };
  }, [session, stage]);

  useEffect(() => {
    if (stage === "terminal") void loadArtifacts();
  }, [loadArtifacts, stage]);

  useEffect(() => {
    if (!window.matchMedia("(max-width: 720px)").matches) return;
    const target =
      stage === "gate_1"
        ? gateOneRef.current
        : stage === "gate_2"
          ? executionRef.current
          : stage === "terminal"
            ? terminalRef.current
            : null;
    if (target === null) return;
    const frame = window.requestAnimationFrame(() => {
      target.scrollIntoView({ behavior: "smooth", block: "start" });
    });
    return () => window.cancelAnimationFrame(frame);
  }, [stage]);

  const priceDuration = useMemo(() => {
    const seconds = session?.average_duration_seconds ?? proposal?.price.average_duration_seconds;
    return seconds === null || seconds === undefined
      ? "未記録"
      : `平均 ${(seconds / 60).toFixed(1)} 分`;
  }, [proposal, session]);
  const priceCost = useMemo(() => {
    const cost = proposal?.price.average_cost_usd;
    return cost === null || cost === undefined ? "未記録" : `平均 $${cost.toFixed(4)}`;
  }, [proposal]);
  const currentPhase = useMemo(() => {
    const phases = session?.phases ?? [];
    return phases.find((phase) => phase.status === "running") ?? phases[phases.length - 1] ?? null;
  }, [session]);
  const selectedProfile = trialOptions?.profiles.find((option) => option.id === spec.profile);
  const selectedProvider = trialOptions?.providers.find((option) => option.id === spec.provider);
  const compatiblePacks = packOptions?.packs.filter(
    (option) => option.profile === spec.profile && option.intent === "create",
  ) ?? [];
  const selectedPack = compatiblePacks.find(
    (option) => `${option.id}@${option.version}` === spec.pack,
  );

  function update<K extends keyof SessionSpec>(field: K, value: SessionSpec[K]) {
    setSpec((current) => {
      if (field === "profile") return { ...current, profile: value as string, pack: null };
      if (field === "provider") {
        return { ...current, provider: value as string, planner_provider: value as string };
      }
      return { ...current, [field]: value };
    });
    setProposal(null);
    setConfirmed(false);
    setError(null);
    setErrorReconnectSessionId(null);
    setStage("compose");
  }

  function startNewRun() {
    setProposal(null);
    setConfirmed(false);
    setCreated(null);
    setSession(null);
    setGateTwoStartedAt(null);
    setElapsedSeconds(0);
    setArtifacts([]);
    setEvidenceDocument(null);
    setEvidenceOpen(false);
    setEvidenceError(null);
    setDirectiveText("");
    setDirective(null);
    setError(null);
    setErrorReconnectSessionId(null);
    setStage("compose");
  }

  async function checkContract() {
    if (spec.goal.trim() === "") {
      setError("契約を確認する前に、目標を入力してください。");
      return;
    }
    if (spec.model.trim() === "") {
      setError("契約を確認する前に、実行モデルの正確な ID を入力してください。");
      return;
    }
    if (spec.planner_model.trim() === "") {
      setError("契約を確認する前に、計画モデルの正確な ID を入力してください。");
      return;
    }
    if (!trialAccessReady) {
      setError("契約を確認する前に、実行時の Trial アクセストークンを入力してください。");
      return;
    }
    setBusy(true);
    setError(null);
    setErrorReconnectSessionId(null);
    try {
      setWorkspaceLease(await fetchWorkspaceLease(trialToken));
      setProposal(await proposeSession(trialToken, spec));
      setConfirmed(false);
      setStage("gate_1");
    } catch (reason) {
      recordError(reason);
    } finally {
      setBusy(false);
    }
  }

  async function inspectWorkspaceLease() {
    if (!trialAccessReady) {
      setError("ワークスペースのリースを確認する前に、実行時の Trial アクセストークンを入力してください。");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      setWorkspaceLease(await fetchWorkspaceLease(trialToken));
    } catch (reason) {
      recordError(reason);
    } finally {
      setBusy(false);
    }
  }

  async function launchConfirmed() {
    if (!confirmed || proposal === null) {
      setError("起動するには Gate 1 の明示的な確認が必要です。");
      return;
    }
    setBusy(true);
    setError(null);
    setErrorReconnectSessionId(null);
    try {
      const value = await launchSession(trialToken, spec, proposal.card_hash);
      setCreated(value);
      setWorkspaceLease(null);
      setReconnectSessionId(value.id);
      replaceSessionQuery(value.id);
      setSession(null);
      setMonitor(initialMonitor);
      const startedAt = epochMilliseconds(value.started_epoch_seconds) ?? Date.now();
      setGateTwoStartedAt(startedAt);
      setElapsedSeconds(elapsedSince(startedAt));
      setArtifacts([]);
      setEvidenceDocument(null);
      setEvidenceOpen(false);
      setEvidenceError(null);
      setSessionIndexRevision((current) => current + 1);
      setStage("gate_2");
    } catch (reason) {
      if (reason instanceof GuiRequestError && reason.status === 409) {
        const currentLease = await fetchWorkspaceLease(trialToken).catch(() => null);
        if (currentLease !== null) setWorkspaceLease(currentLease);
      }
      recordError(reason);
    } finally {
      setBusy(false);
    }
  }

  async function reconnectExisting(requestedId?: string) {
    const id = (requestedId ?? reconnectSessionId).trim();
    if (id === "" || !trialAccessReady) {
      setError(
        id === ""
          ? "再接続するセッション ID を入力してください。"
          : "実行時の Trial アクセストークンを入力してください。",
      );
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const value = await fetchSession(id, trialToken);
      const lastSuccessAt = new Date().toISOString();
      setSession(value);
      setCreated({
        id: value.id,
        started_epoch_seconds: value.started_epoch_seconds,
        gate: "gate_2",
        status: "starting",
        events_path: value.events_path,
      });
      setReconnectSessionId(value.id);
      setErrorReconnectSessionId(null);
      replaceSessionQuery(value.id);
      setMonitor({
        attempt: 0,
        guidance: null,
        lastSuccessAt,
        retryInMs: null,
        status: "connected",
        summary: null,
      });
      const startedAt = epochMilliseconds(value.started_epoch_seconds) ?? Date.now();
      setGateTwoStartedAt(startedAt);
      setElapsedSeconds(elapsedSince(startedAt));
      setSessionIndexRevision((current) => current + 1);
      setStage(value.gate === "gate_3" || value.gate === "gate_4" ? "terminal" : "gate_2");
    } catch (reason) {
      const failure = monitorFailure(reason);
      rejectTrialToken(reason, trialToken);
      setError(failure.guidance);
    } finally {
      setBusy(false);
    }
  }

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
      setSession(null);
      setSessionIndexRevision((current) => current + 1);
      setStage("gate_2");
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

  const launchBlockReason = leaseLaunchBlockReason(workspaceLease);
  const observedSession = useMemo(() => {
    if (created === null) return null;
    if (session !== null && session.id === created.id) {
      return { gate: session.gate, id: session.id, status: session.status };
    }
    return { gate: created.gate, id: created.id, status: created.status };
  }, [created, session]);

  function recordError(reason: unknown) {
    rejectTrialToken(reason, trialToken);
    setError(describeError(reason));
    const active = reconnectIdFromError(reason);
    setErrorReconnectSessionId(active);
    if (active !== null) {
      setReconnectSessionId(active);
      replaceSessionQuery(active);
    }
  }

  return {
    artifacts,
    busy,
    checkContract,
    confirmDirective,
    confirmed,
    created,
    currentPhase,
    directive,
    directiveText,
    elapsedSeconds,
    error,
    errorReconnectSessionId,
    evidenceDocument,
    evidenceError,
    evidenceLoading,
    evidenceOpen,
    executionRef,
    gateOneRef,
    inspectWorkspaceLease,
    launchBlockReason,
    launchConfirmed,
    launchIdentityLocked,
    loadArtifacts,
    monitor,
    observedSession,
    optionsError,
    compatiblePacks,
    persistDirective,
    priceCost,
    priceDuration,
    proposal,
    providerChanged,
    readArtifact,
    readEvents,
    reconnectExisting,
    reconnectSessionId,
    rejectTrialToken,
    selectedProfile,
    selectedProvider,
    selectedPack,
    session,
    sessionIndexRevision,
    setConfirmed,
    setDirective,
    setDirectiveText,
    setProposal,
    setProviderChanged,
    setReconnectSessionId,
    setStage,
    setWorkspaceLease,
    spec,
    stage,
    startNewRun,
    terminalRef,
    trialAccessReady,
    trialOptions,
    trialToken,
    trialTokenAuthEnabled,
    update,
    updateTrialToken,
    workspaceLease,
  };
}

function replaceSessionQuery(id: string) {
  const url = new URL(window.location.href);
  url.searchParams.set("session", id);
  window.history.replaceState(null, "", `${url.pathname}${url.search}${url.hash}`);
}

function epochMilliseconds(epochSeconds: number): number | null {
  return Number.isFinite(epochSeconds) && epochSeconds > 0 ? epochSeconds * 1_000 : null;
}

function elapsedSince(startedAt: number): number {
  return Math.max(0, Math.floor((Date.now() - startedAt) / 1_000));
}

function leaseLaunchBlockReason(lease: TrialWorkspaceLease | null): string | null {
  if (lease === null || lease.status === "idle") return null;
  if (lease.status === "running") {
    return `実行中のセッション ${lease.session_id} がワークスペースを使用しているため、新しい起動はできません。`;
  }
  return `セッション ${lease.session_id} のワークスペース復旧が必要なため、新しい起動はできません。`;
}
