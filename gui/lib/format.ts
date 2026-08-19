const DATE_TIME_FORMATTER = new Intl.DateTimeFormat("ja-JP", {
  dateStyle: "medium",
  timeStyle: "short",
});

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
