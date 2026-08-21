import type { TrialRunState } from "../hooks/use-trial-run";
import { dateTimeLabel, elapsedLabel } from "../lib/format";
import type { MonitorStatus } from "../lib/trial-monitor";
import { TrialRunIdentity } from "./trial-run-identity";

export function TrialGateTwo({ run }: { run: TrialRunState }) {
  const {
    created, currentPhase, elapsedSeconds, evidenceLoading, executionRef,
    loadArtifacts, monitor, priceDuration, proposal, readEvents, session, stage,
  } = run;
  if (stage !== "gate_2" || created === null) return null;
  const runIdentity = session?.identity ?? proposal?.identity ?? null;

  return (
    <section className="panel execution-panel" data-testid="session-progress" ref={executionRef}>
      <header className="panel-heading">
        <div>
          <span className="panel-index">GATE 2 / ファイルに基づく進行状況</span>
          <h2>{created.id}</h2>
        </div>
        <span className={`live-label ${monitor.status === "connected" ? "connected" : ""}`}>
          <i /> 実行: {session?.status ?? "starting"}
        </span>
      </header>
      {runIdentity !== null && <TrialRunIdentity identity={runIdentity} />}
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
  );
}

function monitorLabel(status: MonitorStatus): string {
  if (status === "connected") return "接続中";
  if (status === "degraded") return "不安定";
  return "切断";
}
