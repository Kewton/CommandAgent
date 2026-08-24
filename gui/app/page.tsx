"use client";

import { GettingStarted } from "../components/getting-started";
import { Shell, useShellRuntimeStatus } from "../components/shell";
import { routePath, trialRoutePath, withBasePath } from "../lib/base-path";

const principles = [
  {
    label: "LOCAL-FIRST",
    title: "手元の環境を起点にする",
    body: "コードと実行記録は信頼したローカル環境に置きます。利用するモデル接続先は、実行前に選んだ provider の設定どおりです。",
  },
  {
    label: "EXPLICIT CONFIRMATION",
    title: "実行前に人が確認する",
    body: "Gate 1 は、CLI を起動する前に目標、変更範囲、モデル、必須チェックを固定して確認する段階です。",
  },
  {
    label: "BOUNDED WRITES",
    title: "書き込み先を限定する",
    body: "トライアルが変更できるのは、設定された専用の実行ルートだけです。リポジトリや非公開の拡張ルートとは分離します。",
  },
  {
    label: "HONEST FAILURE",
    title: "失敗を成功に見せない",
    body: "検証に通らなければ、修復を試して証拠を残し、それでも満たせない結果は要対応または失敗として表示します。",
  },
] as const;

const workflow = [
  ["01", "Goal", "作りたいもの、直したい不具合、調べたいことを普段の言葉で入力します。"],
  ["02", "実行前確認", "Gate 1 で目標、書き込み境界、profile、pack、モデル、必須チェックを確認します。"],
  ["03", "計画と実装", "確認した範囲の中で計画を立て、コードを変更します。"],
  ["04", "検証と修復", "必須チェックを実行し、失敗は証拠を保ったまま有界な修復へ戻します。"],
  ["05", "Result", "全条件を満たした結果だけを検証済みとして示し、満たせない場合は正直な失敗と次の行動を返します。"],
] as const;

const extensionLayers = [
  {
    layer: "Layer 1",
    title: "能力語彙",
    body: "実行できる source と check の閉じた一覧です。追加には Rust 実装、schema、golden、corpus とコードレビューが必要です。",
    boundary: "コードとレビューが必要",
  },
  {
    layer: "Layer 2",
    title: "下書きプロファイル",
    body: "既存の能力をタスク向けに組み合わせる manifest です。拡張ルートへ登録できますが、外部 profile は draft のままです。",
    boundary: "ローカル登録可能 / assurance は static まで",
  },
  {
    layer: "Layer 3",
    title: "パック供給",
    body: "補助知識と評価材料を version と exact hash で供給します。検証して pin したローカル pack を Trial に渡せます。",
    boundary: "GUI で作成・検証・pin 可能 / 未承認",
  },
  {
    layer: "Layer 4",
    title: "Admission",
    body: "測定 evidence と maintainer review に基づく昇格境界です。GUI やローカル実行から自己昇格はできません。",
    boundary: "計測・レビュー・昇格が必要",
  },
] as const;

export default function OverviewPage() {
  return (
    <Shell
      active="dashboard"
      title="概要"
      description="CommandAgent の目的、安全設計、最初のトライアル、拡張境界を案内します。"
    >
      <OverviewLanding />
    </Shell>
  );
}

