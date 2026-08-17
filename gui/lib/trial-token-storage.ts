import { guiBasePath } from "./base-path";

const TRIAL_TOKEN_STORAGE_NAMESPACE = "commandagent.gui.trial-token";

export function trialTokenStorageKey(): string {
  return `${TRIAL_TOKEN_STORAGE_NAMESPACE}:${guiBasePath() || "/"}`;
}

export function restoreTrialToken(): string {
  try {
    return window.sessionStorage.getItem(trialTokenStorageKey()) ?? "";
  } catch {
    return "";
  }
}

export function persistTrialToken(value: string): void {
  try {
    if (value === "") {
      window.sessionStorage.removeItem(trialTokenStorageKey());
    } else {
      window.sessionStorage.setItem(trialTokenStorageKey(), value);
    }
  } catch {
    // Keep the token in React memory when Web Storage is unavailable.
  }
}

export function removeRejectedTrialToken(rejectedValue: string): void {
  try {
    const key = trialTokenStorageKey();
    const storedValue = window.sessionStorage.getItem(key);
    if (storedValue?.trim() === rejectedValue.trim()) {
      window.sessionStorage.removeItem(key);
    }
  } catch {
    // The caller still clears a matching in-memory value.
  }
}
