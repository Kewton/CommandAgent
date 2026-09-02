"use client";

import {
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type Dispatch,
  type SetStateAction,
} from "react";

import {
  CHANGED_POLL_INTERVAL_MS,
  TERMINAL_FAILURE_LIMIT,
  type MonitorStatus,
  monitorFailure,
  retryDelay,
  unchangedPollDelay,
} from "../lib/trial-monitor";
import { describeError } from "../lib/errors";
import { fetchSession, fetchSessionPoll, stopSession } from "../lib/trial-api";
import type { CreatedSession, PolledSession } from "../lib/types";
import type { ScreenStage } from "./use-trial-compose";

type MonitorState = {
  attempt: number;
  guidance: string | null;
  lastSuccessAt: string | null;
  retryInMs: number | null;
  status: MonitorStatus;
  summary: string | null;
};

export type TrialStopState = "idle" | "confirming" | "stopping" | "failed";

type UseTrialMonitorProps = {
  reconnectSessionId: string;
  rejectTrialToken: (reason: unknown, rejectedValue: string) => boolean;
  setBusy: Dispatch<SetStateAction<boolean>>;
  setError: Dispatch<SetStateAction<string | null>>;
  setErrorReconnectSessionId: Dispatch<SetStateAction<string | null>>;
  setReconnectSessionId: Dispatch<SetStateAction<string>>;
  setStage: Dispatch<SetStateAction<ScreenStage>>;
  stage: ScreenStage;
  trialAccessReady: boolean;
  trialToken: string;
};

const initialMonitor: MonitorState = {
  attempt: 0,
  guidance: null,
  lastSuccessAt: null,
  retryInMs: null,
  status: "degraded",
  summary: null,
};

