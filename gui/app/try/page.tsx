"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { DocumentViewer } from "../../components/document-viewer";
import { Shell } from "../../components/shell";
import { apiPath } from "../../lib/base-path";
import {
  CHANGED_POLL_INTERVAL_MS,
  TERMINAL_FAILURE_LIMIT,
  type MonitorFailure,
  type MonitorStatus,
  responseFailure,
  retryDelay,
  thrownFailure,
  unchangedPollDelay,
} from "../../lib/trial-monitor";
import type {
  CreatedSession,
  DirectiveProposal,
  DocumentRecord,
  DocumentSummary,
  PolledSession,
  SessionProposal,
  SessionSpec,
  TrialOptions,
  TrialWorkspaceLease,
} from "../../lib/types";

const initialSpec: SessionSpec = {
  goal: "",
  profile: "python-cli",
  provider: "ollama",
  model: "",
  planner_provider: "ollama",
  planner_model: "",
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
  const [gateTwoStartedAt, setGateTwoStartedAt] = useState<number | null>(null);
  const [elapsedSeconds, setElapsedSeconds] = useState(0);
  const [stage, setStage] = useState<ScreenStage>("compose");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [trialOptions, setTrialOptions] = useState<TrialOptions | null>(null);
  const [optionsError, setOptionsError] = useState<string | null>(null);
  const [providerChanged, setProviderChanged] = useState(false);
  const [directiveText, setDirectiveText] = useState("");
  const [directive, setDirective] = useState<DirectiveProposal | null>(null);
  const [workspaceLease, setWorkspaceLease] = useState<TrialWorkspaceLease | null>(null);
  const launchIdentityLocked =
    stage === "gate_2" || stage === "terminal" || stage === "closed";
  const [monitor, setMonitor] = useState<MonitorState>(initialMonitor);
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
      const response = await fetch(
        apiPath(`sessions/${encodeURIComponent(created.id)}/artifacts`),
        { headers: authorizationHeaders(trialToken) },
      );
      if (!response.ok) throw new Error(await apiError(response));
      setArtifacts((await response.json()) as DocumentSummary[]);
    } catch (reason) {
      setEvidenceError(message(reason));
    } finally {
      setEvidenceLoading(false);
    }
  }, [created, trialToken]);

  useEffect(() => {
    let cancelled = false;
    const loadOptions = async () => {
      try {
        const response = await fetch(apiPath("trial-options"));
        if (!response.ok) throw new Error(await apiError(response));
        const value = (await response.json()) as TrialOptions;
        if (!cancelled) setTrialOptions(value);
      } catch (reason) {
        if (!cancelled) setOptionsError(`Trial の選択肢を読み込めませんでした: ${message(reason)}`);
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
        if (value.gate === "gate_3" || value.gate === "gate_4") {
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
        const stop = failure.terminal && attempt >= TERMINAL_FAILURE_LIMIT;
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
  }, [created, stage, trialToken]);

  useEffect(() => {
    if (gateTwoStartedAt === null || stage !== "gate_2") return;
    const tick = () => {
      setElapsedSeconds(Math.floor((Date.now() - gateTwoStartedAt) / 1_000));
    };
    tick();
    const timer = window.setInterval(tick, 1_000);
    return () => window.clearInterval(timer);
  }, [gateTwoStartedAt, stage]);

  useEffect(() => {
    if (stage !== "terminal" || session === null) return;
    const previousTitle = document.title;
    document.title = `✔ ${session.verdict ?? session.status} — CommandAgent`;
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
    const seconds = proposal?.price.average_duration_seconds;
    return seconds === null || seconds === undefined
      ? "未記録"
      : `平均 ${(seconds / 60).toFixed(1)} 分`;
  }, [proposal]);
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

  function update<K extends keyof SessionSpec>(field: K, value: SessionSpec[K]) {
    setSpec((current) => ({ ...current, [field]: value }));
    setProposal(null);
    setConfirmed(false);
    setError(null);
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
    if (trialToken.trim() === "") {
      setError("契約を確認する前に、実行時の Trial アクセストークンを入力してください。");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      setWorkspaceLease(await fetchWorkspaceLease(trialToken));
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

  async function inspectWorkspaceLease() {
    if (trialToken.trim() === "") {
      setError("ワークスペースのリースを確認する前に、実行時の Trial アクセストークンを入力してください。");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      setWorkspaceLease(await fetchWorkspaceLease(trialToken));
    } catch (reason) {
      setError(message(reason));
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
    try {
      const response = await fetch(apiPath("sessions"), {
        method: "POST",
        headers: authorizationHeaders(trialToken, true),
        body: JSON.stringify({ ...spec, confirmation_hash: proposal.card_hash }),
      });
      if (!response.ok) {
        const detail = await apiError(response);
        if (response.status === 409) {
          const currentLease = await fetchWorkspaceLease(trialToken).catch(() => null);
          if (currentLease !== null) setWorkspaceLease(currentLease);
          const active = sessionIdFromConflict(detail);
          if (active !== null) {
            setReconnectSessionId(active);
            replaceSessionQuery(active);
            throw new Error(
              `${detail}。下のセッション ${active} へ再接続してください。再接続の監視は GET のみを使用します。`,
            );
          }
        }
        throw new Error(detail);
      }
      const value = (await response.json()) as CreatedSession;
      setCreated(value);
      setWorkspaceLease(null);
      setReconnectSessionId(value.id);
      replaceSessionQuery(value.id);
      setSession(null);
      setMonitor(initialMonitor);
      setGateTwoStartedAt(Date.now());
      setElapsedSeconds(0);
      setArtifacts([]);
      setEvidenceDocument(null);
      setEvidenceOpen(false);
      setEvidenceError(null);
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
      setError("再接続するセッション ID と実行時の Trial アクセストークンを入力してください。");
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
      setGateTwoStartedAt(Date.now());
      setElapsedSeconds(0);
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
      setWorkspaceLease(null);
      setStage("gate_2");
    } catch (reason) {
      setError(message(reason));
    } finally {
      setBusy(false);
    }
  }

  async function readEvents() {
    if (created === null) return;
    const query = new URLSearchParams({ tail: "200" });
    await readEvidence(
      apiPath(`sessions/${encodeURIComponent(created.id)}/events`, query),
    );
  }

  async function readArtifact(path: string) {
    if (created === null) return;
    const query = new URLSearchParams({ path });
    await readEvidence(
      apiPath(`sessions/${encodeURIComponent(created.id)}/artifacts`, query),
    );
  }

  async function readEvidence(url: string) {
    setEvidenceOpen(true);
    setEvidenceLoading(true);
    setEvidenceError(null);
    try {
      const response = await fetch(url, { headers: authorizationHeaders(trialToken) });
      if (!response.ok) throw new Error(await apiError(response));
      setEvidenceDocument((await response.json()) as DocumentRecord);
    } catch (reason) {
      setEvidenceError(message(reason));
    } finally {
      setEvidenceLoading(false);
    }
  }

  return (
    <Shell
      active="try"
      title="トライアル"
      description="契約と書き込み先を確認してから、既存の CLI 実行を開始・監視します。"
    >
      <section className="trial-layout">
        <div className="trial-compose panel">
          <header className="panel-heading">
            <div>
              <span className="panel-index">GATE 1 / リクエスト</span>
              <h2>固定された起動条件</h2>
            </div>
            <span className="gate-chip">{stageLabel(stage, session)}</span>
          </header>
          <label htmlFor="trial-goal">目標</label>
          <textarea
            data-testid="trial-goal"
            disabled={launchIdentityLocked}
            id="trial-goal"
            onChange={(event) => update("goal", event.target.value)}
            rows={5}
            value={spec.goal}
          />
          <label htmlFor="trial-token">Trial アクセストークン</label>
          <input
            autoComplete="off"
            autoCapitalize="none"
            data-testid="trial-token"
            disabled={launchIdentityLocked}
            id="trial-token"
            onChange={(event) => {
              setTrialToken(event.target.value);
              setWorkspaceLease(null);
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
          <div
            className={`lease-status-card ${workspaceLease?.status ?? "unknown"}`}
            data-testid="workspace-lease-status"
          >
            <div>
              <span>ワークスペースのリース状態</span>
              <strong>{workspaceLeaseLabel(workspaceLease)}</strong>
            </div>
            {workspaceLease !== null && workspaceLease.status !== "idle" && (
              <code data-testid="workspace-lease-session">{workspaceLease.session_id}</code>
            )}
            <p>読み取り専用の確認です。リースの解除や CLI プロセスの起動は行いません。</p>
            <button
              className="secondary-action"
              data-testid="inspect-workspace-lease"
              disabled={busy}
              onClick={() => void inspectWorkspaceLease()}
              type="button"
            >
              ワークスペースのリースを確認
            </button>
          </div>
          <div className="reconnect-card" data-testid="reconnect-card">
            <label htmlFor="reconnect-session">既存セッション ID</label>
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
                監視を再接続
              </button>
            </div>
            <small>GET のみを使用し、別の CLI プロセスは起動しません。</small>
          </div>
          <div className="trial-fields">
            <label>
              プロファイル
              <select
                data-testid="trial-profile"
                disabled={launchIdentityLocked || trialOptions === null}
                value={spec.profile}
                onChange={(event) => update("profile", event.target.value)}
              >
                {trialOptions === null ? (
                  <option value={spec.profile}>許可済みプロファイルを読み込み中…</option>
                ) : (
                  trialOptions.profiles.map((option) => (
                    <option key={option.id} value={option.id}>{option.label}</option>
                  ))
                )}
              </select>
              {selectedProfile !== undefined && (
                <small className="trial-field-hint" data-testid="trial-profile-description">
                  {selectedProfile.description}
                </small>
              )}
            </label>
            <label>
              プロバイダー
              <select
                data-testid="trial-provider"
                disabled={launchIdentityLocked || trialOptions === null}
                value={spec.provider}
                onChange={(event) => {
                  update("provider", event.target.value);
                  setProviderChanged(true);
                }}
              >
                {trialOptions === null ? (
                  <option value={spec.provider}>プロバイダーを読み込み中…</option>
                ) : (
                  trialOptions.providers.map((option) => (
                    <option key={option.id} value={option.id}>{option.label}</option>
                  ))
                )}
              </select>
            </label>
            <label>
              実行モデル
              <input
                aria-describedby={providerChanged ? "trial-provider-model-hint" : undefined}
                data-testid="trial-executor-model"
                disabled={launchIdentityLocked}
                placeholder="正確なモデル ID"
                value={spec.model}
                onChange={(event) => update("model", event.target.value)}
              />
              {providerChanged && selectedProvider !== undefined && (
                <small
                  className="trial-model-warning"
                  data-testid="trial-provider-model-hint"
                  id="trial-provider-model-hint"
                  role="status"
                >
                  プロバイダーを変更しても実行モデルは自動更新されません。{selectedProvider.model_hint}
                </small>
              )}
            </label>
            <label>
              計画モデル
              <input
                data-testid="trial-planner-model"
                disabled={launchIdentityLocked}
                placeholder="正確なモデル ID"
                value={spec.planner_model}
                onChange={(event) => update("planner_model", event.target.value)}
              />
            </label>
          </div>
          <button
            className="secondary-action"
            data-testid="check-contract"
            disabled={busy || launchIdentityLocked}
            onClick={() => void checkContract()}
            type="button"
          >
            契約と価格を確認
          </button>
          {optionsError !== null && <p className="trial-error" role="alert">{optionsError}</p>}
          {error !== null && <p className="trial-error" role="alert">{error}</p>}
        </div>

        <aside className="trial-rail">
          <div className={`rail-step ${stage !== "compose" ? "reached" : ""}`}>
            <span>1</span><div><strong>Gate 1</strong><small>人による確認</small></div>
          </div>
          <div className={`rail-step ${stage === "gate_2" || stage === "terminal" ? "reached" : ""}`}>
            <span>2</span><div><strong>実行</strong><small>既存 CLI のみ</small></div>
          </div>
          <div className={`rail-step ${stage === "terminal" ? "reached" : ""}`}>
            <span>3</span><div><strong>Gate 3 / 4</strong><small>成果物の判定</small></div>
          </div>
        </aside>
      </section>

      {proposal !== null && (stage === "gate_1" || stage === "gate_2") && (
        <section className="gate-one-grid" data-testid="gate-one-card" ref={gateOneRef}>
          <article className="panel contract-card">
            <span className="panel-index">契約</span>
            <h2>{proposal.identity.profile} × {proposal.identity.intent} × {proposal.identity.task_family}</h2>
            <code>{proposal.identity.contract_ref}</code>
            <ul>
              {proposal.identity.contract_checks.map((check) => <li key={check}>{check}</li>)}
            </ul>
            <p>{proposal.identity.full_meaning}</p>
            <div className="workspace-boundary" data-testid="trial-workspace">
              <strong>ファイルシステムの書き込み境界</strong>
              <code>{proposal.identity.workspace}</code>
              <p>委譲された CLI は、このディレクトリ内の内容を作成・変更・削除できます。</p>
            </div>
          </article>
          <article className="panel price-card">
            <span className="panel-index">計測済み価格</span>
            <div className="price-rate">
              <strong>{proposal.identity.band_rate}</strong>
              <span>{proposal.identity.band_full}/{proposal.identity.band_denominator} full</span>
            </div>
            <dl>
              <div><dt>平均所要時間</dt><dd>{priceDuration} · n={proposal.price.duration_n}</dd></div>
              <div><dt>平均費用</dt><dd>{priceCost} · n={proposal.price.cost_n}</dd></div>
              <div><dt>計測</dt><dd>{proposal.identity.band_measurement}</dd></div>
            </dl>
            <label className="confirm-check">
              <input
                checked={confirmed}
                data-testid="gate-one-confirm"
                onChange={(event) => setConfirmed(event.target.checked)}
                type="checkbox"
              />
              この契約、モデル固定値、計測値、表示された書き込み境界を確認しました。
            </label>
            <code className="hash-line">{proposal.card_hash}</code>
            <button
              className="primary-action"
              data-testid="launch-session"
              disabled={!confirmed || busy || stage === "gate_2"}
              onClick={() => void launchConfirmed()}
              type="button"
            >
              確認して CLI に委譲
            </button>
          </article>
        </section>
      )}

      {(stage === "gate_2" || stage === "terminal") && created !== null && (
        <section className="panel execution-panel" data-testid="session-progress" ref={executionRef}>
          <header className="panel-heading">
            <div><span className="panel-index">GATE 2 / ファイルに基づく進行状況</span><h2>{created.id}</h2></div>
            <span className={`live-label ${monitor.status === "connected" ? "connected" : ""}`}>
              <i /> 実行: {session?.status ?? "starting"}
            </span>
          </header>
          <div
            className={`monitor-state ${monitor.status}`}
            data-monitor-status={monitor.status}
            data-testid="monitor-state"
          >
            <div>
              <strong>監視: {monitorLabel(monitor.status)}</strong>
              <span>
                最終更新成功: {formatLastSuccess(monitor.lastSuccessAt)}
              </span>
            </div>
            <small>
              {monitor.summary ?? "次のファイルベース状態更新を待っています。"}
              {monitor.retryInMs === null
                ? ""
                : ` ${monitor.attempt} 回目の再試行まで ${(monitor.retryInMs / 1000).toFixed(2)} 秒。`}
            </small>
            {monitor.guidance !== null && <p>{monitor.guidance}</p>}
          </div>
          <div className="execution-feedback" data-testid="execution-feedback">
            <div data-elapsed-seconds={elapsedSeconds} data-testid="elapsed-time">
              <span>経過時間</span>
              <strong>{formatElapsed(elapsedSeconds)}</strong>
            </div>
            <div data-testid="mean-duration-comparison">
              <span>平均所要時間（予測ではありません）</span>
              <strong>{priceDuration}</strong>
            </div>
            {currentPhase !== null && currentPhase.total > 0 && (
              <div data-testid="phase-progress">
                <span>実行進捗</span>
                <strong>フェーズ {currentPhase.index} / {currentPhase.total}</strong>
              </div>
            )}
          </div>
          <div className="phase-list">
            {session?.phases.length === 0 && <p>最初の CLI イベントを待っています…</p>}
            {session?.phases.map((phase) => (
              <div className={`phase-row ${phase.status}`} key={`${phase.index}-${phase.id}`}>
                <span>{String(phase.index).padStart(2, "0")}</span>
                <div><strong>{phase.id}</strong><small>{phase.stage}</small></div>
                <em>{phase.status}</em>
              </div>
            ))}
          </div>
          <footer>
            <div className="execution-receipt">
              <code>{session?.events_path ?? created.events_path}</code>
              <span>{session?.event_count ?? 0} イベント</span>
            </div>
            <div className="session-file-actions">
              <button
                data-testid="trial-events-footer"
                disabled={evidenceLoading}
                onClick={() => void readEvents()}
                type="button"
              >
                直近のイベント
              </button>
              <button
                disabled={evidenceLoading}
                onClick={() => void loadArtifacts()}
                type="button"
              >
                成果物を参照
              </button>
            </div>
          </footer>
        </section>
      )}

      {evidenceOpen && created !== null && (stage === "gate_2" || stage === "terminal") && (
        <section className="panel session-files-panel" data-testid="trial-session-files">
          <header className="panel-heading">
            <div>
              <span className="panel-index">読み取り専用セッションファイル</span>
              <h2>失敗の証跡</h2>
            </div>
            <span>{evidenceLoading ? "読み込み中…" : `${artifacts.length} 件`}</span>
          </header>
          {evidenceError !== null && <p className="trial-error" role="alert">{evidenceError}</p>}
          <div className="session-files-workbench">
            <aside className="session-file-list" aria-label="セッションファイル">
              <button
                className={evidenceDocument?.path === "events.jsonl" ? "active" : ""}
                data-testid="trial-events-open"
                disabled={evidenceLoading}
                onClick={() => void readEvents()}
                type="button"
              >
                <span>events.jsonl</span>
                <small>直近 200 行</small>
              </button>
              {artifacts.filter((artifact) => artifact.path !== "events.jsonl").map((artifact) => (
                <button
                  className={evidenceDocument?.path === artifact.path ? "active" : ""}
                  data-testid={artifact.path === "summary.md" ? "trial-summary-open" : undefined}
                  disabled={evidenceLoading}
                  key={artifact.path}
                  onClick={() => void readArtifact(artifact.path)}
                  type="button"
                >
                  <span>{artifact.id}</span>
                  <small>{byteLabel(artifact.size_bytes)}</small>
                </button>
              ))}
            </aside>
            <div className="session-file-document" data-testid="trial-file-viewer">
              <DocumentViewer
                document={evidenceDocument}
                empty="イベント、サマリー、または受入成果物を選択すると、ここに表示します。"
              />
            </div>
          </div>
        </section>
      )}

      {stage === "terminal" && session !== null && (
        <section className="terminal-grid" data-testid="terminal-gate" ref={terminalRef}>
          <article className="panel verdict-card">
            <span className="panel-index">{session.gate.toUpperCase()} / 終端</span>
            <h2>{session.verdict ?? session.status}</h2>
            <p>保証: <strong>{session.assurance ?? "未記録"}</strong></p>
            <pre>{session.acceptance_sheet ?? "終端証跡が不完全なため、受入シートは昇格されていません。"}</pre>
          </article>
          <aside className="panel next-action-card">
            <span className="panel-index">次の操作 / D-3d</span>
            <h2>境界指示</h2>
            <p>保存する文言は機密情報を除去してハッシュ化され、固定済みの契約下限は変更できません。</p>
            <textarea
              data-testid="directive-input"
              onChange={(event) => { setDirectiveText(event.target.value); setDirective(null); }}
              placeholder="終端後の指示を追加…"
              rows={4}
              value={directiveText}
            />
            <button className="secondary-action" disabled={busy || directive !== null || directiveText.trim() === ""} onClick={() => void persistDirective()} type="button">
              指示を除染して保存
            </button>
            {directive !== null && (
              <div className="directive-receipt" data-testid="directive-receipt">
                <strong>{directive.scrubbed_directive}</strong>
                <code>{directive.directive_hash}</code>
                <small>{directive.issued_gate} · ラウンド {directive.directive_round}</small>
                <button className="primary-action" disabled={busy} onClick={() => void confirmDirective()} type="button">
                  D-3d 継続を確認
                </button>
              </div>
            )}
            <button className="close-action" data-testid="close-session" onClick={() => setStage("closed")} type="button">追加実行せず終了</button>
          </aside>
        </section>
      )}

      {stage === "closed" && (
        <section className="panel closed-card" data-testid="closed-session">
          <span>セッション終了</span>
          <h2>追加の操作は実行されていません。</h2>
          <button
            className="primary-action"
            data-testid="start-new-run"
            onClick={startNewRun}
            type="button"
          >
            新しい実行を開始
          </button>
        </section>
      )}
    </Shell>
  );
}

function stageLabel(stage: ScreenStage, session: PolledSession | null): string {
  if (stage === "terminal") return session?.gate.toUpperCase() ?? "終端";
  if (stage === "gate_2") return "GATE 2";
  if (stage === "gate_1") return "確認待ち";
  if (stage === "closed") return "終了";
  return "下書き";
}

function formatElapsed(totalSeconds: number): string {
  const hours = Math.floor(totalSeconds / 3_600);
  const minutes = Math.floor((totalSeconds % 3_600) / 60);
  const seconds = totalSeconds % 60;
  return [hours, minutes, seconds].map((part) => String(part).padStart(2, "0")).join(":");
}

async function fetchSession(id: string, token: string): Promise<PolledSession> {
  const result = await fetchSessionPoll(id, token, null);
  if (result.value !== null) return result.value;
  throw {
    guidance:
      "監視が初回取得で304応答を受信しました。プロキシのキャッシュ設定を確認してから再接続してください。",
    summary: "初回のセッション状態がありません",
    terminal: true,
  } satisfies MonitorFailure;
}

async function fetchSessionPoll(
  id: string,
  token: string,
  etag: string | null,
): Promise<{ etag: string | null; value: PolledSession | null }> {
  let response: Response;
  try {
    const headers = authorizationHeaders(token);
    if (etag !== null) headers["if-none-match"] = etag;
    response = await fetch(apiPath(`sessions/${encodeURIComponent(id)}`), {
      headers,
      redirect: "manual",
    });
  } catch (reason) {
    throw thrownFailure(reason);
  }
  if (response.status === 304) {
    return { etag: response.headers.get("etag") ?? etag, value: null };
  }
  if (response.type === "opaqueredirect" || !response.ok) {
    throw await responseFailure(response);
  }
  try {
    return {
      etag: response.headers.get("etag"),
      value: (await response.json()) as PolledSession,
    };
  } catch (reason) {
    throw {
      guidance:
        "監視が不正な状態応答を受信しました。再接続する前に、プロキシ応答と既存セッションの成果物を確認してください。",
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
  if (value === null) return "未接続";
  return new Intl.DateTimeFormat("ja-JP", {
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
  return reason instanceof Error ? reason.message : "Trial リクエストに失敗しました。";
}

function monitorLabel(status: MonitorStatus): string {
  if (status === "connected") return "接続中";
  if (status === "degraded") return "不安定";
  return "切断";
}

function workspaceLeaseLabel(lease: TrialWorkspaceLease | null): string {
  if (lease === null) return "未確認";
  if (lease.status === "recovery_required") return "復旧が必要";
  if (lease.status === "running") return "実行中";
  return "待機中";
}

async function fetchWorkspaceLease(token: string): Promise<TrialWorkspaceLease> {
  const response = await fetch(apiPath("trial-workspace"), {
    headers: authorizationHeaders(token),
  });
  if (!response.ok) throw new Error(await apiError(response));
  return (await response.json()) as TrialWorkspaceLease;
}

function authorizationHeaders(token: string, json = false): Record<string, string> {
  return {
    "x-commandagent-trial-authorization": `Bearer ${token.trim()}`,
    ...(json ? { "content-type": "application/json" } : {}),
  };
}

function byteLabel(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  return `${(bytes / 1024).toFixed(1)} KiB`;
}
