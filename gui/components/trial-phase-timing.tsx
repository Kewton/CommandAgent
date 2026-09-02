import type { ReactNode } from "react";

import { dateTimeLabel, elapsedLabel } from "../lib/format";
import type { PhaseStatus } from "../lib/types";

type TrialPhaseTimingProps = {
  phases: PhaseStatus[];
  totalProcessingDurationMs: number | null;
};

export function TrialPhaseTiming({ phases, totalProcessingDurationMs }: TrialPhaseTimingProps) {
  const hasRecordedBoundaries = phases.some(
    (phase) => phase.started_at_epoch_ms != null || phase.ended_at_epoch_ms != null,
  );
  return (
    <section className="trial-phase-timing" data-testid="trial-phase-timing">
      <header>
        <div>
          <span>処理時間</span>
          <h3>フェーズ別タイムライン</h3>
        </div>
        <p data-testid="trial-total-processing-duration">
          <span>トータル処理時間</span>
          <strong>{durationLabel(totalProcessingDurationMs)}</strong>
        </p>
      </header>
      {phases.length === 0 ? (
        <p className="source-note">フェーズ情報は記録されていません。</p>
      ) : (
        <div className="trial-phase-timing-table">
          <table>
            <thead>
              <tr>
                <th scope="col">フェーズ</th>
                <th scope="col">開始時刻</th>
                <th scope="col">終了時刻</th>
                <th scope="col">所要時間</th>
              </tr>
            </thead>
            <tbody>
              {phases.map((phase) => (
                <tr key={`${phase.index}-${phase.id}`}>
                  <th scope="row">{phaseName(phase)}</th>
                  <td>{timeLabel(phase.started_at_epoch_ms ?? null)}</td>
                  <td>{timeLabel(phase.ended_at_epoch_ms ?? null)}</td>
                  <td>{durationLabel(phase.duration_ms ?? null)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {!hasRecordedBoundaries && phases.length > 0 && (
        <p className="source-note" data-testid="trial-phase-timing-legacy-note">
          このセッションにはフェーズ境界時刻が記録されていないため、開始・終了時刻は未記録です。
        </p>
      )}
    </section>
  );
}

function phaseName(phase: PhaseStatus): string {
  if (phase.id === "plan_generation") return "計画の生成";
  return phase.index > 0 ? `${phase.index}. ${phase.id}` : phase.id;
}

function timeLabel(value: number | null): ReactNode {
  if (value === null || value <= 0) return "未記録";
  const date = new Date(value);
  return <time dateTime={date.toISOString()}>{dateTimeLabel(date, "未記録")}</time>;
}

function durationLabel(value: number | null): string {
  if (value === null || value < 0) return "未記録";
  return elapsedLabel(Math.round(value / 1_000));
}
