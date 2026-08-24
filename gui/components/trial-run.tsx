"use client";

import { useTrialRun, type ScreenStage } from "../hooks/use-trial-run";
import { useTrialPageRouting } from "../hooks/use-trial-page-routing";
import type { TrialRoute } from "../lib/base-path";
import { TrialAccessPanel } from "./trial-access-panel";
import { TrialCompose } from "./trial-compose";
import { TrialGateOne } from "./trial-gate-one";
import { TrialGateTwo } from "./trial-gate-two";
import { TrialSessionIndexPanel } from "./trial-session-index";
import { TrialTerminal, terminalHeading } from "./trial-terminal";

const trialStages = [
  ["依頼", "Gate 1"],
  ["確認", "Gate 1"],
  ["実行", "Gate 2"],
  ["結果", "Gate 3 / 4"],
] as const;

export function TrialRun({ surface }: { surface: TrialRoute }) {
  const run = useTrialRun(terminalHeading, { loadComposeOptions: surface === "compose" });
  const {
    created, error, errorReconnectSessionId, reconnectExisting, rejectTrialToken,
    session, setWorkspaceLease, stage, trialToken,
  } = run;
  const sessionId = session?.id ?? created?.id ?? null;
  useTrialPageRouting(surface, stage, sessionId);
  const displayedStage = surface === "status" && stage === "compose"
    ? "gate_2"
    : surface === "detail" && stage === "compose"
      ? "terminal"
      : stage;

  if (surface === "history") {
    return (
      <div className="trial-history-surface" data-testid="trial-history-surface">
        <TrialAccessPanel purpose="history" run={run} />
        <TrialSessionIndexPanel
          accessToken={trialToken}
          onAccessTokenRejected={rejectTrialToken}
          onLeaseChange={setWorkspaceLease}
        />
      </div>
    );
  }

  return (
    <section className="trial-layout">
      <aside
        aria-label="Trial の進行状況"
        className="trial-rail trial-stage-nav panel"
        data-testid="trial-stage-nav"
      >
        <p aria-atomic="true" aria-live="polite" className="trial-stage-announcement">
          現在の段階: {trialStages[stagePosition(displayedStage)][0]}
        </p>
        <ol className="trial-stage-list">
          {trialStages.map(([label, detail], index) => {
            const position = stagePosition(displayedStage);
            return (
              <li
                aria-current={index === position ? "step" : undefined}
                className={`rail-step ${index <= position ? "reached" : ""} ${index === position ? "current" : ""}`}
                key={label}
              >
                <span>{index + 1}</span>
                <div><strong>{label}</strong><small>{detail}</small></div>
              </li>
            );
          })}
        </ol>
      </aside>

      <div
        className={`trial-stage trial-stage-${displayedStage}`}
        data-stage={displayedStage}
        data-testid="trial-active-stage"
      >
        {surface !== "compose" && <TrialAccessPanel purpose={surface} run={run} />}
        {surface === "compose" && stage !== "compose" && error !== null && (
          <div className="trial-error trial-stage-error" role="alert">
            <p>{error}</p>
            {errorReconnectSessionId !== null && (
              <button
                className="inline-action"
                data-testid="reconnect-session-link"
                onClick={() => void reconnectExisting(errorReconnectSessionId)}
                type="button"
              >
                セッション {errorReconnectSessionId} に再接続
              </button>
            )}
          </div>
        )}

        {surface === "compose" && stage === "compose" && <TrialCompose run={run} />}
        {surface === "compose" && <TrialGateOne run={run} />}
        {surface === "status" && <TrialGateTwo run={run} />}
        {(surface === "status" || surface === "detail") && <TrialTerminal run={run} />}
      </div>
    </section>
  );
}

function stagePosition(stage: ScreenStage): number {
  if (stage === "compose") return 0;
  if (stage === "gate_1") return 1;
  if (stage === "gate_2") return 2;
  return 3;
}
