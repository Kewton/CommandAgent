"use client";

import { useEffect, useState } from "react";

import type { PlanTaskExecution, PlanTaskStatus, TaskProgress } from "../lib/types";

type TrialTaskProgressProps = {
  evidenceLoading: boolean;
  onOpenEvents: () => Promise<void>;
  progress: TaskProgress | undefined;
  terminal: boolean;
};

export function TrialTaskProgress({
  evidenceLoading,
  onOpenEvents,
  progress,
  terminal,
}: TrialTaskProgressProps) {
  const normalized = progress ?? {
    status: terminal ? "unsupported" : "pending",
    executions: [],
  } satisfies TaskProgress;
  if (normalized.status === "pending") {
    return (
      <section className="trial-task-progress pending" data-testid="task-progress-pending">
        <h3>Plan タスク</h3>
        <p>typed task event を待っています。タスク件数や成功数はまだ集計しません。</p>
      </section>
    );
  }
  if (normalized.status === "unsupported") {
    return (
      <section className="trial-task-progress unsupported" data-testid="task-progress-unsupported">
        <h3>Plan タスク</h3>
        <p><strong>unsupported</strong> — このセッションには #375 の完全な typed task event がありません。</p>
        <p>不正確な成功件数は表示しません。必要な場合は events.jsonl を直接確認してください。</p>
        <button
          className="inline-action"
          disabled={evidenceLoading}
          onClick={() => void onOpenEvents()}
          type="button"
        >
          events.jsonl を確認
        </button>
      </section>
    );
  }

  const current = currentTask(normalized.executions);
  return (
    <section className="trial-task-progress supported" data-testid="task-progress">
      <header className="task-progress-heading">
        <div>
          <span>{terminal ? "terminal task results" : "live task progress"}</span>
          <h3>Plan タスク</h3>
        </div>
        <strong>{normalized.executions.length} 実行区間</strong>
      </header>
      {!terminal && current !== null && (
        <p
          aria-atomic="true"
          aria-live="polite"
          className="current-task-announcement"
          data-testid="current-task-progress"
          role="status"
        >
          現在のフェーズ: {phaseLabel(current.execution.phase_id)} / 現在のタスク: {current.task.step_id}
          {" "}（タスク {current.task.step_index} / {current.task.total_steps}）
        </p>
      )}
      {!terminal && current === null && (
        <p className="current-task-announcement" data-testid="current-task-waiting">
          次の typed task event を待っています。
        </p>
      )}
      <div className="task-execution-list">
        {normalized.executions.map((execution) => (
          <TaskExecutionGroup
            evidenceLoading={evidenceLoading}
            execution={execution}
            key={execution.plan_execution_id}
            onOpenEvents={onOpenEvents}
          />
        ))}
      </div>
    </section>
  );
}

function TaskExecutionGroup({
  evidenceLoading,
  execution,
  onOpenEvents,
}: {
  evidenceLoading: boolean;
  execution: PlanTaskExecution;
  onOpenEvents: () => Promise<void>;
}) {
  const counts = statusCounts(execution.tasks);
  const unrecorded = Math.max(0, execution.total_steps - execution.tasks.length);
  return (
    <section
      className="task-execution"
      data-plan-execution-id={execution.plan_execution_id}
      data-testid="task-execution"
    >
      <header>
        <div>
          <h4>実行区間 {execution.execution_index}</h4>
          <span>フェーズ: {phaseLabel(execution.phase_id)} · {execution.mode}</span>
        </div>
        <code title={execution.plan_execution_id}>{execution.plan_execution_id}</code>
      </header>
      <p className="task-counts" data-testid="task-status-counts">
        記録済み {execution.tasks.length} / Plan 全 {execution.total_steps} ·
        完了 {counts.completed} · short-circuited {counts.short_circuited} ·
        FAILED {counts.failed} · interrupted {counts.interrupted} · 実行中 {counts.running}
      </p>
      {unrecorded > 0 && (
        <p className="task-unrecorded" role="note">
          {unrecorded} タスクは typed event がなく、未実行か未記録かを推測しません。
        </p>
      )}
      <ol className="task-list">
        {execution.tasks.map((task) => (
          <TaskDisclosure
            evidenceLoading={evidenceLoading}
            key={task.step_execution_id}
            onOpenEvents={onOpenEvents}
            task={task}
          />
        ))}
      </ol>
    </section>
  );
}

