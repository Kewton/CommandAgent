"use client";

import { useCallback, useEffect, useRef, useState, type Dispatch, type SetStateAction } from "react";

import { useShellRuntimeStatus } from "../components/shell";
import {
  describeError,
  GuiRequestError,
  isTrialTokenRejected,
  reconnectSessionId as reconnectIdFromError,
} from "../lib/errors";
import {
  createSession,
  fetchPackOptions,
  fetchTrialOptions,
  fetchWorkspaceLease,
  proposeSession,
} from "../lib/trial-api";
import {
  persistTrialToken,
  removeRejectedTrialToken,
  restoreTrialToken,
} from "../lib/trial-token-storage";
import type {
  CreatedSession,
  PackOptions,
  SessionSpec,
  TrialOptions,
  TrialWorkspaceLease,
} from "../lib/types";

export type ScreenStage = "compose" | "gate_1" | "gate_2" | "terminal" | "closed";

type UseTrialComposeProps = {
  stage: ScreenStage;
  setStage: Dispatch<SetStateAction<ScreenStage>>;
};

const initialSpec: SessionSpec = {
  goal: "",
  profile: "python-cli",
  provider: "ollama",
  model: "",
  planner_provider: "ollama",
  planner_model: "",
  pack: null,
  think: null,
};

export function useTrialCompose({ stage, setStage }: UseTrialComposeProps) {
  const runtime = useShellRuntimeStatus();
  const composeRef = useRef<HTMLDivElement>(null);
  const gateOneRef = useRef<HTMLElement>(null);
  const packPreselectionApplied = useRef(false);
  const [trialToken, setTrialToken] = useState("");
  const [reconnectSessionId, setReconnectSessionId] = useState("");
  const [spec, setSpec] = useState<SessionSpec>(initialSpec);
  const [proposal, setProposal] = useState<Awaited<ReturnType<typeof proposeSession>> | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [errorReconnectSessionId, setErrorReconnectSessionId] = useState<string | null>(null);
  const [trialOptions, setTrialOptions] = useState<TrialOptions | null>(null);
  const [packOptions, setPackOptions] = useState<PackOptions | null>(null);
  const [optionsError, setOptionsError] = useState<string | null>(null);
  const [providerChanged, setProviderChanged] = useState(false);
  const [workspaceLease, setWorkspaceLease] = useState<TrialWorkspaceLease | null>(null);
  const trialTokenAuthEnabled = runtime?.data?.trial_token_auth_enabled !== false;
  const trialAccessReady = !trialTokenAuthEnabled || trialToken.trim() !== "";
  const launchIdentityLocked = stage === "gate_2" || stage === "terminal" || stage === "closed";

  useEffect(() => {
    const parameters = new URLSearchParams(window.location.search);
    if (parameters.get("sample") === "python-cli") {
      setSpec((current) => ({
        ...current,
        goal: "--pattern で行を絞り込む CLI コマンドを作ってください",
        profile: "python-cli",
        pack: "cli-assist@1.0.0",
      }));
    }
    const sessionId = parameters.get("session");
    if (sessionId !== null) setReconnectSessionId(sessionId);
  }, []);

  useEffect(() => setTrialToken(restoreTrialToken()), []);
  useEffect(() => {
    if (runtime?.data?.trial_token_auth_enabled === false) {
      setTrialToken("");
      persistTrialToken("");
    }
  }, [runtime?.data?.trial_token_auth_enabled]);

  useEffect(() => {
    let cancelled = false;
    void Promise.all([fetchTrialOptions(), fetchPackOptions()])
      .then(([options, packs]) => {
        if (!cancelled) {
          setTrialOptions(options);
          setPackOptions(packs);
        }
      })
      .catch((reason: unknown) => {
        if (!cancelled) setOptionsError(describeError(reason));
      });
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    if (packOptions === null || packPreselectionApplied.current) return;
    packPreselectionApplied.current = true;
    const selector = new URLSearchParams(window.location.search).get("pack");
    const option = packOptions.packs.find(
      (candidate) =>
        candidate.intent === "create" && `${candidate.id}@${candidate.version}` === selector,
    );
    if (selector === null) return;
    if (option !== undefined) {
      setSpec((current) => ({ ...current, profile: option.profile, pack: selector }));
      return;
    }
    setSpec((current) => ({ ...current, pack: null }));
  }, [packOptions]);

  const updateTrialToken = useCallback((value: string) => {
    setTrialToken(value);
    persistTrialToken(value);
  }, []);

  const rejectTrialToken = useCallback((reason: unknown, rejectedValue: string) => {
    if (!isTrialTokenRejected(reason)) return false;
    removeRejectedTrialToken(rejectedValue);
    setTrialToken((current) => current.trim() === rejectedValue.trim() ? "" : current);
    return true;
  }, []);

  const discardProposal = useCallback(() => {
    setProposal(null);
    setConfirmed(false);
  }, []);

  const editProposal = useCallback(() => {
    discardProposal();
    setError(null);
    setErrorReconnectSessionId(null);
    setStage("compose");
  }, [discardProposal, setStage]);

  function update<K extends keyof SessionSpec>(field: K, value: SessionSpec[K]) {
    setSpec((current) => {
      if (field === "profile") return { ...current, profile: value as string, pack: null };
      if (field === "provider" || field === "planner_provider") {
        const next = { ...current, [field]: value as string };
        return next.provider === "ollama" || next.planner_provider === "ollama"
          ? next
          : { ...next, think: null };
      }
      return { ...current, [field]: value };
    });
    setProposal(null);
    setConfirmed(false);
    setError(null);
    setErrorReconnectSessionId(null);
    setStage("compose");
  }

  function recordError(reason: unknown) {
    rejectTrialToken(reason, trialToken);
    setError(trialErrorDescription(reason));
    const active = reconnectIdFromError(reason);
    setErrorReconnectSessionId(active);
    if (active !== null) {
      setReconnectSessionId(active);
      replaceSessionQuery(active);
    }
  }

  async function checkContract() {
    const missing = missingContractField(spec, trialAccessReady);
    if (missing !== null) {
      discardProposal();
      setStage("compose");
      setError(missing);
      return;
    }
    discardProposal();
    setStage("compose");
    setBusy(true);
    setError(null);
    setErrorReconnectSessionId(null);
    try {
      const lease = await fetchWorkspaceLease(trialToken);
      setWorkspaceLease(lease);
      if (lease.status !== "idle") {
        setReconnectSessionId(lease.session_id);
        setErrorReconnectSessionId(lease.session_id);
        setError(leaseLaunchBlockReason(lease));
        return;
      }
      setProposal(await proposeSession(trialToken, spec));
      setConfirmed(false);
      setStage("gate_1");
    } catch (reason) {
      recordError(reason);
    } finally {
      setBusy(false);
    }
  }

  async function inspectWorkspaceLease() {
    if (!trialAccessReady) {
      setError("ワークスペースのリースを確認する前に、実行時のトライアルアクセストークンを入力してください。");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      setWorkspaceLease(await fetchWorkspaceLease(trialToken));
    } catch (reason) {
      recordError(reason);
    } finally {
      setBusy(false);
    }
  }

  async function launchConfirmed(onLaunched: (value: CreatedSession) => void) {
    if (!confirmed || proposal === null) {
      setError("起動するには Gate 1 の明示的な確認が必要です。");
      return;
    }
    setBusy(true);
    setError(null);
    setErrorReconnectSessionId(null);
    try {
      const lease = await fetchWorkspaceLease(trialToken);
      setWorkspaceLease(lease);
      if (lease.status !== "idle") {
        setReconnectSessionId(lease.session_id);
        setErrorReconnectSessionId(lease.session_id);
        setError(leaseLaunchBlockReason(lease));
        return;
      }
      const value = await createSession(trialToken, spec, proposal.card_hash);
      setWorkspaceLease(null);
      onLaunched(value);
    } catch (reason) {
      if (reason instanceof GuiRequestError && reason.status === 409) {
        const currentLease = await fetchWorkspaceLease(trialToken).catch(() => null);
        if (currentLease !== null) setWorkspaceLease(currentLease);
      }
      if (
        reason instanceof GuiRequestError &&
        [401, 412, 428].includes(reason.status)
      ) {
        discardProposal();
        setStage("compose");
      }
      recordError(reason);
    } finally {
      setBusy(false);
    }
  }

  function resetForNewRun() {
    setProposal(null);
    setConfirmed(false);
    setError(null);
    setErrorReconnectSessionId(null);
  }

  const selectedProfile = trialOptions?.profiles.find((option) => option.id === spec.profile);
  const selectedProvider = trialOptions?.providers.find((option) => option.id === spec.provider);
  const compatiblePacks = packOptions?.packs.filter(
    (option) => option.profile === spec.profile && option.intent === "create",
  ) ?? [];
  const selectedPack = compatiblePacks.find(
    (option) => `${option.id}@${option.version}` === spec.pack,
  );

  return {
    busy, checkContract, compatiblePacks, composeRef, confirmed, error, errorReconnectSessionId,
    editProposal, gateOneRef, inspectWorkspaceLease,
    launchBlockReason: leaseLaunchBlockReason(workspaceLease),
    launchConfirmed, launchIdentityLocked, optionsError, proposal, providerChanged,
    reconnectSessionId, recordError, rejectTrialToken, resetForNewRun, selectedPack,
    selectedProfile, selectedProvider, setBusy, setConfirmed, setError,
    setErrorReconnectSessionId, setProposal, setProviderChanged, setReconnectSessionId,
    setWorkspaceLease, spec, trialAccessReady, trialOptions, trialToken,
    trialTokenAuthEnabled, update, updateTrialToken, workspaceLease,
  };
}

