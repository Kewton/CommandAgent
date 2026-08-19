"use client";

import { useEffect, useMemo, useState } from "react";

import { routePath, withBasePath } from "../lib/base-path";
import { describeError, GuiRequestError } from "../lib/errors";
import {
  pinExtensionPack,
  retireExtensionPack,
  stageExtensionPack,
  verifyExtensionPack,
  type PackLifecycleStatus,
  type PackStageReport,
} from "../lib/extension-api";
import {
  blankPackFiles,
  nextjsAcmeFiles,
  type PackWizardFiles,
} from "../lib/pack-wizard-presets";
import { persistTrialToken, restoreTrialToken } from "../lib/trial-token-storage";

const steps = ["対象セル", "出発点", "編集", "検証", "pin"] as const;
const profiles = [
  { id: "nextjs", label: "Next.js" },
  { id: "python-cli", label: "Python CLI" },
  { id: "data", label: "Data" },
  { id: "ingest", label: "Ingest" },
] as const;
const intents = [
  { id: "create", label: "create" },
  { id: "fix", label: "fix" },
  { id: "investigate", label: "investigate" },
] as const;

type WizardStep = 0 | 1 | 2 | 3 | 4;
type StartingPoint = "blank" | "nextjs-acme";
type EditorLifecycle = "draft" | PackLifecycleStatus;
type WizardIssue = {
  fieldId: string;
  label: string;
  message: string;
};

