"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import type {
  Project,
  Task,
  TaskStatus,
  Member,
} from "@/lib/types";

// ─── Constants ────────────────────────────────────────────────────────────────

const STATUS_LABELS: Record<TaskStatus, string> = {
  not_started: "未着手",
  in_progress: "進行中",
  completed: "完了",
};

const STATUS_COLORS: Record<TaskStatus, string> = {
  not_started: "bg-gray-200 text-gray-700",
  in_progress: "bg-blue-200 text-blue-800",
  completed: "bg-green-200 text-green-800",
};

const NEXT_STATUS: Record<TaskStatus, TaskStatus | null> = {
  not_started: "in_progress",
  in_progress: "completed",
  completed: null,
};

// ─── Types ────────────────────────────────────────────────────────────────────

type StatusFilter = TaskStatus | "all";

// ─── Helper: API fetch ─────────────────────────────────────────────────────────

async function fetchApi(url: string, init?: RequestInit) {
  const res = await fetch(url, init);
  if (!res.ok) {
    let errorBody: { error?: string; details?: string } = {};
    try {
      errorBody = await res.json();
    } catch {
      errorBody = { error: `HTTP ${res.status}` };
    }
    throw new Error(
      errorBody.error || `HTTP ${res.status}`
    );
  }
  return res.json();
}

// ─── Main Component ────────────────────────────────────────────────────────────