function OverviewLanding() {
  const runtime = useShellRuntimeStatus();
  const runtimeData = runtime?.data ?? null;
  const runtimeUnavailable = runtime?.failed === true;
  const activeSession = runtimeUnavailable ? null : runtimeData?.session ?? null;
  const readinessStatus = runtimeUnavailable
    ? "unavailable"
    : runtimeData === null
      ? "loading"
      : runtimeData.trial_available
        ? "ready"
        : "action_required";
  const sessionStatus = runtimeUnavailable
    ? "unavailable"
    : runtimeData === null
      ? "loading"
      : activeSession?.state ?? "idle";
  const extensionRoot = runtimeData?.prerequisites.extension_root ?? null;
  const extensionStatus = runtimeUnavailable
    ? "unavailable"
    : extensionRoot?.status ?? "loading";

  return (
    <div className="overview-landing">
      <section
        aria-labelledby="overview-hero-heading"
        className="overview-hero"
        data-testid="overview-hero"
      >
        <div className="overview-hero-copy">
          <span>LOCAL-FIRST CODING AGENT</span>
          <h2 id="overview-hero-heading">目標を、検証可能なコードに。</h2>
          <p>
            CommandAgent は、実行前に範囲を確認し、実装を必須チェックで検証して、
            成功も失敗も根拠とともに返すコーディングエージェントです。
          </p>
          <div className="overview-hero-actions">
            <a
              className="primary-action"
              data-testid="overview-trial-cta"
              href={withBasePath(trialRoutePath("compose"))}
            >
              トライアルを始める
            </a>
            {activeSession !== null && (
              <a
                className="secondary-action"
                data-testid="overview-active-session-cta"
                href={withBasePath(trialRoutePath("status", activeSession.id))}
              >
                {activeSession.state === "recovery_required"
                  ? "要復旧のセッションを見る"
                  : "実行中セッションを見る"}
              </a>
            )}
          </div>
        </div>
        <ul aria-label="CommandAgent の約束" className="overview-hero-facts">
          <li><strong>確認してから実行</strong><span>範囲と条件を先に固定</span></li>
          <li><strong>検証してから完了</strong><span>申告だけを成功にしない</span></li>
          <li><strong>証拠を残して判断</strong><span>修復不能な失敗も隠さない</span></li>
        </ul>
      </section>

      <section aria-labelledby="overview-principles-heading" className="overview-section">
        <header className="overview-section-heading">
          <span>WHY COMMANDAGENT</span>
          <h2 id="overview-principles-heading">安全と検証を先に設計する</h2>
          <p>自律的に変更するからこそ、人の確認、書き込み境界、検証結果を実行の中心に置きます。</p>
        </header>
        <div className="overview-principle-grid">
          {principles.map((principle) => (
            <article key={principle.label}>
              <span>{principle.label}</span>
              <h3>{principle.title}</h3>
              <p>{principle.body}</p>
            </article>
          ))}
        </div>
      </section>

      <section aria-labelledby="overview-terms-heading" className="overview-terms panel">
        <header>
          <span>PLAIN LANGUAGE</span>
          <h2 id="overview-terms-heading">画面で使う 4 つの言葉</h2>
        </header>
        <dl>
          <div><dt>Gate</dt><dd>次へ進む前に、人の確認や機械の判定を必要とする境目です。</dd></div>
          <div><dt>profile</dt><dd>タスクの種類に合う進め方と、最低限必要なチェックの組です。</dd></div>
          <div><dt>pack</dt><dd>profile に加える補助知識と評価材料。version と hash で実行に固定します。</dd></div>
          <div><dt>assurance</dt><dd>実際に通過した検証と証拠から得る保証水準。設定だけでは上がりません。</dd></div>
        </dl>
      </section>

      <section aria-labelledby="overview-flow-heading" className="overview-section">
        <header className="overview-section-heading">
          <span>HOW IT WORKS</span>
          <h2 id="overview-flow-heading">Goal から検証済みの結果まで</h2>
          <p>途中の失敗は修復へ戻し、最後まで満たせない条件はそのまま結果へ残します。</p>
        </header>
        <ol className="overview-flow" data-testid="overview-flow">
          {workflow.map(([index, title, body]) => (
            <li key={index}>
              <span>{index}</span>
              <h3>{title}</h3>
              <p>{body}</p>
            </li>
          ))}
        </ol>
      </section>

      <GettingStarted />

      <section aria-labelledby="overview-extensions-heading" className="overview-section">
        <header className="overview-section-heading overview-section-heading-with-action">
          <div>
            <span>EXTEND WITHOUT BYPASSING TRUST</span>
            <h2 id="overview-extensions-heading">4 つのレイヤーで安全に拡張する</h2>
            <p>登録できるものと、コードレビューや計測による昇格が必要なものを分離します。</p>
          </div>
          <a data-testid="assets-link" href={withBasePath(routePath("assets"))}>拡張の詳細を見る ↗</a>
        </header>
        <ol className="overview-extension-grid" data-testid="overview-extension-layers">
          {extensionLayers.map((layer) => (
            <li key={layer.layer}>
              <span>{layer.layer}</span>
              <h3>{layer.title}</h3>
              <p>{layer.body}</p>
              <strong>{layer.boundary}</strong>
            </li>
          ))}
        </ol>
      </section>

      <section
        aria-atomic="true"
        aria-labelledby="overview-status-heading"
        aria-live="polite"
        className="overview-section overview-live-status"
        data-runtime-state={readinessStatus}
        data-testid="overview-live-status"
      >
        <header className="overview-section-heading">
          <span>LIVE STATUS</span>
          <h2 id="overview-status-heading">現在の状態</h2>
          <p>装飾ではなく、gui_server が返した実際の準備状態とセッションだけを表示します。</p>
        </header>
        <div className="overview-status-grid">
          <article data-status={readinessStatus}>
            <span aria-hidden="true" />
            <div>
              <small>TRIAL READINESS</small>
              <h3>{readinessLabel(readinessStatus)}</h3>
              <p>{readinessDetail(readinessStatus)}</p>
            </div>
          </article>
          <article data-status={sessionStatus}>
            <span aria-hidden="true" />
            <div>
              <small>ACTIVE SESSION</small>
              <h3>{sessionLabel(sessionStatus)}</h3>
              <p>{sessionDetail(sessionStatus, activeSession?.id)}</p>
              {activeSession !== null && (
                <a href={withBasePath(trialRoutePath("status", activeSession.id))}>
                  実行状況を開く ↗
                </a>
              )}
            </div>
          </article>
          <article data-status={extensionStatus}>
            <span aria-hidden="true" />
            <div>
              <small>EXTENSION ROOT</small>
              <h3>{prerequisiteLabel(extensionStatus)}</h3>
              <p>{runtimeUnavailable
                ? "runtime-status を取得できません。gui_server と base path を確認してください。"
                : extensionRoot?.detail ?? "設定状態を確認しています。"}</p>
            </div>
          </article>
        </div>
      </section>

      <section aria-labelledby="overview-owner-links-heading" className="overview-owner-links">
        <header className="overview-section-heading">
          <span>DETAILS LIVE ELSEWHERE</span>
          <h2 id="overview-owner-links-heading">運用の詳細は担当ページで確認する</h2>
          <p>概要には判断に必要な入口だけを残し、一覧や内部指標は重複して表示しません。</p>
        </header>
        <div>
          <a data-testid="overview-measurements-link" href={withBasePath(routePath("measurements"))}>
            <span>計測</span>
            <strong>能力マップ、band、report を確認 ↗</strong>
          </a>
          <a data-testid="overview-runs-link" href={withBasePath(routePath("run"))}>
            <span>リポジトリ実行記録</span>
            <strong>workspace/management/runs の記録を確認 ↗</strong>
          </a>
        </div>
      </section>
    </div>
  );
}

