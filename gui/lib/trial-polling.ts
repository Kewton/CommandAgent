export const CHANGED_POLL_INTERVAL_MS = 1_000;
export const MAX_UNCHANGED_POLL_INTERVAL_MS = 10_000;

export function unchangedPollDelay(unchangedResponses: number): number {
  const exponent = Math.max(0, Math.min(unchangedResponses, 10));
  return Math.min(CHANGED_POLL_INTERVAL_MS * 2 ** exponent, MAX_UNCHANGED_POLL_INTERVAL_MS);
}