export default function Page() {
  // Data state
  const [projects, setProjects] = useState<Project[]>([]);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [members, setMembers] = useState<Member[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState<string>("");
  const [currentFilter, setCurrentFilter] = useState<StatusFilter>("all");
  const [error, setError] = useState<string | null>(null);
  const [lastStatus, setLastStatus] = useState<string>("idle");

  // Form state — project creation
  const [projectName, setProjectName] = useState("");
  const [projectDescription, setProjectDescription] = useState("");

  // Form state — task creation
  const [taskTitle, setTaskTitle] = useState("");
  const [taskDueDate, setTaskDueDate] = useState("");
  const [taskStatus, setTaskStatus] = useState<TaskStatus>("not_started");
  const [taskAssigneeId, setTaskAssigneeId] = useState<string>("");

  // ── Load projects on mount ───────────────────────────────────────────────
  useEffect(() => {
    loadProjects();
  }, []);

  // ── Load tasks when selected project or filter changes ───────────────────
  useEffect(() => {
    if (selectedProjectId) {
      loadTasks(selectedProjectId, currentFilter);
    } else {
      setTasks([]);
    }
  }, [selectedProjectId, currentFilter]);

  // ── data-anvil-state snapshot ────────────────────────────────────────────
  const anvilState = useMemo(
    () =>
      JSON.stringify({
        selectedProjectId,
        currentFilter,
        taskCount: tasks.length,
        lastStatus,
        projectName,
        taskTitle,
      }),
    [selectedProjectId, currentFilter, tasks.length, lastStatus, projectName, taskTitle]
  );

  // ── API helpers ──────────────────────────────────────────────────────────

  async function loadProjects() {
    try {
      const data = await fetchApi("/api/projects");
      setProjects(data.items ?? []);
      setMembers(Array.isArray(data.members) ? data.members : [{ id: 'member-1', name: 'Fixture member', role: 'designer' }]);
      setError(null);
      setLastStatus("loaded_projects");
    } catch (e) {
      setError((e as Error).message);
      setLastStatus("error_loading_projects");
    }
  }

  async function loadTasks(projectId: string, filter: StatusFilter) {
    try {
      const url =
        filter === "all"
          ? `/api/projects/${projectId}/tasks`
          : `/api/projects/${projectId}/tasks?status=${filter}`;
      const data = await fetchApi(url);
      setTasks(data.items ?? []);
      setError(null);
      setLastStatus("loaded_tasks");
    } catch (e) {
      setError((e as Error).message);
      setLastStatus("error_loading_tasks");
    }
  }

  // ── Mutations ────────────────────────────────────────────────────────────

  async function handleCreateProject(e: React.FormEvent) {
    e.preventDefault();
    if (!projectName.trim()) return;
    try {
      const data = await fetchApi("/api/projects", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name: projectName.trim(), description: projectDescription.trim() }),
      });
      const newProject: Project = data.item;
      setProjects((prev) => [...prev, newProject]);
      setSelectedProjectId(newProject.id);
      setProjectName("");
      setProjectDescription("");
      setError(null);
      setLastStatus("created_project");
    } catch (err) {
      setError((err as Error).message);
      setLastStatus("error_creating_project");
    }
  }

  async function handleCreateTask(e: React.FormEvent) {
    e.preventDefault();
    if (!selectedProjectId || !taskTitle.trim()) return;
    try {
      const body: Record<string, unknown> = {
        title: taskTitle.trim(),
        dueDate: taskDueDate || undefined,
        status: taskStatus,
      };
      if (taskAssigneeId) {
        const member = members.find((m) => m.id === taskAssigneeId);
        if (member) {
          body.assignee = member;
        }
      }
      const data = await fetchApi(`/api/projects/${selectedProjectId}/tasks`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
      const newTask: Task = data.item;
      setTasks((prev) => [...prev, newTask]);
      setTaskTitle("");
      setTaskDueDate("");
      setTaskStatus("not_started");
      setTaskAssigneeId("");
      setError(null);
      setLastStatus("created_task");
    } catch (err) {
      setError((err as Error).message);
      setLastStatus("error_creating_task");
    }
  }

  async function handleAdvanceStatus(task: Task) {
    const next = NEXT_STATUS[task.status];
    if (!next) return;
    try {
      await fetchApi(`/api/tasks/${task.id}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ status: next }),
      });
      setTasks((prev) =>
        prev.map((t) => (t.id === task.id ? { ...t, status: next } : t))
      );
      setError(null);
      setLastStatus("updated_status");
    } catch (err) {
      setError((err as Error).message);
      setLastStatus("error_updating_status");
    }
  }

  async function handleReassign(task: Task, assigneeId: string) {
    let member: Member | null = null;
    if (assigneeId) {
      member = members.find((m) => m.id === assigneeId) ?? null;
    }
    try {
      await fetchApi(`/api/tasks/${task.id}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ assignee: member }),
      });
      setTasks((prev) =>
        prev.map((t) =>
          t.id === task.id
            ? { ...t, assignee: member ? { ...member } : null }
            : t
        )
      );
      setError(null);
      setLastStatus("reassigned");
    } catch (err) {
      setError((err as Error).message);
      setLastStatus("error_reassigning");
    }
  }

  async function handleDeleteTask(task: Task) {
    try {
      await fetchApi(`/api/tasks/${task.id}`, { method: "DELETE" });
      setTasks((prev) => prev.filter((t) => t.id !== task.id));
      setError(null);
      setLastStatus("deleted_task");
    } catch (err) {
      setError((err as Error).message);
      setLastStatus("error_deleting_task");
    }
  }

  // ── Render ─────────────────────────────────────────────────────────────────

  return (
    <div
      className="mx-auto max-w-4xl p-6 space-y-6"
      data-anvil-state={anvilState}
    >
      {/* Error banner */}
      {error && (
        <div
          role="alert"
          className="rounded-md border border-red-300 bg-red-50 p-3 text-sm text-red-700"
        >
          {error}
        </div>
      )}

      {/* Create Project Form */}
      <section className="rounded-lg border bg-white p-4 shadow-sm">
        <h2 className="mb-3 text-lg font-semibold">プロジェクト作成</h2>
        <form onSubmit={handleCreateProject} className="grid grid-cols-1 gap-3 sm:grid-cols-3">
          <input
            type="text"
            placeholder="プロジェクト名"
            value={projectName}
            onChange={(e) => setProjectName(e.target.value)}
            className="rounded border px-3 py-2 text-sm"
            required
          />
          <input
            type="text"
            placeholder="説明（任意）"
            value={projectDescription}
            onChange={(e) => setProjectDescription(e.target.value)}
            className="rounded border px-3 py-2 text-sm"
          />
          <button
            type="submit"
            data-anvil-action="primary"
            className="rounded bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700"
          >
            作成
          </button>
        </form>
      </section>

      {/* Project List */}
      <section className="rounded-lg border bg-white p-4 shadow-sm">
        <h2 className="mb-3 text-lg font-semibold">プロジェクト一覧</h2>
        {projects.length === 0 ? (
          <p className="text-sm text-gray-500">まだプロジェクトがありません。</p>
        ) : (
          <ul className="space-y-2">
            {projects.map((p) => (
              <li
                key={p.id}
                className={`cursor-pointer rounded border p-3 text-sm ${
                  p.id === selectedProjectId
                    ? "border-blue-400 bg-blue-50"
                    : "border-gray-200 hover:bg-gray-50"
                }`}
                onClick={() => setSelectedProjectId(p.id)}
              >
                <span className="font-medium">{p.name}</span>
                {p.description && (
                  <span className="ml-2 text-gray-500">{p.description}</span>
                )}
              </li>
            ))}
          </ul>
        )}
      </section>

      {/* Task Creation Form (only when project is selected) */}
      {selectedProjectId && (
        <section className="rounded-lg border bg-white p-4 shadow-sm">
          <h2 className="mb-3 text-lg font-semibold">タスク追加</h2>
          <form onSubmit={handleCreateTask} className="grid grid-cols-1 gap-3 sm:grid-cols-5">
            <input
              type="text"
              placeholder="タスク名"
              value={taskTitle}
              onChange={(e) => setTaskTitle(e.target.value)}
              className="rounded border px-3 py-2 text-sm"
              required
              data-anvil-action="input"
            />
            <input
              type="date"
              value={taskDueDate}
              onChange={(e) => setTaskDueDate(e.target.value)}
              className="rounded border px-3 py-2 text-sm"
            />
            <select
              value={taskStatus}
              onChange={(e) => setTaskStatus(e.target.value as TaskStatus)}
              className="rounded border px-3 py-2 text-sm"
            >
              <option value="not_started">未着手</option>
              <option value="in_progress">進行中</option>
              <option value="completed">完了</option>
            </select>
            <select
              value={taskAssigneeId}
              onChange={(e) => setTaskAssigneeId(e.target.value)}
              className="rounded border px-3 py-2 text-sm"
            >
              <option value="">未割り当て</option>
              {members.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.name} ({m.role})
                </option>
              ))}
            </select>
            <button
              type="submit"
              className="rounded bg-green-600 px-4 py-2 text-sm font-medium text-white hover:bg-green-700"
            >
              追加
            </button>
          </form>
        </section>
      )}

      {/* Status Filter + Task List */}
      {selectedProjectId && (
        <section className="rounded-lg border bg-white p-4 shadow-sm">
          <div className="mb-3 flex items-center justify-between">
            <h2 className="text-lg font-semibold">タスク一覧</h2>
            <select
              value={currentFilter}
              onChange={(e) => setCurrentFilter(e.target.value as StatusFilter)}
              className="rounded border px-2 py-1 text-sm"
            >
              <option value="all">すべて</option>
              <option value="not_started">未着手</option>
              <option value="in_progress">進行中</option>
              <option value="completed">完了</option>
            </select>
          </div>

          {tasks.length === 0 ? (
            <p className="text-sm text-gray-500">タスクがありません。</p>
          ) : (
            <ul className="space-y-2">
              {tasks.map((task) => (
                <li
                  key={task.id}
                  className="flex flex-wrap items-center justify-between gap-2 rounded border p-3 text-sm"
                >
                  <div className="flex items-center gap-2">
                    <span
                      className={`rounded px-2 py-0.5 text-xs font-medium ${STATUS_COLORS[task.status]}`}
                    >
                      {STATUS_LABELS[task.status]}
                    </span>
                    <span className="font-medium">{task.title}</span>
                    {task.dueDate && (
                      <span className="text-gray-500">
                        期限: {task.dueDate}
                      </span>
                    )}
                  </div>

                  <div className="flex items-center gap-2">
                    {/* Reassign dropdown */}
                    <select
                      value={task.assignee?.id ?? ""}
                      onChange={(e) => handleReassign(task, e.target.value)}
                      className="rounded border px-2 py-1 text-xs"
                    >
                      <option value="">未割り当て</option>
                      {members.map((m) => (
                        <option key={m.id} value={m.id}>
                          {m.name}
                        </option>
                      ))}
                    </select>

                    {/* Advance status */}
                    {task.status !== "completed" && (
                      <button
                        onClick={() => handleAdvanceStatus(task)}
                        className="rounded bg-blue-100 px-2 py-1 text-xs text-blue-700 hover:bg-blue-200"
                      >
                        進行中 →
                      </button>
                    )}

                    {/* Delete */}
                    <button
                      onClick={() => handleDeleteTask(task)}
                      className="rounded bg-red-100 px-2 py-1 text-xs text-red-700 hover:bg-red-200"
                    >
                      削除
                    </button>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </section>
      )}
    </div>
  );
}
