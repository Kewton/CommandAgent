import type { FailureDiagnostics } from "../lib/types";

type TrialFailureDiagnosticsProps = {
  diagnostics: FailureDiagnostics | null | undefined;
  fallbackStopReason?: string | null;
  mode?: "failure" | "verification";
  testId: string;
};

export function TrialFailureDiagnostics({
  diagnostics,
  fallbackStopReason = null,
  mode = "failure",
  testId,
}: TrialFailureDiagnosticsProps) {
  const stopReason = diagnostics?.stop_reason ?? fallbackStopReason;
  const releaseReasons = diagnostics?.release_gate_reasons ?? [];
  const probeFindings = diagnostics?.probe_findings ?? [];
  const hasDetails = hasFailureDiagnostics(diagnostics, fallbackStopReason);

  return (
    <section className="trial-failure-diagnostics" data-testid={testId}>
      <h3>{mode === "failure" ? "FAILED の原因" : "検証結果"}</h3>
      {mode === "failure" && !hasDetails && (
        <p>
          構造化された原因はこの応答にありません。下の events.jsonl、summary.md、受入シートを確認してください。
        </p>
      )}
      {nonEmpty(stopReason) && (
        <div>
          <strong>停止理由</strong>
          <code>{firstLine(stopReason)}</code>
        </div>
      )}
      {releaseReasons.length > 0 && (
        <div>
          <strong>リリースゲート理由</strong>
          <ul>{releaseReasons.map((reason) => <li key={reason}><code>{reason}</code></li>)}</ul>
        </div>
      )}
      {probeFindings.length > 0 && (
        <div>
          <strong>{mode === "failure" ? "プローブ所見" : "プローブ結果"}</strong>
          <ul>
            {probeFindings.map((finding) => (
              <li key={finding.name}>
                <code>{finding.name}</code>
                {nonEmpty(finding.status) && <span> — {finding.status}</span>}
                {finding.reasons.length > 0 && <small>{finding.reasons.join(" / ")}</small>}
                {nonEmpty(finding.evidence_path) && <small>証跡: {finding.evidence_path}</small>}
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}

export function hasFailureDiagnostics(
  diagnostics: FailureDiagnostics | null | undefined,
  fallbackStopReason: string | null | undefined = null,
): boolean {
  return nonEmpty(diagnostics?.stop_reason ?? fallbackStopReason) ||
    (diagnostics?.release_gate_reasons.length ?? 0) > 0 ||
    (diagnostics?.probe_findings.some(isBlockingFinding) ?? false);
}

export function hasVerificationResults(
  diagnostics: FailureDiagnostics | null | undefined,
): boolean {
  return diagnostics?.probe_findings.some((finding) =>
    nonEmpty(finding.status) && isSuccessfulStatus(finding.status)
  ) ?? false;
}

function nonEmpty(value: string | null | undefined): value is string {
  return value !== null && value !== undefined && value.trim() !== "" && value !== "completed";
}

function firstLine(value: string): string {
  return value.split("\n", 1)[0];
}

function isBlockingFinding(
  finding: FailureDiagnostics["probe_findings"][number],
): boolean {
  if (finding.status === null || finding.status.trim() === "") {
    return finding.reasons.length > 0 || nonEmpty(finding.evidence_path);
  }
  return !isSuccessfulStatus(finding.status) && !isNotApplicableStatus(finding.status);
}

function isSuccessfulStatus(status: string): boolean {
  return ["ok", "pass", "passed", "ready", "completed", "full", "full_success"]
    .includes(status.trim().toLowerCase());
}

function isNotApplicableStatus(status: string): boolean {
  return ["not_applicable", "not_required"].includes(status.trim().toLowerCase());
}