export function PackWizard() {
  const [open, setOpen] = useState(false);
  const [step, setStep] = useState<WizardStep>(0);
  const [maxStep, setMaxStep] = useState<WizardStep>(0);
  const [profile, setProfile] = useState("nextjs");
  const [intent, setIntent] = useState("create");
  const [startingPoint, setStartingPoint] = useState<StartingPoint>("nextjs-acme");
  const [id, setId] = useState("nextjs-acme");
  const [version, setVersion] = useState("1.0.0");
  const [files, setFiles] = useState<PackWizardFiles>(() => cloneFiles(nextjsAcmeFiles));
  const [token, setToken] = useState("");
  const [lifecycle, setLifecycle] = useState<EditorLifecycle>("draft");
  const [report, setReport] = useState<PackStageReport | null>(null);
  const [issues, setIssues] = useState<WizardIssue[]>([]);
  const [busy, setBusy] = useState(false);
  const [retireAcknowledged, setRetireAcknowledged] = useState(false);
  const immutable = lifecycle === "pinned" || lifecycle === "retired";
  const selector = `${id}@${version}`;
  const exampleAvailable = profile === "nextjs" && intent === "create";
  const trialHref = useMemo(
    () => withBasePath(`${routePath("try")}?pack=${encodeURIComponent(selector)}`),
    [selector],
  );

  useEffect(() => {
    setToken(restoreTrialToken());
  }, []);

  useEffect(() => {
    if (!exampleAvailable && startingPoint === "nextjs-acme") setStartingPoint("blank");
  }, [exampleAvailable, startingPoint]);

  function moveTo(next: WizardStep) {
    setStep(next);
    setMaxStep((current) => Math.max(current, next) as WizardStep);
  }

  function applyStartingPoint() {
    if (immutable) return;
    if (startingPoint === "nextjs-acme") {
      setId("nextjs-acme");
      setVersion("1.0.0");
      setFiles(cloneFiles(nextjsAcmeFiles));
    } else {
      const nextId = `local-${profile}-pack`;
      setId(nextId);
      setVersion("1.0.0");
      setFiles(blankPackFiles(profile, intent));
    }
    setLifecycle("draft");
    setReport(null);
    setIssues([]);
    setRetireAcknowledged(false);
    moveTo(2);
  }

  function updateIdentity(field: "id" | "version", value: string) {
    if (immutable) return;
    if (field === "id") setId(value);
    else setVersion(value);
    setFiles((current) => ({
      ...current,
      assist: replaceIdentity(current.assist, field, value),
      eval: replaceIdentity(current.eval, field, value),
    }));
    markDirty();
  }

  function markDirty() {
    setReport(null);
    setIssues([]);
    if (lifecycle !== "draft") setLifecycle("staged");
  }

  function updateDocument(field: "assist" | "eval", value: string) {
    if (immutable) return;
    setFiles((current) => ({ ...current, [field]: value }));
    markDirty();
  }

  function updateMaterial(index: number, field: "name" | "content", value: string) {
    if (immutable) return;
    setFiles((current) => ({
      ...current,
      materials: current.materials.map((material, materialIndex) =>
        materialIndex === index ? { ...material, [field]: value } : material,
      ),
    }));
    markDirty();
  }

  function addMaterial() {
    if (immutable) return;
    setFiles((current) => ({
      ...current,
      materials: [...current.materials, { name: "NOTES.md", content: "# Notes\n" }],
    }));
    markDirty();
  }

  function removeMaterial(index: number) {
    if (immutable) return;
    setFiles((current) => ({
      ...current,
      materials: current.materials.filter((_, materialIndex) => materialIndex !== index),
    }));
    markDirty();
  }

  async function stageAndVerify() {
    const localIssues = validateEditor(id, version, files);
    moveTo(3);
    setReport(null);
    setIssues(localIssues);
    if (localIssues.length > 0) return;
    setBusy(true);
    try {
      const nextReport = await stageExtensionPack(token, {
        id,
        version,
        files: memberMap(files),
      });
      setLifecycle("staged");
      setReport(nextReport);
      setIssues([]);
    } catch (reason) {
      if (reason instanceof GuiRequestError && reason.code === "extension_verification_failed") {
        setLifecycle("staged");
      } else {
        const recoveredLifecycle = immutableLifecycleFromConflict(reason);
        if (recoveredLifecycle !== null) setLifecycle(recoveredLifecycle);
      }
      setIssues([serverIssue(reason, files)]);
    } finally {
      setBusy(false);
    }
  }

  async function reverify() {
    setBusy(true);
    setReport(null);
    setIssues([]);
    try {
      const nextReport = await verifyExtensionPack(token, id, version);
      setLifecycle("staged");
      setReport(nextReport);
    } catch (reason) {
      const recoveredLifecycle = immutableLifecycleFromConflict(reason);
      if (recoveredLifecycle !== null) setLifecycle(recoveredLifecycle);
      setIssues([serverIssue(reason, files)]);
    } finally {
      setBusy(false);
    }
  }

  async function pin() {
    if (report === null) return;
    setBusy(true);
    setIssues([]);
    try {
      await pinExtensionPack(token, id, version, report.hash);
      setLifecycle("pinned");
    } catch (reason) {
      const recoveredLifecycle = immutableLifecycleFromConflict(reason);
      if (recoveredLifecycle !== null) setLifecycle(recoveredLifecycle);
      setIssues([serverIssue(reason, files)]);
    } finally {
      setBusy(false);
    }
  }

  async function retire() {
    if (lifecycle !== "pinned" || !retireAcknowledged) return;
    setBusy(true);
    setIssues([]);
    try {
      await retireExtensionPack(token, id, version);
      setLifecycle("retired");
      setRetireAcknowledged(false);
    } catch (reason) {
      const recoveredLifecycle = immutableLifecycleFromConflict(reason);
      if (recoveredLifecycle !== null) setLifecycle(recoveredLifecycle);
      setIssues([serverIssue(reason, files)]);
    } finally {
      setBusy(false);
    }
  }

  function focusEditorField(fieldId: string) {
    setStep(2);
    window.requestAnimationFrame(() => document.getElementById(fieldId)?.focus());
  }

  if (!open) {
    return (
      <section className="pack-wizard-launch">
        <div>
          <small>LOCAL SUPPLY / GUI</small>
          <h2>pack 作成ウィザード</h2>
          <p>対象セルと出発点を選び、編集した exact bytes を検証してから pin します。</p>
        </div>
        <button
          className="primary-action"
          data-testid="pack-wizard-open"
          onClick={() => setOpen(true)}
          type="button"
        >
          pack 作成ウィザードを開く
        </button>
      </section>
    );
  }

  return (
    <section className="pack-wizard" data-lifecycle={lifecycle} data-testid="pack-wizard">
      <header className="pack-wizard-heading">
        <div>
          <small>LOCAL SUPPLY / 未承認・帯域未計測</small>
          <h2>pack 作成ウィザード</h2>
        </div>
        <button className="text-action" onClick={() => setOpen(false)} type="button">閉じる</button>
      </header>

      <ol className="pack-wizard-steps" aria-label="pack 作成手順">
        {steps.map((label, index) => (
          <li className={index === step ? "current" : index <= maxStep ? "reached" : ""} key={label}>
            <button
              aria-current={index === step ? "step" : undefined}
              disabled={index > maxStep}
              onClick={() => setStep(index as WizardStep)}
              type="button"
            >
              <span>{index + 1}</span>{label}
            </button>
          </li>
        ))}
      </ol>

      {step === 0 && (
        <div className="pack-wizard-panel" data-testid="pack-wizard-target">
          <h3>1. 対象セル</h3>
          <p>pack が追加する profile × intent を固定します。検証時に既存の contract floor と照合されます。</p>
          <div className="pack-wizard-fields two-column">
            <label>
              profile
              <select
                data-testid="pack-wizard-profile"
                disabled={immutable}
                onChange={(event) => setProfile(event.target.value)}
                value={profile}
              >
                {profiles.map((option) => <option key={option.id} value={option.id}>{option.label}</option>)}
              </select>
            </label>
            <label>
              intent
              <select
                data-testid="pack-wizard-intent"
                disabled={immutable}
                onChange={(event) => setIntent(event.target.value)}
                value={intent}
              >
                {intents.map((option) => <option key={option.id} value={option.id}>{option.label}</option>)}
              </select>
            </label>
          </div>
          <WizardActions disabled={immutable} primary="出発点を選ぶ" onPrimary={() => moveTo(1)} />
        </div>
      )}

      {step === 1 && (
        <div className="pack-wizard-panel" data-testid="pack-wizard-starting-point">
          <h3>2. 出発点</h3>
          <p>{profile} × {intent} のメンバーをゼロから始めるか、検証可能な例から始めます。</p>
          <div className="pack-wizard-choice-grid">
            <label className={startingPoint === "blank" ? "selected" : ""}>
              <input
                checked={startingPoint === "blank"}
                disabled={immutable}
                name="pack-starting-point"
                onChange={() => setStartingPoint("blank")}
                type="radio"
              />
              <strong>最小 assist scaffold</strong>
              <small>選択した identity と空の inject から始めます。</small>
            </label>
            <label className={startingPoint === "nextjs-acme" ? "selected" : ""}>
              <input
                checked={startingPoint === "nextjs-acme"}
                data-testid="pack-wizard-nextjs-acme"
                disabled={immutable || !exampleAvailable}
                name="pack-starting-point"
                onChange={() => setStartingPoint("nextjs-acme")}
                type="radio"
              />
              <strong>nextjs-acme 例</strong>
              <small>Next.js × create 専用。assist / eval / materials 2 件を読み込みます。</small>
            </label>
          </div>
          <WizardActions
            disabled={immutable}
            primary="編集を開始"
            onBack={() => setStep(0)}
            onPrimary={applyStartingPoint}
          />
        </div>
      )}

      {step === 2 && (
        <div className="pack-wizard-panel" data-testid="pack-wizard-editor">
          <h3>3. 編集</h3>
          <p>pin 後は編集できません。内容を変えるときは新しい version で staged にしてください。</p>
          <div className="pack-wizard-fields two-column">
            <label>
              pack ID
              <input
                autoCapitalize="none"
                data-testid="pack-wizard-id"
                disabled={immutable}
                id="pack-wizard-id"
                onChange={(event) => updateIdentity("id", event.target.value)}
                spellCheck={false}
                value={id}
              />
            </label>
            <label>
              version
              <input
                autoCapitalize="none"
                data-testid="pack-wizard-version"
                disabled={immutable}
                id="pack-wizard-version"
                onChange={(event) => updateIdentity("version", event.target.value)}
                spellCheck={false}
                value={version}
              />
            </label>
          </div>
          <label className="pack-wizard-token">
            Trial access token
            <input
              autoCapitalize="none"
              autoComplete="off"
              data-testid="pack-wizard-token"
              id="pack-wizard-token"
              onChange={(event) => {
                setToken(event.target.value);
                persistTrialToken(event.target.value);
              }}
              spellCheck={false}
              type="password"
              value={token}
            />
            <small>トークン認証が有効なときだけ必要です。現在の tab と base path にだけ保存します。</small>
          </label>
          <div className="pack-wizard-editors">
            <label>
              assist.yaml
              <textarea
                data-testid="pack-wizard-assist"
                disabled={immutable}
                id="pack-wizard-assist"
                onChange={(event) => updateDocument("assist", event.target.value)}
                rows={16}
                spellCheck={false}
                value={files.assist}
              />
            </label>
            <label>
              eval.yaml
              <textarea
                data-testid="pack-wizard-eval"
                disabled={immutable}
                id="pack-wizard-eval"
                onChange={(event) => updateDocument("eval", event.target.value)}
                rows={16}
                spellCheck={false}
                value={files.eval}
              />
            </label>
          </div>
          <div className="pack-wizard-materials">
            <header>
              <div><strong>materials/*.md</strong><small>直下の UTF-8 Markdown のみ</small></div>
              <button className="secondary-action" disabled={immutable} onClick={addMaterial} type="button">材料を追加</button>
            </header>
            {files.materials.map((material, index) => (
              <article data-testid="pack-wizard-material" key={`${index}-${material.name}`}>
                <label>
                  file name
                  <input
                    disabled={immutable}
                    id={`pack-wizard-material-name-${index}`}
                    onChange={(event) => updateMaterial(index, "name", event.target.value)}
                    spellCheck={false}
                    value={material.name}
                  />
                </label>
                <label>
                  content
                  <textarea
                    disabled={immutable}
                    id={`pack-wizard-material-content-${index}`}
                    onChange={(event) => updateMaterial(index, "content", event.target.value)}
                    rows={8}
                    spellCheck={false}
                    value={material.content}
                  />
                </label>
                <button className="text-action danger" disabled={immutable} onClick={() => removeMaterial(index)} type="button">この材料を外す</button>
              </article>
            ))}
          </div>
          {immutable && <ImmutableNotice lifecycle={lifecycle} />}
          <WizardActions
            busy={busy}
            disabled={immutable}
            primary="保存して検証"
            onBack={() => setStep(1)}
            onPrimary={() => void stageAndVerify()}
          />
        </div>
      )}

      {step === 3 && (
        <div className="pack-wizard-panel" data-testid="pack-wizard-verification">
          <h3>4. 検証</h3>
          <p>GUI ではなくサーバーが strict schema、closed vocabulary、contract floor、scrub、exact-byte hash を検証します。</p>
          <IssueList issues={issues} onFocus={focusEditorField} />
          {busy && <p className="pack-wizard-status" role="status">検証中…</p>}
          {report !== null && (
            <div className="pack-verification-success" data-testid="pack-verification-success">
              <strong>検証済み</strong>
              <dl>
                <div><dt>conformance</dt><dd>{report.conformance.status}</dd></div>
                <div><dt>credential scrub</dt><dd>{report.scrub.status}</dd></div>
                <div><dt>target</dt><dd>{report.conformance.profile} × {report.conformance.intent}</dd></div>
                <div><dt>members</dt><dd>{report.scrub.scanned.length}</dd></div>
              </dl>
              <code data-testid="pack-wizard-hash">{report.hash}</code>
            </div>
          )}
          <div className="pack-wizard-actions">
            <button className="secondary-action" disabled={immutable || busy} onClick={() => setStep(2)} type="button">編集に戻る</button>
            <button className="secondary-action" disabled={immutable || busy || lifecycle !== "staged"} onClick={() => void reverify()} type="button">保存済み bytes を再検証</button>
            <button className="primary-action" data-testid="pack-wizard-to-pin" disabled={report === null || immutable || busy} onClick={() => moveTo(4)} type="button">pin を確認</button>
          </div>
        </div>
      )}

      {step === 4 && (
        <div className="pack-wizard-panel" data-testid="pack-wizard-pin">
          <h3>5. pin</h3>
          <p>pin は検証した exact bytes を固定します。承認や計測済み band を与える操作ではありません。</p>
          <IssueList issues={issues} onFocus={focusEditorField} />
          <div className="pack-pin-review">
            <span>{selector}</span>
            <strong>ローカル（未承認・帯域未計測）</strong>
            <code>{report?.hash ?? "検証済み hash なし"}</code>
          </div>
          {lifecycle === "staged" && (
            <button className="primary-action pack-pin-action" data-testid="pack-wizard-pin-action" disabled={busy || report === null} onClick={() => void pin()} type="button">この hash を pin</button>
          )}
          {lifecycle === "pinned" && (
            <div className="pack-pinned-state" data-testid="pack-wizard-pinned" role="status">
              <strong>pinned — 編集不可</strong>
              <p>ファイルと pin は固定されました。Trial はこの id@version と hash を再取得します。</p>
              <a className="pack-trial-link" data-testid="pack-wizard-trial-link" href={trialHref}>Trial で使う ↗</a>
              <details className="pack-retire-panel">
                <summary>この version を退役させる</summary>
                <p>退役は取り消せません。bytes、pin、journal は保存され、Trial では選択できなくなります。</p>
                <label>
                  <input
                    checked={retireAcknowledged}
                    data-testid="pack-wizard-retire-confirm"
                    onChange={(event) => setRetireAcknowledged(event.target.checked)}
                    type="checkbox"
                  />
                  不可逆の退役であることを確認しました
                </label>
                <button className="danger-action" data-testid="pack-wizard-retire-action" disabled={busy || !retireAcknowledged} onClick={() => void retire()} type="button">退役させる</button>
              </details>
            </div>
          )}
          {lifecycle === "retired" && (
            <div className="pack-retired-state" data-testid="pack-wizard-retired" role="status">
              <strong>retired — 終端状態</strong>
              <p>編集、再 pin、unretire、Trial 選択はできません。変更は新しい version で作成してください。</p>
            </div>
          )}
        </div>
      )}
    </section>
  );
}

function WizardActions({
  primary,
  onPrimary,
  onBack,
  busy = false,
  disabled = false,
}: {
  primary: string;
  onPrimary: () => void;
  onBack?: () => void;
  busy?: boolean;
  disabled?: boolean;
}) {
  return (
    <div className="pack-wizard-actions">
      {onBack !== undefined && <button className="secondary-action" disabled={busy} onClick={onBack} type="button">戻る</button>}
      <button className="primary-action" disabled={busy || disabled} onClick={onPrimary} type="button">
        {busy ? "処理中…" : primary}
      </button>
    </div>
  );
}

function IssueList({ issues, onFocus }: { issues: WizardIssue[]; onFocus: (fieldId: string) => void }) {
  if (issues.length === 0) return null;
  return (
    <div className="pack-wizard-issues" data-testid="pack-wizard-issues" role="alert">
      <strong>{issues.length} 件の項目を修正してください</strong>
      <ul>
        {issues.map((issue, index) => (
          <li key={`${issue.fieldId}-${index}`}>
            <div><b>{issue.label}</b><span>{issue.message}</span></div>
            <button data-focus-target={issue.fieldId} onClick={() => onFocus(issue.fieldId)} type="button">該当項目へ移動</button>
          </li>
        ))}
      </ul>
    </div>
  );
}

function ImmutableNotice({ lifecycle }: { lifecycle: EditorLifecycle }) {
  return (
    <p className="pack-immutable-notice" role="status">
      {lifecycle === "retired" ? "retired は終端状態です。" : "pinned bytes は上書きできません。"}
      新しい version を作成してください。
    </p>
  );
}

function validateEditor(id: string, version: string, files: PackWizardFiles): WizardIssue[] {
  const issues: WizardIssue[] = [];
  if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(id)) {
    issues.push({ fieldId: "pack-wizard-id", label: "pack ID", message: "小文字 ASCII の kebab-case で入力してください。" });
  }
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version)) {
    issues.push({ fieldId: "pack-wizard-version", label: "version", message: "MAJOR.MINOR.PATCH 形式の semantic version で入力してください。" });
  }
  if (files.assist.trim() === "" && files.eval.trim() === "") {
    issues.push({ fieldId: "pack-wizard-assist", label: "assist.yaml / eval.yaml", message: "少なくとも一方が必要です。" });
  }
  const names = new Set<string>();
  files.materials.forEach((material, index) => {
    if (!/^[A-Za-z0-9._-]+\.md$/.test(material.name)) {
      issues.push({ fieldId: `pack-wizard-material-name-${index}`, label: `material ${index + 1} の file name`, message: "materials/ 直下の .md basename にしてください。" });
    } else if (names.has(material.name)) {
      issues.push({ fieldId: `pack-wizard-material-name-${index}`, label: `material ${index + 1} の file name`, message: "同じ file name が複数あります。" });
    }
    names.add(material.name);
  });
  return issues;
}

function serverIssue(reason: unknown, files: PackWizardFiles): WizardIssue {
  const detail = serverDetail(reason);
  const lower = detail.toLowerCase();
  if (reason instanceof GuiRequestError && reason.code === "trial_token_invalid") {
    return { fieldId: "pack-wizard-token", label: "Trial access token", message: describeError(reason) };
  }
  if (lower.includes("eval.yaml")) {
    return { fieldId: "pack-wizard-eval", label: "eval.yaml", message: detail };
  }
  if (lower.includes("assist.yaml") || lower.includes("identities differ")) {
    return { fieldId: "pack-wizard-assist", label: "assist.yaml", message: detail };
  }
  const material = files.materials.findIndex((candidate) => detail.includes(`materials/${candidate.name}`));
  if (material >= 0) {
    return { fieldId: `pack-wizard-material-content-${material}`, label: `materials/${files.materials[material].name}`, message: detail };
  }
  if (lower.includes("version") || lower.includes("major.minor.patch")) {
    return { fieldId: "pack-wizard-version", label: "version", message: detail };
  }
  if (lower.includes("pack id") || lower.includes("identifier") || lower.includes("already pinned") || lower.includes("retired")) {
    return { fieldId: "pack-wizard-id", label: "pack ID / version", message: detail };
  }
  return { fieldId: "pack-wizard-assist", label: "pack members", message: detail };
}

function serverDetail(reason: unknown): string {
  if (reason instanceof GuiRequestError) {
    if (typeof reason.report === "object" && reason.report !== null && "reason" in reason.report) {
      const reportReason = (reason.report as { reason?: unknown }).reason;
      if (typeof reportReason === "string" && reportReason.trim() !== "") return reportReason;
    }
    return reason.serverMessage;
  }
  return describeError(reason);
}

function immutableLifecycleFromConflict(reason: unknown): "pinned" | "retired" | null {
  if (!(reason instanceof GuiRequestError) || reason.code !== "extension_conflict") return null;
  if (reason.serverMessage.toLowerCase().includes("retired")) return "retired";
  if (reason.serverMessage.toLowerCase().includes("pinned")) return "pinned";
  return null;
}

function memberMap(files: PackWizardFiles): Record<string, string> {
  const members: Record<string, string> = {};
  if (files.assist.trim() !== "") members["assist.yaml"] = files.assist;
  if (files.eval.trim() !== "") members["eval.yaml"] = files.eval;
  for (const material of files.materials) members[`materials/${material.name}`] = material.content;
  return members;
}

function replaceIdentity(document: string, field: "id" | "version", value: string): string {
  if (document === "") return document;
  return document.replace(new RegExp(`^(  ${field}: ).*$`, "m"), `$1${value}`);
}

function cloneFiles(files: PackWizardFiles): PackWizardFiles {
  return { ...files, materials: files.materials.map((material) => ({ ...material })) };
}
