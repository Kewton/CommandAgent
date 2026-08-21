import type { TrialRunState } from "../hooks/use-trial-run";
import { GateCardMarkdown } from "./gate-card-markdown";

export function TrialGateOne({ run }: { run: TrialRunState }) {
  const {
    busy, confirmed, gateOneRef, launchBlockReason, launchConfirmed, priceCost,
    priceDuration, proposal, setConfirmed, stage,
  } = run;
  if (proposal === null || stage !== "gate_1") return null;

  return (
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
  );
}
