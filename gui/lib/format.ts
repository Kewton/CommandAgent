const DATE_TIME_FORMATTER = new Intl.DateTimeFormat("ja-JP", {
  dateStyle: "medium",
  timeStyle: "short",
});

const TRIAL_GATE_LABELS: Readonly<Record<string, string>> = {
  gate_1: "Gate 1（実行前確認）",
  gate_2: "Gate 2（実行）",
  gate_3: "Gate 3（完了）",
  gate_4: "Gate 4（要対応）",
};

const TRIAL_STATUS_LABELS: Readonly<Record<string, string>> = {
  aborted: "中止",
  completed: "完了",
  failed: "失敗",
  incomplete: "未完了",
  interrupted: "中断",
  pending: "待機中",
  running: "実行中",
  starting: "開始中",
  unreadable: "読み取り不可",
};

const PHASE_STATUS_LABELS: Readonly<Record<string, string>> = {
  aborted: "中止",
  completed: "完了",
  failed: "失敗",
  interrupted: "中断",
  pending: "待機中",
  running: "実行中",
};

const PHASE_STAGE_LABELS: Readonly<Record<string, string>> = {
  complete: "完了",
  execute: "実装中",
  lint: "計画を確認中",
  phase_verification_result: "検証中",
  profile_invariant: "プロファイルを検証中",
  profile_observed: "プロファイルを確認中",
  queued: "待機中",
  recovery_prompt_saved: "復旧手順を準備済み",
  scaffold: "計画中",
  start: "開始準備中",
  ultra_phase_context_attached: "実行条件を準備中",
  ultra_phase_context_updated: "実行条件を更新中",
  ultra_phase_execute_complete: "実装完了",
  ultra_phase_plan_validated: "計画確認済み",
  ultra_phase_profile_check: "プロファイルを検証中",
  ultra_phase_scaffold_complete: "計画済み",
  ultra_phase_start: "開始準備中",
};

export function dateTimeLabel(value: number | string | Date, unavailable: string): string {
  if (typeof value === "number" && value <= 0) return unavailable;
  const date = typeof value === "number" ? new Date(value * 1_000) : new Date(value);
  if (Number.isNaN(date.getTime())) return unavailable;
  return DATE_TIME_FORMATTER.format(date);
}

export function byteLabel(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  return `${(bytes / 1024).toFixed(1)} KiB`;
}

export function elapsedLabel(totalSeconds: number): string {
  const hours = Math.floor(totalSeconds / 3_600);
  const minutes = Math.floor((totalSeconds % 3_600) / 60);
  const seconds = totalSeconds % 60;
  return [hours, minutes, seconds].map((part) => String(part).padStart(2, "0")).join(":");
}

export function repositoryRunStatusLabel(state: string, statusText: string): string {
  if (state === "pass") return "成功";
  if (state === "fail") return "失敗";
  if (state === "pending") {
    return normalizedEnumValue(statusText) === "recorded" ? "記録あり" : "進行中";
  }
  return normalizedEnumValue(statusText) === "not_recorded" ? "未記録" : "判定不能";
}

export function trialGateLabel(value: string | null | undefined): string {
  if (value === null || value === undefined || value.trim() === "") return "Gate 未確定";
  return TRIAL_GATE_LABELS[normalizedEnumValue(value)] ?? "Gate 不明";
}

export function trialStatusLabel(value: string | null | undefined): string {
  return enumLabel(value, TRIAL_STATUS_LABELS, "状態不明");
}

export function phaseStatusLabel(value: string | null | undefined): string {
  return enumLabel(value, PHASE_STATUS_LABELS, "状態不明");
}

export function phaseStageLabel(value: string | null | undefined): string {
  return enumLabel(value, PHASE_STAGE_LABELS, "段階不明");
}

function enumLabel(
  value: string | null | undefined,
  labels: Readonly<Record<string, string>>,
  fallback: string,
): string {
  if (value === null || value === undefined) return fallback;
  return labels[normalizedEnumValue(value)] ?? fallback;
}

function normalizedEnumValue(value: string): string {
  return value.trim().toLowerCase().replace(/[\s-]+/g, "_");
}
