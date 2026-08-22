"use client";

import { useEffect, useState } from "react";

import type { TrialRunState } from "../hooks/use-trial-run";
import { apiPath } from "../lib/base-path";
import type { TrialWorkspaceLease } from "../lib/types";

const localModelProviders = new Set(["ollama", "lm-studio"]);

export function TrialCompose({ run }: { run: TrialRunState }) {
  const {
    busy, checkContract, compatiblePacks, created, error, errorReconnectSessionId,
    inspectWorkspaceLease, launchIdentityLocked, optionsError, reconnectExisting,
    reconnectSessionId, selectedPack, selectedProfile, selectedProvider, setConfirmed, setProposal,
    setProviderChanged, setReconnectSessionId, setStage, setWorkspaceLease, spec,
    trialAccessReady, trialOptions, trialToken, trialTokenAuthEnabled, update, updateTrialToken,
    workspaceLease,
  } = run;
  const providerModels = useProviderModels(spec.provider);
  const [requestedPack, setRequestedPack] = useState<string | null>(null);
  const executorModelUnknown = unknownDiscoveredModel(spec.model, providerModels);
  const plannerModelUnknown = unknownDiscoveredModel(spec.planner_model, providerModels);
  const packPreselectionWarning =
    requestedPack !== null && trialOptions !== null && spec.pack !== requestedPack;

  useEffect(() => {
    setRequestedPack(new URLSearchParams(window.location.search).get("pack"));
  }, []);

  return (
    <div className="trial-compose panel">
      <header className="panel-heading">
        <div>
          <span className="panel-index">GATE 1 / 実行前確認</span>
          <h2>実行内容を確認</h2>
        </div>
        <span className="gate-chip">下書き</span>
      </header>
      <div className="gate-one-primer" data-testid="gate-one-primer">
        <strong>Gate 1 は CLI 実行前の確認です</strong>
        <p>
          目標、正確なモデル ID、変更範囲、検証条件をカードで確認します。
          サンプルも自動実行されず、確認チェックを入れるまで CLI は起動しません。
        </p>
      </div>
      <label htmlFor="trial-goal">目標</label>
      <textarea
        data-testid="trial-goal"
        disabled={launchIdentityLocked}
        id="trial-goal"
        onChange={(event) => update("goal", event.target.value)}
        rows={5}
        value={spec.goal}
      />
      {trialTokenAuthEnabled ? (
        <>
          <label htmlFor="trial-token">Trial アクセストークン</label>
          <input
            autoComplete="off"
            autoCapitalize="none"
            data-testid="trial-token"
            disabled={launchIdentityLocked}
            id="trial-token"
            onChange={(event) => {
              updateTrialToken(event.target.value);
              setWorkspaceLease(null);
              if (created === null) {
                setProposal(null);
                setConfirmed(false);
                setStage("compose");
              }
            }}
            spellCheck={false}
            type="password"
            value={trialToken}
          />
        </>
      ) : (
        <p className="source-note" data-testid="trial-token-auth-disabled">
          Trial トークン認証はサーバー設定で無効です。
        </p>
      )}
      <div
        className={`lease-status-card ${workspaceLease?.status ?? "unknown"}`}
        data-testid="workspace-lease-status"
      >
        <div>
          <span>ワークスペースのリース状態</span>
          <strong>{workspaceLeaseLabel(workspaceLease)}</strong>
        </div>
        {workspaceLease !== null && workspaceLease.status !== "idle" && (
          <code data-testid="workspace-lease-session">{workspaceLease.session_id}</code>
        )}
        <p>読み取り専用の確認です。リースの解除や CLI プロセスの起動は行いません。</p>
        <button
          className="secondary-action"
          data-testid="inspect-workspace-lease"
          disabled={busy}
          onClick={() => void inspectWorkspaceLease()}
          type="button"
        >
          ワークスペースのリースを確認
        </button>
      </div>
      <div className="reconnect-card" data-testid="reconnect-card">
        <label htmlFor="reconnect-session">既存セッション ID</label>
        <div>
          <input
            autoCapitalize="none"
            autoComplete="off"
            data-testid="reconnect-session"
            id="reconnect-session"
            onChange={(event) => setReconnectSessionId(event.target.value)}
            spellCheck={false}
            value={reconnectSessionId}
          />
          <button
            className="secondary-action"
            data-testid="reconnect-session-button"
            disabled={busy || reconnectSessionId.trim() === "" || !trialAccessReady}
            onClick={() => void reconnectExisting()}
            type="button"
          >
            監視を再接続
          </button>
        </div>
        <small>GET のみを使用し、別の CLI プロセスは起動しません。</small>
      </div>
      <div className="trial-fields">
        <label>
          プロファイル
          <select
            data-testid="trial-profile"
            disabled={launchIdentityLocked || trialOptions === null}
            value={spec.profile}
            onChange={(event) => {
              setRequestedPack(null);
              update("profile", event.target.value);
            }}
          >
            {trialOptions === null ? (
              <option value={spec.profile}>許可済みプロファイルを読み込み中…</option>
            ) : (
              trialOptions.profiles.map((option) => (
                <option key={option.id} value={option.id}>{option.label}</option>
              ))
            )}
          </select>
          {selectedProfile !== undefined && (
            <small className="trial-field-hint" data-testid="trial-profile-description">
              {selectedProfile.description}
              {selectedProfile.status === "draft" && (
                <> manifest: {selectedProfile.manifest_hash} · 未承認 · 保証上限 static</>
              )}
            </small>
          )}
        </label>
        <label>
          検証 pack
          <select
            data-testid="trial-pack"
            disabled={launchIdentityLocked || trialOptions === null || selectedProfile?.status === "draft"}
            value={spec.pack ?? ""}
            onChange={(event) => {
              setRequestedPack(null);
              update("pack", event.target.value || null);
            }}
          >
            <option value="">選択なし</option>
            {compatiblePacks.map((option) => {
              const selector = `${option.id}@${option.version}`;
              return (
                <option key={selector} value={selector}>
                  {selector} · {option.source_label}
                </option>
              );
            })}
          </select>
          {selectedPack !== undefined && (
            <small className="trial-field-hint" data-testid="trial-pack-source">
              {selectedPack.profile} × {selectedPack.intent} · 供給元: {selectedPack.source_label}
            </small>
          )}
          {selectedProfile?.status === "draft" && (
            <small className="trial-field-hint" data-testid="trial-draft-pack-note">
              draft profile では検証 pack は「選択なし」固定です。
            </small>
          )}
          {packPreselectionWarning && (
            <small className="trial-field-hint" data-testid="trial-pack-preselection-warning" role="status">
              この pack は現在の profile / intent では選べません。
            </small>
          )}
        </label>
        <label>
          プロバイダー
          <select
            data-testid="trial-provider"
            disabled={launchIdentityLocked || trialOptions === null}
            value={spec.provider}
            onChange={(event) => {
              update("provider", event.target.value);
              setProviderChanged(true);
            }}
          >
            {trialOptions === null ? (
              <option value={spec.provider}>プロバイダーを読み込み中…</option>
            ) : (
              trialOptions.providers.map((option) => (
                <option key={option.id} value={option.id}>{option.label}</option>
              ))
            )}
          </select>
        </label>
        <label>
          実行モデル
          <input
            aria-describedby={describedBy(
              run.providerChanged ? "trial-provider-model-hint" : null,
              executorModelUnknown ? "trial-executor-model-warning" : null,
            )}
            data-testid="trial-executor-model"
            disabled={launchIdentityLocked}
            list="trial-provider-model-options"
            placeholder="正確なモデル ID"
            value={spec.model}
            onChange={(event) => update("model", event.target.value)}
          />
          {run.providerChanged && selectedProvider !== undefined && (
            <small
              className="trial-model-warning"
              data-testid="trial-provider-model-hint"
              id="trial-provider-model-hint"
              role="status"
            >
              プロバイダーを変更しても実行モデルは自動更新されません。{selectedProvider.model_hint}
            </small>
          )}
          {executorModelUnknown && (
            <small
              className="trial-model-warning"
              data-testid="trial-executor-model-warning"
              id="trial-executor-model-warning"
              role="status"
            >
              この実行モデルは取得済みの候補にありません。正確な ID か確認してください。
            </small>
          )}
        </label>
        <label>
          計画モデル
          <input
            aria-describedby={plannerModelUnknown ? "trial-planner-model-warning" : undefined}
            data-testid="trial-planner-model"
            disabled={launchIdentityLocked}
            list="trial-provider-model-options"
            placeholder="正確なモデル ID"
            value={spec.planner_model}
            onChange={(event) => update("planner_model", event.target.value)}
          />
          {plannerModelUnknown && (
            <small
              className="trial-model-warning"
              data-testid="trial-planner-model-warning"
              id="trial-planner-model-warning"
              role="status"
            >
              この計画モデルは取得済みの候補にありません。正確な ID か確認してください。
            </small>
          )}
        </label>
        <datalist id="trial-provider-model-options">
          {providerModels.map((model) => <option key={model} value={model} />)}
        </datalist>
      </div>
      <div className="trial-action-bar trial-request-actions">
        <button
          className="secondary-action"
          data-testid="check-contract"
          disabled={busy || launchIdentityLocked}
          onClick={() => void checkContract()}
          type="button"
        >
          契約と見積りを確認
        </button>
      </div>
      {optionsError !== null && <p className="trial-error" role="alert">{optionsError}</p>}
      {error !== null && (
        <div className="trial-error" role="alert">
          <p>{error}</p>
          {errorReconnectSessionId !== null && (
            <a
              data-testid="reconnect-session-link"
              href={`?session=${encodeURIComponent(errorReconnectSessionId)}`}
              onClick={(event) => {
                event.preventDefault();
                void reconnectExisting(errorReconnectSessionId);
              }}
            >
              セッション {errorReconnectSessionId} に再接続
            </a>
          )}
        </div>
      )}
    </div>
  );
}

