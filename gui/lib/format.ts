export function byteLabel(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  return `${(bytes / 1024).toFixed(1)} KiB`;
}

export function dateLabel(epochSeconds: number, unavailable: string): string {
  if (epochSeconds === 0) return unavailable;
  return new Intl.DateTimeFormat("ja-JP", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(epochSeconds * 1_000));
}

export function elapsedLabel(totalSeconds: number): string {
  const hours = Math.floor(totalSeconds / 3_600);
  const minutes = Math.floor((totalSeconds % 3_600) / 60);
  const seconds = totalSeconds % 60;
  return [hours, minutes, seconds].map((part) => String(part).padStart(2, "0")).join(":");
}

export function lastSuccessLabel(value: string | null): string {
  if (value === null) return "未接続";
  return new Intl.DateTimeFormat("ja-JP", {
    dateStyle: "medium",
    timeStyle: "medium",
  }).format(new Date(value));
}

export function sessionTimeLabel(epochSeconds: number): string {
  if (epochSeconds <= 0) return "反映待ち";
  return new Date(epochSeconds * 1_000).toISOString();
}
