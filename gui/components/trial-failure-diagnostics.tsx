import type { FailureDiagnostics } from "../lib/types";

type TrialFailureDiagnosticsProps = {
  diagnostics: FailureDiagnostics | null | undefined;
  fallbackStopReason?: string | null;
  testId: string;
};

export function TrialFailureDiagnostics({
  diagnostics,
  fallbackStopReason = null,
  testId,
}: TrialFailureDiagnosticsProps) {
  const stopReason = diagnostics?.stop_reason ?? fallbackStopReason;
  const releaseReasons = diagnostics?.release_gate_reasons ?? [];
  const probeFindings = diagnostics?.probe_findings ?? [];
  const hasDetails = hasFailureDiagnostics(diagnostics, fallbackStopReason);

  return (
    <section className="trial-failure-diagnostics" data-testid={testId}>
      <h3>FAILED の原因</h3>
      {!hasDetails && (
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
          <strong>プローブ所見</strong>
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
    (diagnostics?.probe_findings.length ?? 0) > 0;
}

function nonEmpty(value: string | null | undefined): value is string {
  return value !== null && value !== undefined && value.trim() !== "" && value !== "completed";
}

function firstLine(value: string): string {
  return value.split("\n", 1)[0];
}