function workspaceLeaseLabel(lease: TrialWorkspaceLease | null): string {
  if (lease === null) return "未確認";
  if (lease.status === "recovery_required") return "復旧が必要";
  if (lease.status === "running") return "実行中";
  return "待機中";
}

function unknownDiscoveredModel(model: string, candidates: string[]): boolean {
  return candidates.length > 0 && model.trim() !== "" && !candidates.includes(model);
}

function describedBy(...ids: Array<string | null>): string | undefined {
  const value = ids.filter((id): id is string => id !== null).join(" ");
  return value === "" ? undefined : value;
}

function useProviderModels(provider: string): string[] {
  const [models, setModels] = useState<string[]>([]);

  useEffect(() => {
    if (!localModelProviders.has(provider)) {
      setModels([]);
      return;
    }
    const controller = new AbortController();
    setModels([]);
    void fetch(apiPath("provider-models", new URLSearchParams({ provider })), {
      cache: "no-store",
      signal: controller.signal,
    })
      .then(async (response) => response.ok ? response.json() as Promise<unknown> : [])
      .then((value) => {
        if (!controller.signal.aborted) {
          setModels(
            Array.isArray(value)
              ? value.filter((model): model is string => typeof model === "string")
              : [],
          );
        }
      })
      .catch((reason: unknown) => {
        if (!(reason instanceof DOMException && reason.name === "AbortError")) setModels([]);
      });
    return () => controller.abort();
  }, [provider]);

  return models;
}
