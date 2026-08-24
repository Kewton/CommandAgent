"use client";

import { useState } from "react";

import { useTrialRun, type ScreenStage } from "../hooks/use-trial-run";
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

export function TrialRun() {
  const [highlightedSessionId, setHighlightedSessionId] = useState<string | null>(null);
  const run = useTrialRun(terminalHeading);
  const {
    error, errorReconnectSessionId, observedSession, reconnectExisting,
    reconnectSessionId, rejectTrialToken, session, sessionIndexRevision, setWorkspaceLease,
    stage, trialToken,
  } = run;

  return (
    <section className="trial-layout">
      <aside
        aria-label="Trial の進行状況"
        className="trial-rail trial-stage-nav panel"
        data-testid="trial-stage-nav"
      >
        <p aria-atomic="true" aria-live="polite" className="trial-stage-announcement">
          現在の段階: {trialStages[stagePosition(stage)][0]}
        </p>
        <ol className="trial-stage-list">
          {trialStages.map(([label, detail], index) => {
            const position = stagePosition(stage);
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
        className={`trial-stage trial-stage-${stage}`}
        data-stage={stage}
        data-testid="trial-active-stage"
      >
        {stage !== "compose" && error !== null && (
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

        {stage === "compose" && <TrialCompose run={run} />}
        <TrialGateOne run={run} />
        <TrialGateTwo run={run} />
        <TrialTerminal onHighlightSession={setHighlightedSessionId} run={run} />
        <TrialSessionIndexPanel
          accessToken={trialToken}
          deferAutomaticRevalidation={stage === "compose" && reconnectSessionId.trim() !== ""}
          highlight={highlightedSessionId}
          observedSession={observedSession}
          onAccessTokenRejected={rejectTrialToken}
          onLeaseChange={setWorkspaceLease}
          revalidationKey={sessionIndexRevision}
        />
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
