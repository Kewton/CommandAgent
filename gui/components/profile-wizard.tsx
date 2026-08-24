"use client";

import { useEffect, useState } from "react";

import { useShellRuntimeStatus } from "./shell";
import { describeError } from "../lib/errors";
import {
  listExtensionProfiles,
  previewExtensionProfile,
  registerExtensionProfile,
  type ExtensionProfileCatalogEntry,
  type ExtensionProfilePreview,
  type ExtensionProfileRegistration,
  type ProfileDocumentKind,
} from "../lib/extension-api";
import { persistTrialToken, restoreTrialToken } from "../lib/trial-token-storage";

const manifestPath = "profiles/neutral-profile/manifest.toml";
const overlayPath = "profiles/nextjs/overlay.toml";

export function ProfileWizard({ enabled }: { enabled: boolean }) {
  const runtime = useShellRuntimeStatus();
  const [open, setOpen] = useState(false);
  const [kind, setKind] = useState<ProfileDocumentKind>("manifest");
  const [path, setPath] = useState(manifestPath);
  const [content, setContent] = useState(manifestTemplate("neutral-profile"));
  const [token, setToken] = useState("");
  const [preview, setPreview] = useState<ExtensionProfilePreview | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const [registration, setRegistration] = useState<ExtensionProfileRegistration | null>(null);
  const [catalog, setCatalog] = useState<ExtensionProfileCatalogEntry[] | null>(null);
  const [catalogError, setCatalogError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const tokenAuthEnabled = runtime?.data?.trial_token_auth_enabled !== false;

  useEffect(() => {
    const restored = restoreTrialToken();
    setToken(restored);
    if (enabled && restored !== "") void refreshCatalog(restored);
  // The initial catalog request intentionally runs once per root readiness change.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled]);

  useEffect(() => {
    if (runtime?.data?.trial_token_auth_enabled === false) {
      setToken("");
      persistTrialToken("");
      if (enabled) void refreshCatalog("");
    }
  // Authentication mode changes are server-owned and require a catalog retry.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled, runtime?.data?.trial_token_auth_enabled]);

  function markDirty(nextPath = path, nextContent = content) {
    setPath(nextPath);
    setContent(nextContent);
    setPreview(null);
    setConfirmed(false);
    setRegistration(null);
    setError(null);
  }

  function changeKind(next: ProfileDocumentKind) {
    setKind(next);
    if (next === "manifest") {
      markDirty(manifestPath, manifestTemplate("neutral-profile"));
    } else {
      markDirty(overlayPath, overlayTemplate("nextjs-extra"));
    }
  }

  async function refreshCatalog(nextToken = token) {
    try {
      setCatalog(await listExtensionProfiles(nextToken));
      setCatalogError(null);
    } catch (reason) {
      setCatalogError(describeError(reason));
    }
  }

  async function validate() {
    setBusy(true);
    setPreview(null);
    setConfirmed(false);
    setRegistration(null);
    setError(null);
    try {
      setPreview(await previewExtensionProfile(token, { path, content }));
    } catch (reason) {
      setError(describeError(reason));
    } finally {
      setBusy(false);
    }
  }

  async function save() {
    if (preview === null || !confirmed) return;
    setBusy(true);
    setRegistration(null);
    setError(null);
    try {
      const saved = await registerExtensionProfile(token, {
        path,
        content,
        expected_hash: preview.hash,
      });
      setRegistration(saved);
      setPreview(saved.profile);
      await refreshCatalog();
    } catch (reason) {
      setError(describeError(reason));
    } finally {
      setBusy(false);
    }
  }

  const wizard = !open ? (
    <section className="pack-wizard-launch profile-wizard-launch">
      <div>
        <small>ローカル供給 / DRAFT ONLY</small>
        <h2>プロファイル登録ウィザード</h2>
        <p>既存 validator で preview し、exact hash と保存先を確認してから atomic に保存します。</p>
        {!enabled && <p className="profile-wizard-disabled">開始するには --extension-root が必要です。</p>}
      </div>
      <button
        className="primary-action"
        data-testid="profile-wizard-open"
        disabled={!enabled}
        onClick={() => {
          if (!enabled) return;
          setOpen(true);
          void refreshCatalog();
        }}
        type="button"
      >
        登録ウィザードを開く
      </button>
    </section>
  ) : (
    <section className="pack-wizard profile-wizard" data-testid="profile-wizard">
      <header className="pack-wizard-heading">
        <div>
          <small>Layer 2 / draft・未承認・保証上限 static</small>
          <h2>プロファイル登録ウィザード</h2>
        </div>
        <button className="text-action" onClick={() => setOpen(false)} type="button">閉じる</button>
      </header>
      <div className="pack-wizard-panel">
        <h3>1. 文書を編集して検証</h3>
        <p>保存先は extension root 内の profile 文書だけです。絶対 path、traversal、symlink は拒否されます。</p>
        <div className="pack-wizard-fields two-column">
          <label>
            文書種別
            <select
              data-testid="profile-wizard-kind"
              onChange={(event) => changeKind(event.target.value as ProfileDocumentKind)}
              value={kind}
            >
              <option value="manifest">compact manifest v2</option>
              <option value="overlay">additive overlay v1</option>
            </select>
          </label>
          <label>
            extension-root 相対 path
            <input
              autoCapitalize="none"
              data-testid="profile-wizard-path"
              onChange={(event) => markDirty(event.target.value, content)}
              spellCheck={false}
              value={path}
            />
          </label>
        </div>
        {tokenAuthEnabled ? (
          <label className="pack-wizard-token">
            Trial アクセストークン
            <input
              autoCapitalize="none"
              autoComplete="off"
              data-testid="profile-wizard-token"
              onChange={(event) => {
                setToken(event.target.value);
                persistTrialToken(event.target.value);
              }}
              spellCheck={false}
              type="password"
              value={token}
            />
            <small>現在のタブと base path にだけ保存します。</small>
          </label>
        ) : (
          <p className="source-note" data-testid="profile-wizard-token-auth-disabled">
            トークン認証はサーバー設定で無効です。Origin 検証は引き続き必須です。
          </p>
        )}
        <label className="profile-document-editor">
          {kind === "manifest" ? "manifest.toml" : "overlay.toml"}
          <textarea
            data-testid="profile-wizard-content"
            onChange={(event) => markDirty(path, event.target.value)}
            rows={18}
            spellCheck={false}
            value={content}
          />
        </label>
        <div className="pack-wizard-actions">
          <button
            className="primary-action"
            data-testid="profile-wizard-preview"
            disabled={busy}
            onClick={() => void validate()}
            type="button"
          >
            {busy ? "検証中…" : "保存前に検証"}
          </button>
        </div>

        {error !== null && <p className="trial-error" data-testid="profile-wizard-error" role="alert">{error}</p>}
        {preview !== null && (
          <section className="profile-preview" data-testid="profile-wizard-confirmation">
            <h3>2. exact identity を確認</h3>
            <dl className="extension-metadata">
              <div><dt>profile id</dt><dd><code>{preview.id}</code></dd></div>
              <div><dt>normalized path</dt><dd><code>{preview.path}</code></dd></div>
              <div><dt>exact hash</dt><dd><code>{preview.hash}</code></dd></div>
              <div><dt>status</dt><dd>{preview.status} / 未承認</dd></div>
              <div><dt>assurance</dt><dd>上限 {preview.assurance_ceiling}</dd></div>
              <div><dt>base</dt><dd>{preview.base_profile ?? "なし / standalone"}</dd></div>
            </dl>
            {preview.warnings.map((warning) => <p className="pack-warning" key={warning}>{warning}</p>)}
            <label className="profile-confirmation-check">
              <input
                checked={confirmed}
                data-testid="profile-wizard-confirm"
                onChange={(event) => setConfirmed(event.target.checked)}
                type="checkbox"
              />
              この profile id、normalized path、exact hash、draft/static 上限を確認しました。
            </label>
            <div className="pack-wizard-actions">
              <button
                className="primary-action"
                data-testid="profile-wizard-register"
                disabled={busy || !confirmed}
                onClick={() => void save()}
                type="button"
              >
                {busy ? "保存中…" : "atomic に保存"}
              </button>
            </div>
          </section>
        )}
        {registration !== null && (
          <section
            className="profile-registration-result"
            data-restart-required={registration.restart_required}
            data-testid="profile-registration-result"
            role="status"
          >
            <strong>{registration.idempotent ? "同一内容を確認しました" : "保存しました"}</strong>
            <p>保存成功と runtime 反映は別です。{registration.restart_instruction}</p>
            <code>restart_required: {String(registration.restart_required)}</code>
          </section>
        )}
      </div>
    </section>
  );

  return (
    <>
      {wizard}
      <ProfileSupplyCatalog
        catalog={catalog}
        error={catalogError}
        onRefresh={() => void refreshCatalog()}
      />
    </>
  );
}

function ProfileSupplyCatalog({
  catalog,
  error,
  onRefresh,
}: {
  catalog: ExtensionProfileCatalogEntry[] | null;
  error: string | null;
  onRefresh: () => void;
}) {
  return (
    <section className="profile-supply-catalog" data-testid="profile-supply-catalog">
      <header>
        <div><small>LIVE EXTENSION-ROOT CATALOG</small><h3>供給済み profile</h3></div>
        <button className="secondary-action" onClick={onRefresh} type="button">更新</button>
      </header>
      {error !== null && <p className="trial-error">{error}</p>}
      {catalog !== null && catalog.length === 0 && <p className="source-note">供給済み文書はありません。</p>}
      <div className="profile-grid">
        {catalog?.map((entry) => (
          <article className="extension-item-card" data-testid="profile-supply-row" key={entry.id}>
            <header><span>{entry.kind}</span><strong>{entry.status}</strong></header>
            <h3>{entry.display_name}</h3>
            <dl className="extension-metadata">
              <div><dt>source</dt><dd>{entry.source}</dd></div>
              <div><dt>path</dt><dd><code>{entry.path}</code></dd></div>
              <div><dt>hash</dt><dd><code>{entry.hash}</code></dd></div>
              <div><dt>availability</dt><dd>{entry.available ? "Trial 利用可" : "runtime 未反映"}</dd></div>
            </dl>
            <p className={entry.available ? "extension-availability positive" : "pack-warning"}>
              {entry.restart_required
                ? "restart_required: GUI サーバー再起動後に Trial 候補へ反映"
                : "draft / 未承認 / 保証上限 static として利用可"}
            </p>
          </article>
        ))}
      </div>
    </section>
  );
}

function manifestTemplate(id: string): string {
  return `[metadata]
id = "${id}"
display_name = "${id}"
schema_version = "v2"
task_family = "unknown"
[plan]
intent = "create"
phases = [{ id = "implementation", prompt = "Complete the requested work for {goal}." }]
[artifacts]
required = ["README.md"]
[guidance.variants.default]
triggers = [{ condition = "always" }]
messages = { instruction = "Keep the implementation scoped to the requested goal." }
[[checks.final]]
id = "scaffold_files_present"
params = { files = ["README.md"] }
`;
}

function overlayTemplate(id: string): string {
  return `[metadata]
id = "${id}"
display_name = "Next.js extra obligations"
schema_version = "v1"
status = "draft"
[overlay]
base_profile = "nextjs"
mode = "additive"
[artifacts]
required = ["SECURITY.md"]
`;
}