function missingContractField(spec: SessionSpec, trialAccessReady: boolean): string | null {
  const action = "「契約と見積りを確認」";
  if (spec.goal.trim() === "") {
    return `契約を確認する前に、目標を入力してください。続けるには${action}を選びます。`;
  }
  if (spec.model.trim() === "") {
    return `契約を確認する前に、実行モデルの正確な ID を入力してください。続けるには${action}を選びます。`;
  }
  if (spec.planner_model.trim() === "") {
    return `契約を確認する前に、計画モデルの正確な ID を入力してください。続けるには${action}を選びます。`;
  }
  if (!trialAccessReady) {
    return `契約を確認する前に、実行時の Trial アクセストークンを入力してください。続けるには${action}を選びます。`;
  }
  return null;
}

function leaseLaunchBlockReason(lease: TrialWorkspaceLease | null): string | null {
  if (lease === null || lease.status === "idle") return null;
  if (lease.status === "running") {
    return `実行中のセッション ${lease.session_id} がワークスペースを使用しているため、新しい起動はできません。再接続してください。`;
  }
  return `セッション ${lease.session_id} のワークスペース復旧が必要です。再接続してください。新しい起動はできません。`;
}

function trialErrorDescription(reason: unknown): string {
  return describeError(reason)
    .replaceAll("「契約と価格を確認」", "「契約と見積りを確認」")
    .replaceAll("契約と価格を確認", "「契約と見積りを確認」");
}

export function replaceSessionQuery(id: string) {
  const url = new URL(window.location.href);
  url.searchParams.set("session", id);
  window.history.replaceState(null, "", `${url.pathname}${url.search}${url.hash}`);
}
