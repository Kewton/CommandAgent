"use client";

import { useState, type KeyboardEvent } from "react";

import { PackWizard } from "../../components/pack-wizard";
import { Shell, useShellRuntimeStatus } from "../../components/shell";
import { EmptyState, ErrorState, LoadingState } from "../../components/states";
import { routePath, withBasePath } from "../../lib/base-path";
import type { DocumentRecord, PackSummary, TrialOptions } from "../../lib/types";
import { useResource } from "../../lib/use-resource";

type AssetTab = "profiles" | "packs" | "references";
const assetTabs = ["profiles", "packs", "references"] as const;

const assetTabLabels: Record<AssetTab, string> = {
  profiles: "Layer 2 プロファイル",
  packs: "Layer 3 パック",
  references: "参照資料",
};

const intentLabels: Readonly<Record<string, string>> = {
  create: "作成",
  fix: "修正",
  investigate: "調査",
};

const layerDefinitions = [
  {
    layer: "Layer 1",
    name: "能力語彙",
    source: "compiled capability catalog",
    status: "reviewed / closed vocabulary",
    hash: "build と repository commit に固定",
    assurance: "能力だけでは保証を獲得しない",
    registration: "実装・schema・golden・corpus を含む Issue / PR review",
    can: "型付き source / check を実装し、レビュー後に catalog へ登録",
    cannot: "GUI、YAML、Markdown から任意ロジックや能力語彙を追加",
    locked: true,
  },
  {
    layer: "Layer 2",
    name: "下書きプロファイル",
    source: "private extension root / profiles",
    status: "draft only",
    hash: "manifest exact-byte hash",
    assurance: "上限 static / profile_not_admitted",
    registration: "manifest を配置し、登録 Issue で測定と review を依頼",
    can: "既存の閉じた語彙で task family、contract、checks を組み立てる",
    cannot: "admitted を自己申告し、full assurance へ自己昇格",
    locked: false,
  },
  {
    layer: "Layer 3",
    name: "パック供給",
    source: "repository または private extension root / packs",
    status: "staged -> verified -> pinned -> retired",
    hash: "pack exact-byte hash と pack.sha256",
    assurance: "pack 単独では admission / assurance を付与しない",
    registration: "作成ウィザードで検証・pin、Trial 後に review Issue / PR",
    can: "assist / eval / bounded materials を安全に供給し Trial へ渡す",
    cannot: "pin 後の変更、削除、unretire、未登録 capability の実行",
    locked: false,
  },
  {
    layer: "Layer 4",
    name: "Admission",
    source: "compiled catalog + measured repository evidence",
    status: "maintainer reviewed",
    hash: "reviewed profile / pack / evidence の exact identity",
    assurance: "実行で獲得し、admission ceiling を超えない",
    registration: "測定 evidence 付き Issue / PR と maintainer admission review",
    can: "reviewed identity を Gate 1 と acceptance へ投影",
    cannot: "GUI から admission 追加、帯域 claim、自己昇格",
    locked: true,
  },
] as const;

