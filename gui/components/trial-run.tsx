"use client";

import { DocumentViewer } from "./document-viewer";
import { GateCardMarkdown } from "./gate-card-markdown";
import { TrialSessionIndexPanel } from "./trial-session-index";
import { byteLabel, elapsedLabel, lastSuccessLabel } from "../lib/format";
import type { MonitorStatus } from "../lib/trial-monitor";
import type { PolledSession, TrialWorkspaceLease } from "../lib/types";
import { useTrialRun, type ScreenStage } from "../hooks/use-trial-run";

export function TrialRun() {
  const {
    artifacts,
    busy,
    checkContract,
    compatiblePacks,
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
  } = useTrialRun(terminalHeading);

  return (
      <section className="trial-layout">
        <aside
          aria-label="Trial の進行状況"
          className="trial-rail trial-stage-nav panel"
          data-testid="trial-stage-nav"
        >
          {[
            ["依頼", "Gate 1"],
            ["確認", "Gate 1"],
            ["実行", "Gate 2"],
            ["結果", "Gate 3 / 4"],
          ].map(([label, detail], index) => {
            const position = stagePosition(stage);
            return (
              <div
                aria-current={index === position ? "step" : undefined}
                className={`rail-step ${index <= position ? "reached" : ""} ${index === position ? "current" : ""}`}
                key={label}
              >
                <span>{index + 1}</span>
                <div><strong>{label}</strong><small>{detail}</small></div>
              </div>
            );
          })}
        </aside>

        <div
          className={`trial-stage trial-stage-${stage}`}
          data-stage={stage}
          data-testid="trial-active-stage"
        >
          {stage !== "compose" && error !== null && (
            <div className="trial-error trial-stage-error" role="alert">
              <p>{error}</p>
              {errorReconnectSessionId !== null && (
                <a
                  data-testid="reconnect-session-link"
                  href={`?session=${encodeURIComponent(errorReconnectSessionId)}`}
                  onClick={(event) => {
                    event.preventDefault();
                    void reconnectExisting(errorReconnectSessionId);
                  }}
                >
                  セッション {errorReconnectSessionId} に再接続
                </a>
              )}
            </div>
          )}

          {stage === "compose" && (
            <>
              <div className="trial-compose panel">
          <header className="panel-heading">
            <div>
              <span className="panel-index">GATE 1 / 実行前確認</span>
              <h2>実行内容を確認</h2>
            </div>
            <span className="gate-chip">{stageLabel(stage, session)}</span>
          </header>
          <div className="gate-one-primer" data-testid="gate-one-primer">
            <strong>Gate 1 は CLI 実行前の確認です</strong>
            <p>
              目標、正確なモデル ID、変更範囲、検証条件をカードで確認します。
              サンプルも自動実行されず、確認チェックを入れるまで CLI は起動しません。
            </p>
          </div>
          <label htmlFor="trial-goal">目標</label>
          <textarea
            data-testid="trial-goal"
            disabled={launchIdentityLocked}
            id="trial-goal"
            onChange={(event) => update("goal", event.target.value)}
            rows={5}
            value={spec.goal}
          />
          {trialTokenAuthEnabled ? (
            <>
              <label htmlFor="trial-token">Trial アクセストークン</label>
              <input
                autoComplete="off"
                autoCapitalize="none"
                data-testid="trial-token"
                disabled={launchIdentityLocked}
                id="trial-token"
                onChange={(event) => {
                  updateTrialToken(event.target.value);
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
            </>
          ) : (
            <p className="source-note" data-testid="trial-token-auth-disabled">
              Trial トークン認証はサーバー設定で無効です。
            </p>
          )}
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
                disabled={busy || reconnectSessionId.trim() === "" || !trialAccessReady}
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
                  {selectedProfile.status === "draft" && (
                    <> manifest: {selectedProfile.manifest_hash} · 未承認 · 保証上限 static</>
                  )}
                </small>
              )}
            </label>
            <label>
              検証 pack
              <select
                data-testid="trial-pack"
                disabled={launchIdentityLocked || trialOptions === null || selectedProfile?.status === "draft"}
                value={spec.pack ?? ""}
                onChange={(event) => update("pack", event.target.value || null)}
              >
                <option value="">選択なし</option>
                {compatiblePacks.map((option) => {
                  const selector = `${option.id}@${option.version}`;
                  return (
                    <option key={selector} value={selector}>
                      {selector} · {option.source_label}
                    </option>
                  );
                })}
              </select>
              {selectedPack !== undefined && (
                <small className="trial-field-hint" data-testid="trial-pack-source">
                  {selectedPack.profile} × {selectedPack.intent} · 供給元: {selectedPack.source_label}
                </small>
              )}
              {selectedProfile?.status === "draft" && (
                <small className="trial-field-hint" data-testid="trial-draft-pack-note">
                  draft profile では検証 pack は「選択なし」固定です。
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
          <div className="trial-action-bar trial-request-actions">
            <button
              className="secondary-action"
              data-testid="check-contract"
              disabled={busy || launchIdentityLocked}
              onClick={() => void checkContract()}
              type="button"
            >
              契約と見積りを確認
            </button>
          </div>
          {optionsError !== null && <p className="trial-error" role="alert">{optionsError}</p>}
          {error !== null && (
            <div className="trial-error" role="alert">
              <p>{error}</p>
              {errorReconnectSessionId !== null && (
                <a
                  data-testid="reconnect-session-link"
                  href={`?session=${encodeURIComponent(errorReconnectSessionId)}`}
                  onClick={(event) => {
                    event.preventDefault();
                    void reconnectExisting(errorReconnectSessionId);
                  }}
                >
                  セッション {errorReconnectSessionId} に再接続
                </a>
              )}
            </div>
          )}
              </div>
            </>
          )}

      {proposal !== null && stage === "gate_1" && (
        <section className="gate-one-grid" data-testid="gate-one-card" ref={gateOneRef}>
          <article className="panel contract-card">
            <GateCardMarkdown markdown={proposal.card_markdown} />
          </article>
          <article className="panel price-card">
            <span className="panel-index">時間と費用の目安</span>
            <h2>過去の実行記録から確認</h2>
            <dl>
              <div><dt>所要時間</dt><dd>{priceDuration} ({proposal.price.duration_n} 件)</dd></div>
              <div><dt>費用</dt><dd>{priceCost} ({proposal.price.cost_n} 件)</dd></div>
            </dl>
            <div className="workspace-boundary" data-testid="trial-workspace">
              <strong>ファイルを変更できる範囲</strong>
              <code>{proposal.identity.workspace}</code>
              <p>実行する CLI は、このディレクトリ内の内容だけを作成・変更・削除できます。</p>
            </div>
            <div className="confirmation-id">
              <strong>確認 ID</strong>
              <code className="hash-line">{proposal.card_hash}</code>
              <p>確認内容が1つでも変わると、この ID も変わります。</p>
            </div>
            <div className="gate-one-actions trial-action-bar">
              <label className="confirm-check">
                <input
                  checked={confirmed}
                  data-testid="gate-one-confirm"
                  onChange={(event) => setConfirmed(event.target.checked)}
                  type="checkbox"
                />
                必須チェック、使用モデル、過去の実行結果、表示されたファイル変更範囲を確認しました。
              </label>
              <button
                className="primary-action"
                data-testid="launch-session"
                disabled={!confirmed || busy || launchBlockReason !== null}
                onClick={() => void launchConfirmed()}
                type="button"
              >
                確認して CLI を実行
              </button>
              {launchBlockReason !== null && (
                <p className="launch-block-reason" data-testid="launch-block-reason">
                  {launchBlockReason}
                </p>
              )}
            </div>
          </article>
        </section>
      )}

      {stage === "gate_2" && created !== null && (
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
                最終更新成功: {lastSuccessLabel(monitor.lastSuccessAt)}
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
              <strong>{elapsedLabel(elapsedSeconds)}</strong>
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
            <span className="panel-index">実行結果 / {session.gate === "gate_3" ? "Gate 3" : "Gate 4"}</span>
            <h2 data-testid="terminal-result-heading">{terminalHeading(session)}</h2>
            <p className="terminal-gate-explanation">{gateExplanation(session.gate)}</p>
            <dl className="terminal-result-summary" data-testid="terminal-result-summary">
              <div><dt>結果</dt><dd data-testid="terminal-verdict-summary">{verdictSummary(session)}</dd></div>
              <div><dt>保証水準</dt><dd data-testid="terminal-assurance-summary">{assuranceSummary(session.assurance)}</dd></div>
              <div><dt>状態</dt><dd data-testid="terminal-status-summary">{statusSummary(session.status)}</dd></div>
            </dl>
            <a
              className="terminal-history-link"
              data-testid="terminal-session-history-link"
              href={`#trial-session-${session.id}`}
            >
              このセッションを GUI Trial 実行履歴で確認
            </a>
            <pre>{session.acceptance_sheet ?? "実行結果の証跡が不足しているため、受入シートは生成されていません。"}</pre>
          </article>
          <aside className="panel next-action-card">
            <span className="panel-index">任意の次の操作</span>
            <h2>追加の依頼を入力</h2>
            <p>保存前に認証情報を除去し、実行前に内容をもう一度確認します。確定済みの必須チェックは変更できません。</p>
            <textarea
              data-testid="directive-input"
              onChange={(event) => { setDirectiveText(event.target.value); setDirective(null); }}
              placeholder="実行結果を踏まえた追加の依頼を入力…"
              rows={4}
              value={directiveText}
            />
            <button className="secondary-action" disabled={busy || directive !== null || directiveText.trim() === ""} onClick={() => void persistDirective()} type="button">
              追加の依頼を確認用に準備
            </button>
            {directive !== null && (
              <div className="directive-receipt" data-testid="directive-receipt">
                <strong>{directive.scrubbed_directive}</strong>
                <code>{directive.directive_hash}</code>
                <small>{directive.issued_gate} · 追加依頼 {directive.directive_round}</small>
                <button className="primary-action" disabled={busy} onClick={() => void confirmDirective()} type="button">
                  確認して追加の依頼を実行
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
          <TrialSessionIndexPanel
            accessToken={trialToken}
            observedSession={observedSession}
            onAccessTokenRejected={rejectTrialToken}
            onLeaseChange={setWorkspaceLease}
            revalidationKey={sessionIndexRevision}
          />
        </div>
      </section>
  );
}

function terminalHeading(session: PolledSession): string {
  return session.gate === "gate_3"
    ? "すべての必須チェックに合格しました"
    : "すべての必須チェックには合格していません";
}

function gateExplanation(gate: string): string {
  return gate === "gate_3"
    ? "Gate 3 は、固定された必須チェックをすべて満たした実行結果です。"
    : "Gate 4 は、未達または不十分な必須チェックがあり、証跡と次の操作を確認する結果です。";
}

function verdictSummary(session: PolledSession): string {
  if (session.verdict === null) return "最終受け入れは記録されていません。";
  if (["static", "partial", "failed", "none", "reduced"].includes(session.verdict)) {
    return "最終受け入れは記録されていません。";
  }
  return session.gate === "gate_3"
    ? "最終受け入れは合格として記録されています。"
    : "最終受け入れは不合格として記録されています。";
}

function assuranceSummary(assurance: string | null): string {
  switch (assurance) {
    case "full":
      return "必要な実行証跡がすべて記録されています。";
    case "partial":
      return "必要な実行証跡の一部だけが記録されています。";
    case "static":
      return "実行検証は完了しておらず、静的な証跡だけが記録されています。";
    case "failed":
      return "記録された証跡に、必須チェックの不合格があります。";
    case null:
      return "保証水準は記録されていません。";
    default:
      return "詳しい保証水準は下の受入シートで確認してください。";
  }
}

function statusSummary(status: string): string {
  switch (status) {
    case "completed": return "実行は完了しました。";
    case "failed": return "実行は失敗として終了しました。";
    case "interrupted": return "実行は中断されました。";
    case "starting": return "実行を開始しています。";
    case "running": return "実行中です。";
    default: return "詳しい状態は下の受入シートで確認してください。";
  }
}

function stagePosition(stage: ScreenStage): number {
  if (stage === "compose") return 0;
  if (stage === "gate_1") return 1;
  if (stage === "gate_2") return 2;
  return 3;
}

function stageLabel(stage: ScreenStage, session: PolledSession | null): string {
  if (stage === "terminal") return session?.gate.toUpperCase() ?? "終端";
  if (stage === "gate_2") return "GATE 2";
  if (stage === "gate_1") return "確認待ち";
  if (stage === "closed") return "終了";
  return "下書き";
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