function readinessLabel(status: string): string {
  if (status === "ready") return "トライアル利用可";
  if (status === "action_required") return "トライアル利用不可";
  if (status === "unavailable") return "状態取得失敗";
  return "確認中";
}

function readinessDetail(status: string): string {
  if (status === "ready") return "必要な実行前確認へ進めます。";
  if (status === "action_required") return "前提チェックの要対応項目を解決してください。";
  if (status === "unavailable") return "runtime-status を取得できないため、利用可能とは判断しません。";
  return "gui_server から準備状態を取得しています。";
}

function sessionLabel(status: string): string {
  if (status === "running") return "実行中";
  if (status === "recovery_required") return "要復旧";
  if (status === "idle") return "実行中なし";
  if (status === "unavailable") return "状態取得失敗";
  return "確認中";
}

function sessionDetail(status: string, id?: string): string {
  if (status === "running") return `セッション ${shortSessionId(id)} が実行ルートを使用しています。`;
  if (status === "recovery_required") return `セッション ${shortSessionId(id)} の確認と復旧が必要です。`;
  if (status === "idle") return "実行ルートを使用中のトライアルはありません。";
  if (status === "unavailable") return "セッションの有無を確認できません。";
  return "セッション状態を取得しています。";
}

function prerequisiteLabel(status: string): string {
  if (status === "ready") return "設定済み";
  if (status === "unconfigured") return "未設定";
  if (status === "action_required") return "要対応";
  if (status === "unavailable") return "状態取得失敗";
  return "確認中";
}

function shortSessionId(id?: string): string {
  if (id === undefined) return "不明";
  return id.length > 12 ? id.slice(0, 12) : id;
}