function TaskDisclosure({
  evidenceLoading,
  onOpenEvents,
  task,
}: {
  evidenceLoading: boolean;
  onOpenEvents: () => Promise<void>;
  task: PlanTaskStatus;
}) {
  const [open, setOpen] = useState(task.status === "failed");
  useEffect(() => {
    if (task.status === "failed") setOpen(true);
  }, [task.status]);
  const presentation = statusPresentation(task.status);
  return (
    <li data-status={task.status} data-testid={`task-${task.status}`}>
      <details
        onToggle={(event) => setOpen(event.currentTarget.open)}
        open={open}
      >
        <summary aria-expanded={open}>
          <span aria-hidden="true" className="task-status-symbol">{presentation.symbol}</span>
          <span className="task-position">{task.step_index} / {task.total_steps}</span>
          <strong>{task.step_id}</strong>
          <span>{task.step_kind}</span>
          <em>{presentation.label}</em>
        </summary>
        <div className="task-detail">
          <dl>
            <div><dt>結果</dt><dd>{outcomeLabel(task)}</dd></div>
            <div><dt>検証</dt><dd>{verificationLabel(task)}</dd></div>
            <div><dt>修復試行</dt><dd>{task.repair_attempts}</dd></div>
          </dl>
          {task.failure_summary !== null && (
            <p className="task-failure-summary" data-testid="task-failure-reason">
              <strong>失敗理由:</strong> {task.failure_summary}
            </p>
          )}
          {task.verification_failures.length > 0 && (
            <div>
              <strong>検証結果</strong>
              <ul>{task.verification_failures.map((failure, index) => <li key={`${index}-${failure}`}>{failure}</li>)}</ul>
              {task.verification_failures_truncated && <small>ほかの検証失敗は events.jsonl を確認してください。</small>}
            </div>
          )}
          {task.changed_paths.length > 0 && (
            <div>
              <strong>関連パス</strong>
              <ul>{task.changed_paths.map((path) => <li key={path}><code>{path}</code></li>)}</ul>
              {task.changed_paths_truncated && <small>ほかの変更パスは events.jsonl を確認してください。</small>}
            </div>
          )}
          {task.status === "failed" && (
            <button
              className="inline-action task-evidence-link"
              data-testid="task-evidence-link"
              disabled={evidenceLoading}
              onClick={() => void onOpenEvents()}
              type="button"
            >
              events.jsonl で関連証跡を確認
            </button>
          )}
        </div>
      </details>
    </li>
  );
}

function currentTask(executions: PlanTaskExecution[]) {
  for (let executionIndex = executions.length - 1; executionIndex >= 0; executionIndex -= 1) {
    const execution = executions[executionIndex];
    for (let taskIndex = execution.tasks.length - 1; taskIndex >= 0; taskIndex -= 1) {
      const task = execution.tasks[taskIndex];
      if (task.status === "running") return { execution, task };
    }
  }
  return null;
}

function phaseLabel(value: string | null): string {
  return value === null || value.trim() === "" ? "フェーズ未指定" : value;
}

function statusCounts(tasks: PlanTaskStatus[]) {
  const counts = { completed: 0, short_circuited: 0, failed: 0, interrupted: 0, running: 0 };
  for (const task of tasks) counts[task.status] += 1;
  return counts;
}

function statusPresentation(status: PlanTaskStatus["status"]): { label: string; symbol: string } {
  switch (status) {
    case "completed": return { label: "completed（完了）", symbol: "✓" };
    case "short_circuited": return { label: "short-circuited（実行省略）", symbol: "↷" };
    case "failed": return { label: "FAILED（失敗）", symbol: "!" };
    case "interrupted": return { label: "interrupted（中断）", symbol: "■" };
    case "running": return { label: "running（実行中）", symbol: "●" };
  }
}

function outcomeLabel(task: PlanTaskStatus): string {
  return task.outcome ?? "terminal event 待ち";
}

function verificationLabel(task: PlanTaskStatus): string {
  if (task.verification_status === null) return "未記録";
  return `${task.verification_status}（失敗 ${task.verification_failure_count} 件）`;
}
