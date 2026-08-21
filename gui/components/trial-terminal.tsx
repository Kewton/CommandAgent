import type { TrialRunState } from "../hooks/use-trial-run";
import { byteLabel } from "../lib/format";
import type { PolledSession } from "../lib/types";
import { DocumentViewer } from "./document-viewer";
import { TrialRunIdentity } from "./trial-run-identity";

type TrialTerminalProps = {
  onHighlightSession: (id: string) => void;
  run: TrialRunState;
};

export function TrialTerminal({ onHighlightSession, run }: TrialTerminalProps) {
  const {
    artifacts, busy, confirmDirective, created, directive, directiveText,
    evidenceDocument, evidenceError, evidenceLoading, evidenceOpen, persistDirective,
    readArtifact, readEvents, session, setDirective, setDirectiveText, setStage,
    stage, startNewRun, terminalRef,
  } = run;

  return (
    <>
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
            <span className="panel-index">
              実行結果 / {session.gate === "gate_3" ? "Gate 3" : "Gate 4"}
            </span>
            <h2 data-testid="terminal-result-heading">{terminalHeading(session)}</h2>
            <p className="terminal-gate-explanation">{gateExplanation(session.gate)}</p>
            <TrialRunIdentity identity={session.identity} />
            <dl className="terminal-result-summary" data-testid="terminal-result-summary">
              <div>
                <dt>結果</dt>
                <dd data-testid="terminal-verdict-summary">{verdictSummary(session)}</dd>
              </div>
              <div>
                <dt>保証水準</dt>
                <dd data-testid="terminal-assurance-summary">{assuranceSummary(session.assurance)}</dd>
              </div>
              <div>
                <dt>状態</dt>
                <dd data-testid="terminal-status-summary">{statusSummary(session.status)}</dd>
              </div>
            </dl>
            <a
              className="terminal-history-link"
              data-testid="terminal-session-history-link"
              href={`#trial-session-${session.id}`}
              onClick={() => onHighlightSession(session.id)}
            >
              このセッションを GUI Trial 実行履歴で確認
            </a>
            <pre>
              {session.acceptance_sheet ?? "実行結果の証跡が不足しているため、受入シートは生成されていません。"}
            </pre>
          </article>
          <aside className="panel next-action-card">
            <span className="panel-index">任意の次の操作</span>
            <h2>追加の依頼を入力</h2>
            <p>保存前に認証情報を除去し、実行前に内容をもう一度確認します。確定済みの必須チェックは変更できません。</p>
            <textarea
              data-testid="directive-input"
              onChange={(event) => {
                setDirectiveText(event.target.value);
                setDirective(null);
              }}
              placeholder="実行結果を踏まえた追加の依頼を入力…"
              rows={4}
              value={directiveText}
            />
            <button
              className="secondary-action"
              disabled={busy || directive !== null || directiveText.trim() === ""}
              onClick={() => void persistDirective()}
              type="button"
            >
              追加の依頼を確認用に準備
            </button>
            {directive !== null && (
              <div className="directive-receipt" data-testid="directive-receipt">
                <strong>{directive.scrubbed_directive}</strong>
                <code>{directive.directive_hash}</code>
                <small>{directive.issued_gate} · 追加依頼 {directive.directive_round}</small>
                <button
                  className="primary-action"
                  disabled={busy}
                  onClick={() => void confirmDirective()}
                  type="button"
                >
                  確認して追加の依頼を実行
                </button>
              </div>
            )}
            <button
              className="close-action"
              data-testid="close-session"
              onClick={() => setStage("closed")}
              type="button"
            >
              追加実行せず終了
            </button>
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
    </>
  );
}

export function terminalHeading(session: PolledSession): string {
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
    case "full": return "必要な実行証跡がすべて記録されています。";
    case "partial": return "必要な実行証跡の一部だけが記録されています。";
    case "static": return "実行検証は完了しておらず、静的な証跡だけが記録されています。";
    case "failed": return "記録された証跡に、必須チェックの不合格があります。";
    case null: return "保証水準は記録されていません。";
    default: return "詳しい保証水準は下の受入シートで確認してください。";
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
