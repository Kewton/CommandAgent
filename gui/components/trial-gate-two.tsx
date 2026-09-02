import type { TrialRunState } from "../hooks/use-trial-run";
import {
  dateTimeLabel,
  elapsedLabel,
  phaseStageLabel,
  phaseStatusLabel,
  trialStatusLabel,
} from "../lib/format";
import type { MonitorStatus } from "../lib/trial-monitor";
import { TrialRunIdentity } from "./trial-run-identity";
import { TrialTaskProgress } from "./trial-task-progress";

export function TrialGateTwo({ run }: { run: TrialRunState }) {
  const {
    busy, cancelStop, confirmStop, created, currentPhase, elapsedSeconds, evidenceLoading, executionRef,
    loadArtifacts, monitor, priceDuration, proposal, readEvents, session, stage,
    stopActiveSession, stopError, stopState,
  } = run;
  if (stage !== "gate_2" || created === null) return null;
  const runIdentity = session?.identity ?? proposal?.identity ?? null;

  return (
    <section
      className="panel execution-panel"
      data-testid="session-progress"
      ref={executionRef}
      tabIndex={-1}
    >
      <header className="panel-heading">
        <div>
          <span className="panel-index">Gate 2 / ファイルに基づく進行状況</span>
          <h2>{created.id}</h2>
        </div>
        <span className={`live-label ${monitor.status === "connected" ? "connected" : ""}`}>
          <i /> 実行: {trialStatusLabel(session?.status ?? "starting")}
        </span>
      </header>
      {runIdentity !== null && (
        <TrialRunIdentity identity={runIdentity} recovery={session?.recovery_auto_run} />
      )}
      <p
        aria-atomic="true"
        aria-live={monitor.status === "lost" ? "assertive" : "polite"}
        className="trial-monitor-announcement"
        data-testid="monitor-announcement"
        role="status"
      >
        トライアル監視: {monitorLabel(monitor.status)}
      </p>
      <div
        className={`monitor-state ${monitor.status}`}
        data-monitor-status={monitor.status}
        data-testid="monitor-state"
      >
        <div>
          <strong>監視: {monitorLabel(monitor.status)}</strong>
          <span>最終更新成功: {dateTimeLabel(monitor.lastSuccessAt ?? "", "未接続")}</span>
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
      {created.process_generation !== null && (
        <div className="trial-stop-control" data-stop-state={stopState} data-testid="trial-stop-control">
          {stopState === "idle" && (
            <button
              className="danger-action"
              data-testid="trial-stop-open"
              disabled={busy}
              onClick={confirmStop}
              type="button"
            >
              実行を停止
            </button>
          )}
          {stopState === "confirming" && (
            <div
              aria-labelledby="trial-stop-confirm-heading"
              aria-modal="false"
              className="trial-stop-confirmation"
              data-testid="trial-stop-confirmation"
              role="alertdialog"
            >
              <strong id="trial-stop-confirm-heading">この実行を中断しますか？</strong>
              <p>対象セッションの現在のプロセスだけに割り込みを送り、結果は Gate 4 として記録します。</p>
              <div>
                <button autoFocus onClick={cancelStop} type="button">戻る</button>
                <button
                  className="danger-action"
                  data-testid="trial-stop-confirm"
                  onClick={() => void stopActiveSession()}
                  type="button"
                >
                  停止する
                </button>
              </div>
            </div>
          )}
          {stopState === "stopping" && (
            <p aria-live="assertive" data-testid="trial-stop-pending" role="status">
              停止処理中です。プロセスの終了と監査記録を確認しています。
            </p>
          )}
          {stopState === "failed" && (
            <div className="trial-stop-error" data-testid="trial-stop-error" role="alert">
              <strong>停止要求を完了できませんでした。</strong>
              <p>{stopError}</p>
              <button onClick={confirmStop} type="button">停止を再試行</button>
              <button onClick={cancelStop} type="button">閉じる</button>
            </div>
          )}
        </div>
      )}
      <div className="phase-list">
        {session === null && <p>実行状態を取得しています。イベント件数はまだ未取得です。</p>}
        {session !== null && session.phases.length === 0 && (
          <p>計画を生成中です（イベント {session.event_count} 件、フェーズはまだありません）。</p>
        )}
        {session?.phases.map((phase) => (
          <div className={`phase-row ${phase.status}`} key={`${phase.index}-${phase.id}`}>
            <span>{String(phase.index).padStart(2, "0")}</span>
            <div>
              <strong>{phaseName(phase.id)}</strong>
              <small>{phaseStageLabel(phase.stage)}</small>
            </div>
            <em>{phaseStatusLabel(phase.status)}</em>
          </div>
        ))}
      </div>
      {session !== null && (
        <TrialTaskProgress
          evidenceLoading={evidenceLoading}
          onOpenEvents={readEvents}
          progress={session.task_progress}
          terminal={false}
        />
      )}
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
  );
}

function monitorLabel(status: MonitorStatus): string {
  if (status === "connected") return "接続中";
  if (status === "degraded") return "不安定";
  return "切断";
}

function phaseName(id: string): string {
  return id === "plan_generation" ? "計画の生成" : id;
}