function assetTitle(document: DocumentRecord): string {
  return document.content.match(/^#\s+(.+)$/m)?.[1] ?? document.id;
}

export default function AssetsPage() {
  return (
    <Shell
      active="assets"
      title="拡張"
      description="4 レイヤーの境界、登録導線、利用不可理由、実行に固定される exact hash を一つの画面で確認します。"
    >
      <ExtensionCatalog />
    </Shell>
  );
}

function ExtensionCatalog() {
  const [tab, setTab] = useState<AssetTab>("profiles");
  const runtime = useShellRuntimeStatus();
  const packs = useResource<PackSummary[]>("packs");
  const options = useResource<TrialOptions>("trial-options");
  const contracts = useResource<DocumentRecord[]>("contracts");
  const suites = useResource<DocumentRecord[]>("suites");
  const draftProfiles = options.data?.profiles.filter((profile) => profile.status === "draft") ?? [];
  const warningCount = packs.data?.filter((pack) => pack.warning !== null).length ?? 0;
  const extensionRoot = runtime?.data?.prerequisites.extension_root ?? null;
  const extensionRootStatus = runtime?.failed
    ? "action_required"
    : extensionRoot?.status ?? "loading";

  function handleTabKeyDown(event: KeyboardEvent<HTMLButtonElement>, current: AssetTab) {
    const currentIndex = assetTabs.indexOf(current);
    let nextIndex: number;
    switch (event.key) {
      case "ArrowRight":
        nextIndex = (currentIndex + 1) % assetTabs.length;
        break;
      case "ArrowLeft":
        nextIndex = (currentIndex - 1 + assetTabs.length) % assetTabs.length;
        break;
      case "Home":
        nextIndex = 0;
        break;
      case "End":
        nextIndex = assetTabs.length - 1;
        break;
      default:
        return;
    }
    event.preventDefault();
    const nextTab = assetTabs[nextIndex];
    setTab(nextTab);
    document.getElementById(`asset-tab-${nextTab}`)?.focus();
  }

  return (
    <>
      <section aria-labelledby="extension-layers-heading" className="extension-boundary">
        <header className="extension-section-heading">
          <div>
            <small>EXTENSION BOUNDARY</small>
            <h2 id="extension-layers-heading">4 レイヤーと依存関係</h2>
          </div>
          <p>上のレイヤーは下のレイヤーを参照します。下位レイヤーの検証だけで上位へ昇格しません。</p>
        </header>
        <ol aria-label="拡張レイヤーの依存順" className="extension-layer-map">
          {layerDefinitions.map((definition, index) => (
            <li
              className={definition.locked ? "extension-layer-card locked" : "extension-layer-card"}
              data-layer={definition.layer}
              data-testid="extension-layer-card"
              key={definition.layer}
            >
              <header>
                <span>{definition.layer}</span>
                {definition.locked && <strong>GUI 変更不可</strong>}
              </header>
              <h3>{definition.name}</h3>
              <ExtensionMetadata
                fields={[
                  ["layer", definition.layer],
                  ["source", definition.source],
                  ["status", definition.status],
                  ["hash", definition.hash],
                  ["assurance", definition.assurance],
                  ["登録／昇格", definition.registration],
                ]}
              />
              <p className="extension-can"><b>できること</b>{definition.can}</p>
              <p className="extension-cannot"><b>できないこと</b>{definition.cannot}</p>
              {index < layerDefinitions.length - 1 && (
                <span aria-hidden="true" className="extension-layer-arrow">→</span>
              )}
            </li>
          ))}
        </ol>
      </section>

      <section
        aria-labelledby="extension-root-heading"
        className="extension-root-card"
        data-status={extensionRootStatus}
        data-testid="extension-root-status"
      >
        <div>
          <small>EXTENSION ROOT</small>
          <h2 id="extension-root-heading">供給ルートの設定状態</h2>
        </div>
        <strong>{extensionRootLabel(extensionRootStatus)}</strong>
        <p>
          {runtime?.failed
            ? "runtime-status を取得できません。gui_server と base path を確認してください。"
            : extensionRoot?.detail ?? "設定状態を確認しています。"}
        </p>
        {extensionRootStatus !== "ready" && extensionRootStatus !== "loading" && (
          <code>gui_server --extension-root &lt;private-directory&gt;</code>
        )}
      </section>

      <div className="asset-tabs" aria-label="拡張カタログ" role="tablist">
        {assetTabs.map((item, index) => (
          <button
            aria-controls={`asset-panel-${item}`}
            aria-selected={tab === item}
            className={tab === item ? "active" : ""}
            id={`asset-tab-${item}`}
            key={item}
            onClick={() => setTab(item)}
            onKeyDown={(event) => handleTabKeyDown(event, item)}
            role="tab"
            tabIndex={tab === item ? 0 : -1}
            type="button"
          >
            <span>0{index + 1}</span>
            {assetTabLabels[item]}
          </button>
        ))}
      </div>

      {tab === "profiles" && (
        <section
          aria-labelledby="asset-tab-profiles"
          className="asset-content"
          id="asset-panel-profiles"
          role="tabpanel"
        >
          <header className="catalog-intro">
            <div>
              <small>LAYER 2 CATALOG</small>
              <h2>下書きプロファイル</h2>
            </div>
            <p>有効な manifest だけを表示します。配置しても admitted にはならず、保証上限は static です。</p>
          </header>
          {options.loading && <LoadingState label="下書きプロファイルを読み込んでいます" />}
          {options.error !== null && <ErrorState message={options.error} />}
          {options.data !== null && draftProfiles.length === 0 && (
            <EmptyState message={extensionRootStatus === "unconfigured"
              ? "extension root が未設定のため、下書きプロファイルを利用できません。"
              : "登録済みの下書きプロファイルはありません。"} />
          )}
          <div className="profile-grid">
            {draftProfiles.map((profile) => (
              <article className="extension-item-card" data-testid="extension-profile-row" key={profile.id}>
                <header>
                  <span>Layer 2</span>
                  <strong>draft</strong>
                </header>
                <h3>{profile.label}</h3>
                <p><code>{profile.id}</code>{profile.base_profile === null ? "" : ` / base: ${profile.base_profile}`}</p>
                <ExtensionMetadata
                  fields={[
                    ["layer", "Layer 2 / draft profile"],
                    ["source", `extension root / profiles/${profile.id}`],
                    ["status", "draft / Trial 選択可"],
                    ["hash", profile.manifest_hash ?? "算出不可 / 利用不可"],
                    ["assurance", `${profile.assurance_ceiling} / profile_not_admitted`],
                    ["登録／昇格", "GUI では昇格不可。測定 evidence 付き登録 Issue で review"],
                  ]}
                />
                <p className="extension-availability positive">利用可: 登録済み manifest。実行しても admission 上限は変わりません。</p>
                <a
                  className="registration-issue-link"
                  data-testid="profile-registration-issue-link"
                  href={profileRegistrationIssueUrl(profile.id, profile.manifest_hash)}
                  rel="noreferrer"
                  target="_blank"
                >
                  安全な登録 Issue を作る ↗
                </a>
                <small className="registration-note">秘密や private root の絶対 path は Issue に含めないでください。</small>
              </article>
            ))}
          </div>
        </section>
      )}

      {tab === "packs" && (
        <section
          aria-labelledby="asset-tab-packs"
          className="asset-content"
          id="asset-panel-packs"
          role="tabpanel"
        >
          <header className="catalog-intro">
            <div>
              <small>LAYER 3 CATALOG</small>
              <h2>パック供給</h2>
            </div>
            <p>既存の作成ウィザード、catalog、Trial 選択を同じ exact-byte identity で接続します。</p>
          </header>
          <PackWizard onCatalogChange={packs.refresh} />
          {packs.loading && <LoadingState label="パックの固定情報を読み込んでいます" />}
          {packs.error !== null && <ErrorState message={packs.error} />}
          {packs.data?.length === 0 && <EmptyState message="固定済みパックが見つかりません。" />}
          {warningCount > 0 && (
            <p className="pack-warning-status" data-testid="pack-warning-status" role="status">
              {warningCount} 件のパック警告があります。
            </p>
          )}
          <div className="pack-grid">
            {packs.data?.map((pack) => {
              const selector = `${pack.id}@${pack.version}`;
              const unavailableReason = packUnavailableReason(pack);
              return (
                <article
                  className={pack.warning === null ? "pack-card" : "pack-card warning"}
                  data-pack-source={pack.source}
                  data-testid="extension-pack-row"
                  key={selector}
                >
                  <header>
                    <span className={`pack-source source-${pack.source}`}>{pack.source_label}</span>
                    <strong>{selector}</strong>
                  </header>
                  <h2>{pack.id}</h2>
                  <p>{pack.path} · {profileDisplayLabel(pack.profile)} × {intentDisplayLabel(pack.intent)}</p>
                  <ExtensionMetadata
                    fields={[
                      ["layer", "Layer 3 / pack supply"],
                      ["source", pack.source_label],
                      ["status", packStatusLabel(pack)],
                      ["hash", pack.observed_hash ?? "算出不可"],
                      ["assurance", packAssuranceLabel(pack)],
                      ["登録／昇格", packRegistrationLabel(pack)],
                    ]}
                  />
                  <div className="pin-block">
                    <span>固定値 / 期待ハッシュ</span>
                    <code>{pack.expected_hash ?? "未固定"}</code>
                    <span>観測ハッシュ</span>
                    <code>{pack.observed_hash ?? "算出不可"}</code>
                  </div>
                  {unavailableReason !== null && (
                    <p className="pack-warning" data-testid="pack-warning" role="note">
                      {unavailableReason}
                    </p>
                  )}
                  <footer>
                    <PackMemberPresence name="assist.yaml" present={pack.has_assist} />
                    <PackMemberPresence name="eval.yaml" present={pack.has_eval} />
                    {pack.trial_eligible && pack.intent !== null && (
                      <a
                        className="pack-trial-link"
                        data-testid="pack-trial-link"
                        href={withBasePath(`${routePath("try")}?pack=${encodeURIComponent(selector)}`)}
                      >
                        トライアルで使う ↗
                      </a>
                    )}
                  </footer>
                </article>
              );
            })}
          </div>
        </section>
      )}

      {tab === "references" && (
        <section
          aria-labelledby="asset-tab-references"
          className="asset-content reference-panel"
          id="asset-panel-references"
          role="tabpanel"
        >
          <header className="catalog-intro">
            <div>
              <small>READ-ONLY REFERENCES</small>
              <h2>Contract / Suite は拡張種別ではありません</h2>
            </div>
            <p>契約文書と計測スイートは admission 判断の参照資料です。この画面から登録・変更・昇格できません。</p>
          </header>
          <DocumentGroup
            documents={contracts.data}
            empty="契約文書が見つかりません。"
            error={contracts.error}
            loading={contracts.loading}
            title="Contract（契約）"
            type="contracts"
          />
          <DocumentGroup
            documents={suites.data}
            empty="計測スイートが見つかりません。"
            error={suites.error}
            loading={suites.loading}
            title="Suite（計測）"
            type="suites"
          />
        </section>
      )}
    </>
  );
}

function ExtensionMetadata({ fields }: { fields: ReadonlyArray<readonly [string, string]> }) {
  return (
    <dl className="extension-metadata">
      {fields.map(([label, value]) => (
        <div key={label}>
          <dt>{label}</dt>
          <dd>{label === "hash" ? <code>{value}</code> : value}</dd>
        </div>
      ))}
    </dl>
  );
}

function DocumentGroup({
  documents,
  error,
  loading,
  empty,
  title,
  type,
}: {
  documents: DocumentRecord[] | null;
  error: string | null;
  loading: boolean;
  empty: string;
  title: string;
  type: "contracts" | "suites";
}) {
  const [openPath, setOpenPath] = useState<string | null>(null);
  return (
    <section aria-labelledby={`reference-${type}-heading`} className="reference-group">
      <h3 id={`reference-${type}-heading`}>{title}</h3>
      {loading && <LoadingState />}
      {error !== null && <ErrorState message={error} />}
      {documents?.length === 0 && <EmptyState message={empty} />}
      <div className="document-grid">
        {documents?.map((document, index) => {
          const open = document.path === openPath;
          const contentId = `document-content-${type}-${index}`;
          return (
            <article className={open ? "document-card open" : "document-card"} key={document.path}>
              <button
                aria-controls={contentId}
                aria-expanded={open}
                onClick={() => setOpenPath(open ? null : document.path)}
                type="button"
              >
                <span>
                  <small>{document.path}</small>
                  <strong>{assetTitle(document)}</strong>
                </span>
                <i aria-hidden="true">{open ? "−" : "+"}</i>
              </button>
              {open && <pre id={contentId} tabIndex={0}>{document.content}</pre>}
            </article>
          );
        })}
      </div>
    </section>
  );
}

function PackMemberPresence({ name, present }: { name: "assist.yaml" | "eval.yaml"; present: boolean }) {
  return (
    <span className={present ? "present" : "absent"}>
      <span aria-hidden="true" className="pack-member-icon">{present ? "✓" : "−"}</span>
      {" "}{name}: {present ? "あり" : "なし"}
    </span>
  );
}

function extensionRootLabel(status: string): string {
  switch (status) {
    case "ready": return "設定済み / 利用可";
    case "unconfigured": return "未設定 / 利用不可";
    case "action_required": return "不正または取得失敗 / 利用不可";
    default: return "確認中";
  }
}

function packStatusLabel(pack: PackSummary): string {
  if (pack.retired) return "retired / 利用不可";
  if (pack.warning !== null) return "invalid / 利用不可";
  if (pack.source === "admitted") return "admitted / Trial 利用可";
  if (pack.trial_eligible) return "pinned / Trial 利用可";
  return `${pack.source} / 未承認`;
}

function packAssuranceLabel(pack: PackSummary): string {
  return pack.source === "admitted"
    ? "実行 evidence から獲得し、profile admission 上限に従う"
    : "未承認・帯域未計測。profile admission 上限を超えない";
}

function packRegistrationLabel(pack: PackSummary): string {
  if (pack.source === "admitted") return "compiled catalog 登録済み。変更は新 version の review";
  if (pack.source === "local") return "ウィザードで verify / pin / Trial 後、登録 Issue / PR";
  return "repository に存在するだけでは未承認。測定 evidence 付き admission review";
}

function packUnavailableReason(pack: PackSummary): string | null {
  if (pack.warning !== null) return `利用不可理由: ${pack.warning}`;
  if (pack.trial_eligible) return null;
  if (!pack.conformance_ok) return "利用不可理由: 現在の profile / intent 契約と非互換です。";
  if (!pack.hash_matches_pin) return "利用不可理由: 未 pin または exact hash が一致しません。";
  return "利用不可理由: repository にあるだけでは未承認です。compiled catalog と測定 review が必要です。";
}

function profileRegistrationIssueUrl(id: string, hash: string | null): string {
  const title = `[Extension profile] Register ${id}`;
  const body = [
    "Layer 2 draft profile registration review",
    "",
    `- profile: ${id}`,
    `- manifest exact hash: ${hash ?? "unavailable"}`,
    "- assurance ceiling before admission: static",
    "",
    "Do not attach secrets or private extension-root paths. Add reproducible tests and measured evidence in a repository PR.",
  ].join("\n");
  return `https://github.com/Kewton/CommandAgent/issues/new?title=${encodeURIComponent(title)}&body=${encodeURIComponent(body)}`;
}

function profileDisplayLabel(profile: string | null): string {
  if (profile === null) return "プロファイル不明";
  if (profile === "community-mini-app") return "コミュニティ・ミニアプリ";
  return profile;
}

function intentDisplayLabel(intent: string | null): string {
  if (intent === null) return "目的不明";
  return intentLabels[intent] ?? "目的不明";
}