export function useTrialMonitor(props: UseTrialMonitorProps) {
  const {
    reconnectSessionId, rejectTrialToken, setBusy, setError,
    setErrorReconnectSessionId, setReconnectSessionId, setStage, stage,
    trialAccessReady, trialToken,
  } = props;
  const executionRef = useRef<HTMLElement>(null);
  const automaticReconnectAttempt = useRef<string | null>(null);
  const [created, setCreated] = useState<CreatedSession | null>(null);
  const [session, setSession] = useState<PolledSession | null>(null);
  const [gateTwoStartedAt, setGateTwoStartedAt] = useState<number | null>(null);
  const [elapsedSeconds, setElapsedSeconds] = useState(0);
  const [monitor, setMonitor] = useState<MonitorState>(initialMonitor);
  const [sessionIndexRevision, setSessionIndexRevision] = useState(0);
  const [stopState, setStopState] = useState<TrialStopState>("idle");
  const [stopError, setStopError] = useState<string | null>(null);

  useLayoutEffect(() => {
    const id = requestedSessionId();
    if (id !== null) replaceMonitoredSessionQuery(id);
  }, []);

  useEffect(() => {
    if (
      created === null || !trialAccessReady || stage === "closed" || stage === "terminal"
    ) return;
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
  }, [created, rejectTrialToken, setStage, stage, trialAccessReady, trialToken]);

  useEffect(() => {
    if (gateTwoStartedAt === null || stage !== "gate_2") return;
    const tick = () => setElapsedSeconds(elapsedSince(gateTwoStartedAt));
    tick();
    const timer = window.setInterval(tick, 1_000);
    return () => window.clearInterval(timer);
  }, [gateTwoStartedAt, stage]);

  useEffect(() => {
    const id = requestedSessionId();
    if (stage !== "compose" || id === null || !trialAccessReady) return;
    const attempt = reconnectAttemptKey(id, trialToken);
    if (automaticReconnectAttempt.current === attempt) return;
    automaticReconnectAttempt.current = attempt;
    void reconnectExisting(id).then((restored) => {
      if (!restored && automaticReconnectAttempt.current === attempt) {
        automaticReconnectAttempt.current = null;
      }
    });
  }, [reconnectSessionId, stage, trialAccessReady, trialToken]);

  useEffect(() => {
    if (stage !== "terminal" || session === null) return;
    const frame = window.requestAnimationFrame(() => {
      const heading = document.querySelector("[data-testid='terminal-result-heading']")
        ?.textContent?.trim();
      if (heading === undefined || heading === "") return;
      const marker = session.gate === "gate_3" ? "✔" : "✗";
      document.title = `${marker} ${heading} | CommandAgent`;
    });
    return () => window.cancelAnimationFrame(frame);
  }, [session, stage]);

  function acceptLaunch(value: CreatedSession) {
    setCreated(value);
    setReconnectSessionId(value.id);
    replaceMonitoredSessionQuery(value.id);
    automaticReconnectAttempt.current = reconnectAttemptKey(value.id, trialToken);
    setSession(null);
    setMonitor(initialMonitor);
    setStopState("idle");
    setStopError(null);
    const startedAt = epochMilliseconds(value.started_epoch_seconds) ?? Date.now();
    setGateTwoStartedAt(startedAt);
    setElapsedSeconds(elapsedSince(startedAt));
    setSessionIndexRevision((current) => current + 1);
    setStage("gate_2");
  }

  async function reconnectExisting(requestedId?: string): Promise<boolean> {
    const id = (requestedId ?? reconnectSessionId).trim();
    if (id === "" || !trialAccessReady) {
      setError(
        id === ""
          ? "再接続するセッション ID を入力してください。"
          : "実行時のトライアルアクセストークンを入力してください。",
      );
      return false;
    }
    setBusy(true);
    setError(null);
    try {
      const value = await fetchSession(id, trialToken);
      const lastSuccessAt = new Date().toISOString();
      setSession(value);
      setCreated({
        id: value.id,
        process_generation: value.process_generation,
        started_epoch_seconds: value.started_epoch_seconds,
        gate: "gate_2",
        status: "starting",
        events_path: value.events_path,
      });
      setReconnectSessionId(value.id);
      setErrorReconnectSessionId(null);
      replaceMonitoredSessionQuery(value.id);
      automaticReconnectAttempt.current = reconnectAttemptKey(value.id, trialToken);
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
      return true;
    } catch (reason) {
      const failure = monitorFailure(reason);
      rejectTrialToken(reason, trialToken);
      setError(failure.guidance);
      return false;
    } finally {
      setBusy(false);
    }
  }

  function resumeForDirective(processGeneration: string) {
    setCreated((current) => current === null ? null : {
      ...current,
      process_generation: processGeneration,
    });
    setSession(null);
    setStopState("idle");
    setStopError(null);
    setSessionIndexRevision((current) => current + 1);
    setStage("gate_2");
  }

  function resetForNewRun() {
    setCreated(null);
    setSession(null);
    setGateTwoStartedAt(null);
    setElapsedSeconds(0);
    setStopState("idle");
    setStopError(null);
    automaticReconnectAttempt.current = null;
    clearSessionQuery();
  }

  const currentPhase = useMemo(() => {
    const phases = session?.phases ?? [];
    return phases.find((phase) => phase.status === "running") ?? phases[phases.length - 1] ?? null;
  }, [session]);
  const observedSession = useMemo(() => {
    if (created === null) return null;
    if (session !== null && session.id === created.id) {
      return { gate: session.gate, id: session.id, status: session.status };
    }
    return { gate: created.gate, id: created.id, status: created.status };
  }, [created, session]);

  function confirmStop() {
    if (stage !== "gate_2" || created?.process_generation == null) return;
    setStopError(null);
    setStopState("confirming");
  }

  function cancelStop() {
    if (stopState === "confirming" || stopState === "failed") {
      setStopError(null);
      setStopState("idle");
    }
  }

  async function stopActiveSession() {
    if (
      stage !== "gate_2" || created === null ||
      created.process_generation === null || stopState === "stopping"
    ) return;
    setStopState("stopping");
    setStopError(null);
    try {
      await stopSession(trialToken, created.id, created.process_generation);
    } catch (reason) {
      rejectTrialToken(reason, trialToken);
      setStopError(describeError(reason));
      setStopState("failed");
    }
  }

  return {
    acceptLaunch, cancelStop, confirmStop, created, currentPhase, elapsedSeconds, executionRef, monitor,
    observedSession, reconnectExisting, resetForNewRun, resumeForDirective,
    session, sessionIndexRevision, stopActiveSession, stopError, stopState,
  };
}

function epochMilliseconds(epochSeconds: number): number | null {
  return Number.isFinite(epochSeconds) && epochSeconds > 0 ? epochSeconds * 1_000 : null;
}

function elapsedSince(startedAt: number): number {
  return Math.max(0, Math.floor((Date.now() - startedAt) / 1_000));
}

function requestedSessionId(): string | null {
  const value = new URLSearchParams(window.location.search).get("session")?.trim();
  return value === undefined || value === "" ? null : value;
}

function reconnectAttemptKey(id: string, token: string): string {
  return `${id}\n${token.trim()}`;
}

function replaceMonitoredSessionQuery(id: string) {
  const url = new URL(window.location.href);
  url.searchParams.set("session", id);
  url.searchParams.delete("sample");
  replaceLocationQuery(url);
}

function clearSessionQuery() {
  const url = new URL(window.location.href);
  url.searchParams.delete("session");
  replaceLocationQuery(url);
}

function replaceLocationQuery(url: URL) {
  window.history.replaceState(null, "", `${url.pathname}${url.search}${url.hash}`);
}
