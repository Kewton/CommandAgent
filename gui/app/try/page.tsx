"use client";

import { useEffect, useMemo, useRef, useState } from "react";

import { Shell } from "../../components/shell";
import { apiPath } from "../../lib/base-path";
import {
  POLL_INTERVAL_MS,
  TERMINAL_FAILURE_LIMIT,
  type MonitorFailure,
  type MonitorStatus,
  responseFailure,
  retryDelay,
  thrownFailure,
} from "../../lib/trial-monitor";
import type {
  CreatedSession,
  DirectiveProposal,
  PolledSession,
  SessionProposal,
  SessionSpec,
} from "../../lib/types";

const initialSpec: SessionSpec = {
  goal: "Create a CLI --pattern filter command",
  profile: "python-cli",
  provider: "ollama",
  model: "qwen3:8b",
  planner_provider: "ollama",
  planner_model: "qwen3:8b",
};

type ScreenStage = "compose" | "gate_1" | "gate_2" | "terminal" | "closed";

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

export default function TrialRunPage() {
  const gateOneRef = useRef<HTMLElement>(null);
  const executionRef = useRef<HTMLElement>(null);
  const terminalRef = useRef<HTMLElement>(null);
  const [trialToken, setTrialToken] = useState("");
  const [reconnectSessionId, setReconnectSessionId] = useState("");
  const [spec, setSpec] = useState<SessionSpec>(initialSpec);
  const [proposal, setProposal] = useState<SessionProposal | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const [created, setCreated] = useState<CreatedSession | null>(null);
  const [session, setSession] = useState<PolledSession | null>(null);
  const [stage, setStage] = useState<ScreenStage>("compose");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [directiveText, setDirectiveText] = useState("");
  const [directive, setDirective] = useState<DirectiveProposal | null>(null);
  const [monitor, setMonitor] = useState<MonitorState>(initialMonitor);

  useEffect(() => {
    const id = new URLSearchParams(window.location.search).get("session");
    if (id !== null) setReconnectSessionId(id);
  }, []);

  useEffect(() => {
    if (
      created === null ||
      trialToken.trim() === "" ||
      stage === "closed" ||
      stage === "terminal"
    ) {
      return;
    }
    let cancelled = false;
    let attempt = 0;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const poll = async () => {
      try {
        const value = await fetchSession(created.id, trialToken);
        if (cancelled) return;
        attempt = 0;
        setSession(value);
        setMonitor({
          attempt: 0,
          guidance: null,
          lastSuccessAt: new Date().toISOString(),
          retryInMs: null,
          status: "connected",
          summary: null,
        });
        if (value.gate === "gate_3" || value.gate === "gate_4") {
          setStage("terminal");
          return;
        }
        setStage("gate_2");
        timer = setTimeout(() => void poll(), POLL_INTERVAL_MS);
      } catch (reason) {
        if (cancelled) return;
        attempt += 1;
        const failure = monitorFailure(reason);
        const stop = failure.terminal && attempt >= TERMINAL_FAILURE_LIMIT;
        const delay = retryDelay(attempt);
        setMonitor((current) => ({
          attempt,
          guidance: stop
            ? `Monitoring stopped after ${attempt} attempts. ${failure.guidance}`
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
  }, [created, stage, trialToken]);

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
    const seconds = proposal?.price.average_duration_seconds;
    return seconds === null || seconds === undefined
      ? "not recorded"
      : `${(seconds / 60).toFixed(1)} min mean`;
  }, [proposal]);
  const priceCost = useMemo(() => {
    const cost = proposal?.price.average_cost_usd;
    return cost === null || cost === undefined ? "not recorded" : `$${cost.toFixed(4)} mean`;
  }, [proposal]);

  function update<K extends keyof SessionSpec>(field: K, value: SessionSpec[K]) {
    setSpec((current) => ({ ...current, [field]: value }));
    setProposal(null);
    setConfirmed(false);
    setStage("compose");
  }

  async function checkContract() {
    if (trialToken.trim() === "") {
      setError("Enter the runtime Trial access token before checking the contract.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const response = await fetch(apiPath("session-proposals"), {
        method: "POST",
        headers: authorizationHeaders(trialToken, true),
        body: JSON.stringify(spec),
      });
      if (!response.ok) throw new Error(await apiError(response));
      setProposal((await response.json()) as SessionProposal);
      setConfirmed(false);
      setStage("gate_1");
    } catch (reason) {
      setError(message(reason));
    } finally {
      setBusy(false);
    }
  }

  async function launchConfirmed() {
    if (!confirmed || proposal === null) {
      setError("Gate 1 must be explicitly confirmed before launch.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const response = await fetch(apiPath("sessions"), {
        method: "POST",
        headers: authorizationHeaders(trialToken, true),
        body: JSON.stringify({ ...spec, confirmation_hash: proposal.card_hash }),
      });
      if (!response.ok) {
        const detail = await apiError(response);
        if (response.status === 409) {
          const active = sessionIdFromConflict(detail);
          if (active !== null) {
            setReconnectSessionId(active);
            replaceSessionQuery(active);
            throw new Error(
              `${detail}. Reconnect to session ${active} below; reconnect monitoring performs GET only.`,
            );
          }
        }
        throw new Error(detail);
      }
      const value = (await response.json()) as CreatedSession;
      setCreated(value);
      setReconnectSessionId(value.id);
      replaceSessionQuery(value.id);
      setSession(null);
      setMonitor(initialMonitor);
      setStage("gate_2");
    } catch (reason) {
      setError(message(reason));
    } finally {
      setBusy(false);
    }
  }

  async function reconnectExisting() {
    const id = reconnectSessionId.trim();
    if (id === "" || trialToken.trim() === "") {
      setError("Enter an existing session ID and the runtime Trial access token to reconnect.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const value = await fetchSession(id, trialToken);
      const lastSuccessAt = new Date().toISOString();
      setSession(value);
      setCreated({ id: value.id, gate: "gate_2", status: "starting", events_path: value.events_path });
      setReconnectSessionId(value.id);
      replaceSessionQuery(value.id);
      setMonitor({
        attempt: 0,
        guidance: null,
        lastSuccessAt,
        retryInMs: null,
        status: "connected",
        summary: null,
      });
      setStage(value.gate === "gate_3" || value.gate === "gate_4" ? "terminal" : "gate_2");
    } catch (reason) {
      const failure = monitorFailure(reason);
      setError(failure.guidance);
    } finally {
      setBusy(false);
    }
  }

  async function persistDirective() {
    if (created === null || directiveText.trim() === "") return;
    setBusy(true);
    setError(null);
    try {
      const response = await fetch(
        apiPath(`sessions/${encodeURIComponent(created.id)}/directives`),
        {
          method: "POST",
          headers: authorizationHeaders(trialToken, true),
          body: JSON.stringify({ directive: directiveText }),
        },
      );
      if (!response.ok) throw new Error(await apiError(response));
      setDirective((await response.json()) as DirectiveProposal);
    } catch (reason) {
      setError(message(reason));
    } finally {
      setBusy(false);
    }
  }

  async function confirmDirective() {
    if (created === null || directive === null) return;
    setBusy(true);
    setError(null);
    try {
      const response = await fetch(
        apiPath(
          `sessions/${encodeURIComponent(created.id)}/directives/${encodeURIComponent(directive.directive_hash)}`,
        ),
        { method: "POST", headers: authorizationHeaders(trialToken, true), body: "{}" },
      );
      if (!response.ok) throw new Error(await apiError(response));
      setDirective(null);
      setDirectiveText("");
      setStage("gate_2");
    } catch (reason) {
      setError(message(reason));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Shell
      active="try"
      eyebrow="02 / CONFIRMED TRIAL"
      title="Launch once. Trust the gates."
      description="The GUI confirms and launches. The existing CLI executes; filesystem events and acceptance artifacts remain authoritative."
    >
      <section className="trial-layout">
        <div className="trial-compose panel">
          <header className="panel-heading">
            <div>
              <span className="panel-index">GATE 1 / REQUEST</span>
              <h2>Frozen launch identity</h2>
            </div>
            <span className="gate-chip">{stageLabel(stage, session)}</span>
          </header>
          <label htmlFor="trial-goal">Goal</label>
          <textarea
            data-testid="trial-goal"
            id="trial-goal"
            onChange={(event) => update("goal", event.target.value)}
            rows={5}
            value={spec.goal}
          />
          <label htmlFor="trial-token">Trial access token</label>
          <input
            autoComplete="off"
            autoCapitalize="none"
            data-testid="trial-token"
            id="trial-token"
            onChange={(event) => {
              setTrialToken(event.target.value);
              if (created === null) {
                setProposal(null);
                setConfirmed(false);
                setStage("compose");
              }
            }}
            spellCheck={false}
            type="password"
            value={trialToken}
          />
          <div className="reconnect-card" data-testid="reconnect-card">
            <label htmlFor="reconnect-session">Existing session ID</label>
            <div>
              <input
                autoCapitalize="none"
                autoComplete="off"
                data-testid="reconnect-session"
                id="reconnect-session"
                onChange={(event) => setReconnectSessionId(event.target.value)}
                spellCheck={false}
                value={reconnectSessionId}
              />
              <button
                className="secondary-action"
                data-testid="reconnect-session-button"
                disabled={busy || reconnectSessionId.trim() === "" || trialToken.trim() === ""}
                onClick={() => void reconnectExisting()}
                type="button"
              >
                Reconnect monitoring
              </button>
            </div>
            <small>GET only. This cannot dispatch another CLI process.</small>
          </div>
          <div className="trial-fields">
            <label>
              Profile
              <select value={spec.profile} onChange={(event) => update("profile", event.target.value)}>
                <option value="python-cli">python-cli</option>
                <option value="data">data</option>
                <option value="ingest">ingest</option>
                <option value="nextjs">nextjs</option>
              </select>
            </label>
            <label>
              Provider
              <select value={spec.provider} onChange={(event) => update("provider", event.target.value)}>
                <option value="ollama">ollama</option>
                <option value="lm-studio">LM Studio</option>
                <option value="openai">openai</option>
                <option value="gemini">gemini</option>
              </select>
            </label>
            <label>
              Executor model
              <input value={spec.model} onChange={(event) => update("model", event.target.value)} />
            </label>
            <label>
              Planner model
              <input
                value={spec.planner_model}
                onChange={(event) => update("planner_model", event.target.value)}
              />
            </label>
          </div>
          <button
            className="secondary-action"
            data-testid="check-contract"
            disabled={busy || stage === "gate_2"}
            onClick={() => void checkContract()}
            type="button"
          >
            Check contract and price
          </button>
          {error !== null && <p className="trial-error" role="alert">{error}</p>}
        </div>

        <aside className="trial-rail">
          <div className={`rail-step ${stage !== "compose" ? "reached" : ""}`}>
            <span>1</span><div><strong>Gate 1</strong><small>Human confirmation</small></div>
          </div>
          <div className={`rail-step ${stage === "gate_2" || stage === "terminal" ? "reached" : ""}`}>
            <span>2</span><div><strong>Execute</strong><small>Existing CLI only</small></div>
          </div>
          <div className={`rail-step ${stage === "terminal" ? "reached" : ""}`}>
            <span>3</span><div><strong>Gate 3 / 4</strong><small>Artifact verdict</small></div>
          </div>
        </aside>
      </section>

      {proposal !== null && (stage === "gate_1" || stage === "gate_2") && (
        <section className="gate-one-grid" data-testid="gate-one-card" ref={gateOneRef}>
          <article className="panel contract-card">
            <span className="panel-index">CONTRACT</span>
            <h2>{proposal.identity.profile} × {proposal.identity.intent} × {proposal.identity.task_family}</h2>
            <code>{proposal.identity.contract_ref}</code>
            <ul>
              {proposal.identity.contract_checks.map((check) => <li key={check}>{check}</li>)}
            </ul>
            <p>{proposal.identity.full_meaning}</p>
            <div className="workspace-boundary" data-testid="trial-workspace">
              <strong>Filesystem write boundary</strong>
              <code>{proposal.identity.workspace}</code>
              <p>The delegated CLI may create, modify, or delete content inside this directory.</p>
            </div>
          </article>
          <article className="panel price-card">
            <span className="panel-index">MEASURED PRICE TAG</span>
            <div className="price-rate">
              <strong>{proposal.identity.band_rate}</strong>
              <span>{proposal.identity.band_full}/{proposal.identity.band_denominator} full</span>
            </div>
            <dl>
              <div><dt>Mean duration</dt><dd>{priceDuration} · n={proposal.price.duration_n}</dd></div>
              <div><dt>Mean cost</dt><dd>{priceCost} · n={proposal.price.cost_n}</dd></div>
              <div><dt>Measurement</dt><dd>{proposal.identity.band_measurement}</dd></div>
            </dl>
            <label className="confirm-check">
              <input
                checked={confirmed}
                data-testid="gate-one-confirm"
                onChange={(event) => setConfirmed(event.target.checked)}
                type="checkbox"
              />
              I confirm this exact contract, model pin, measured value tag, and displayed filesystem write boundary.
            </label>
            <code className="hash-line">{proposal.card_hash}</code>
            <button
              className="primary-action"
              data-testid="launch-session"
              disabled={!confirmed || busy || stage === "gate_2"}
              onClick={() => void launchConfirmed()}
              type="button"
            >
              Confirm and delegate to CLI
            </button>
          </article>
        </section>
      )}

      {(stage === "gate_2" || stage === "terminal") && created !== null && (
        <section className="panel execution-panel" data-testid="session-progress" ref={executionRef}>
          <header className="panel-heading">
            <div><span className="panel-index">GATE 2 / FILE-BACKED PROGRESS</span><h2>{created.id}</h2></div>
            <span className={`live-label ${monitor.status === "connected" ? "connected" : ""}`}>
              <i /> execution: {session?.status ?? "starting"}
            </span>
          </header>
          <div
            className={`monitor-state ${monitor.status}`}
            data-monitor-status={monitor.status}
            data-testid="monitor-state"
          >
            <div>
              <strong>Monitoring: {monitor.status}</strong>
              <span>
                Last successful update: {formatLastSuccess(monitor.lastSuccessAt)}
              </span>
            </div>
            <small>
              {monitor.summary ?? "Waiting for the next file-backed status update."}
              {monitor.retryInMs === null
                ? ""
                : ` Retry ${monitor.attempt} in ${(monitor.retryInMs / 1000).toFixed(2)}s.`}
            </small>
            {monitor.guidance !== null && <p>{monitor.guidance}</p>}
          </div>
          <div className="phase-list">
            {session?.phases.length === 0 && <p>Waiting for the first CLI event…</p>}
            {session?.phases.map((phase) => (
              <div className={`phase-row ${phase.status}`} key={`${phase.index}-${phase.id}`}>
                <span>{String(phase.index).padStart(2, "0")}</span>
                <div><strong>{phase.id}</strong><small>{phase.stage}</small></div>
                <em>{phase.status}</em>
              </div>
            ))}
          </div>
          <footer><code>{session?.events_path ?? created.events_path}</code><span>{session?.event_count ?? 0} events</span></footer>
        </section>
      )}

      {stage === "terminal" && session !== null && (
        <section className="terminal-grid" data-testid="terminal-gate" ref={terminalRef}>
          <article className="panel verdict-card">
            <span className="panel-index">{session.gate.toUpperCase()} / TERMINAL</span>
            <h2>{session.verdict ?? session.status}</h2>
            <p>Assurance: <strong>{session.assurance ?? "not recorded"}</strong></p>
            <pre>{session.acceptance_sheet ?? "Terminal evidence is incomplete; no sheet was promoted."}</pre>
          </article>
          <aside className="panel next-action-card">
            <span className="panel-index">NEXT ACTION / D-3d</span>
            <h2>Boundary instruction</h2>
            <p>Saved text is scrubbed and hashed. It cannot alter the frozen contract floor.</p>
            <textarea
              data-testid="directive-input"
              onChange={(event) => { setDirectiveText(event.target.value); setDirective(null); }}
              placeholder="Add a post-terminal instruction…"
              rows={4}
              value={directiveText}
            />
            <button className="secondary-action" disabled={busy || directive !== null || directiveText.trim() === ""} onClick={() => void persistDirective()} type="button">
              Scrub and persist instruction
            </button>
            {directive !== null && (
              <div className="directive-receipt" data-testid="directive-receipt">
                <strong>{directive.scrubbed_directive}</strong>
                <code>{directive.directive_hash}</code>
                <small>{directive.issued_gate} · round {directive.directive_round}</small>
                <button className="primary-action" disabled={busy} onClick={() => void confirmDirective()} type="button">
                  Confirm D-3d continuation
                </button>
              </div>
            )}
            <button className="close-action" onClick={() => setStage("closed")} type="button">End without another run</button>
          </aside>
        </section>
      )}

      {stage === "closed" && <section className="panel closed-card"><span>SESSION CLOSED</span><h2>No further action was dispatched.</h2></section>}
    </Shell>
  );
}

function stageLabel(stage: ScreenStage, session: PolledSession | null): string {
  if (stage === "terminal") return session?.gate.toUpperCase() ?? "TERMINAL";
  if (stage === "gate_2") return "GATE 2";
  if (stage === "gate_1") return "AWAITING CONFIRMATION";
  if (stage === "closed") return "CLOSED";
  return "DRAFT";
}

async function fetchSession(id: string, token: string): Promise<PolledSession> {
  let response: Response;
  try {
    response = await fetch(apiPath(`sessions/${encodeURIComponent(id)}`), {
      headers: authorizationHeaders(token),
      redirect: "manual",
    });
  } catch (reason) {
    throw thrownFailure(reason);
  }
  if (response.type === "opaqueredirect" || !response.ok) {
    throw await responseFailure(response);
  }
  try {
    return (await response.json()) as PolledSession;
  } catch (reason) {
    throw {
      guidance:
        "Monitoring received an invalid status response. Inspect the proxy response and existing session artifacts before reconnecting.",
      summary: message(reason),
      terminal: true,
    } satisfies MonitorFailure;
  }
}

function monitorFailure(reason: unknown): MonitorFailure {
  if (isMonitorFailure(reason)) return reason;
  return thrownFailure(reason);
}

function isMonitorFailure(reason: unknown): reason is MonitorFailure {
  if (typeof reason !== "object" || reason === null) return false;
  const candidate = reason as Partial<MonitorFailure>;
  return (
    typeof candidate.guidance === "string" &&
    typeof candidate.summary === "string" &&
    typeof candidate.terminal === "boolean"
  );
}

function sessionIdFromConflict(detail: string): string | null {
  return detail.match(/(?:already running session|non-terminal session) ([0-9a-f-]{36})/i)?.[1] ?? null;
}

function replaceSessionQuery(id: string) {
  const url = new URL(window.location.href);
  url.searchParams.set("session", id);
  window.history.replaceState(null, "", `${url.pathname}${url.search}${url.hash}`);
}

function formatLastSuccess(value: string | null): string {
  if (value === null) return "not yet connected";
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "medium",
  }).format(new Date(value));
}

async function apiError(response: Response): Promise<string> {
  const text = await response.text();
  try {
    const parsed = JSON.parse(text) as { error?: string };
    return `${response.status}: ${parsed.error ?? text}`;
  } catch {
    return `${response.status}: ${text}`;
  }
}

function message(reason: unknown): string {
  return reason instanceof Error ? reason.message : "The trial request failed.";
}

function authorizationHeaders(token: string, json = false): Record<string, string> {
  return {
    "x-commandagent-trial-authorization": `Bearer ${token.trim()}`,
    ...(json ? { "content-type": "application/json" } : {}),
  };
}
