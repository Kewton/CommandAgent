"use client";

import { useState, useCallback } from "react";

// ─── Types ────────────────────────────────────────────────────────────────────
type TaskStatus = "not_started" | "in_progress" | "completed";

interface TeamMember {
  id: string;
  name: string;
  role: string;
}

interface Task {
  id: string;
  title: string;
  dueDate: string;
  status: TaskStatus;
  assignedTo: string[];
  projectId: string;
}

interface Project {
  id: string;
  name: string;
  createdAt: string;
}

type StatusFilter = "all" | TaskStatus;

// ─── Constants ────────────────────────────────────────────────────────────────
const PREDEFINED_MEMBERS: TeamMember[] = [
  { id: "m1", name: "田中 花子", role: "デザイナー" },
  { id: "m2", name: "佐藤 太郎", role: "開発" },
  { id: "m3", name: "鈴木 次郎", role: "PM" },
  { id: "m4", name: "高橋 美咲", role: "営業" },
];

const STATUS_LABELS: Record<TaskStatus, string> = {
  not_started: "未着手",
  in_progress: "進行中",
  completed: "完了",
};

const STATUS_COLORS: Record<TaskStatus, string> = {
  not_started: "bg-gray-200 text-gray-700",
  in_progress: "bg-blue-100 text-blue-700",
  completed: "bg-green-100 text-green-700",
};

// ─── Helpers ──────────────────────────────────────────────────────────────────
let idCounter = 0;
function nextId(prefix: string): string {
  idCounter += 1;
  return `${prefix}-${Date.now()}-${idCounter}`;
}

// ─── Main Page Component ──────────────────────────────────────────────────────
export default function Page() {
  // State
  const [projects, setProjects] = useState<Project[]>([]);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [activeFilter, setActiveFilter] = useState<StatusFilter>("all");
  const [selectedMembers, setSelectedMembers] = useState<string[]>([]);

  // Form inputs
  const [projectName, setProjectName] = useState("");
  const [taskTitle, setTaskTitle] = useState("");
  const [taskDueDate, setTaskDueDate] = useState("");
  const [selectedProjectId, setSelectedProjectId] = useState("");
  const [taskStatus, setTaskStatus] = useState<TaskStatus>("not_started");
  const [message, setMessage] = useState<string | null>(null);

  // ── Create Project ────────────────────────────────────────────────────────
  const handleCreateProject = useCallback(() => {
    const trimmed = projectName.trim();
    if (!trimmed) {
      setMessage("プロジェクト名を入力してください。");
      return;
    }
    const newProject: Project = {
      id: nextId("proj"),
      name: trimmed,
      createdAt: new Date().toISOString(),
    };
    setProjects((prev) => [...prev, newProject]);
    setProjectName("");
    setMessage(`プロジェクト「${trimmed}」を作成しました。`);
  }, [projectName]);

  // ── Add Task ──────────────────────────────────────────────────────────────
  const handleAddTask = useCallback(() => {
    const trimmedTitle = taskTitle.trim();
    if (!trimmedTitle) {
      setMessage("タスク名を入力してください。");
      return;
    }
    if (!taskDueDate) {
      setMessage("期限日を入力してください。");
      return;
    }
    if (!selectedProjectId) {
      setMessage("プロジェクトを選択してください。");
      return;
    }
    const newTask: Task = {
      id: nextId("task"),
      title: trimmedTitle,
      dueDate: taskDueDate,
      status: taskStatus,
      assignedTo: [...selectedMembers],
      projectId: selectedProjectId,
    };
    setTasks((prev) => [...prev, newTask]);
    setTaskTitle("");
    setTaskDueDate("");
    setSelectedMembers([]);
    setMessage(`タスク「${trimmedTitle}」を追加しました。`);
  }, [taskTitle, taskDueDate, selectedProjectId, taskStatus, selectedMembers]);

  // ── Toggle Member Assignment ──────────────────────────────────────────────
  const toggleMember = useCallback((memberId: string) => {
    setSelectedMembers((prev) =>
      prev.includes(memberId)
        ? prev.filter((id) => id !== memberId)
        : [...prev, memberId]
    );
  }, []);

  // ── Change Task Status ────────────────────────────────────────────────────
  const handleStatusChange = useCallback(
    (taskId: string, newStatus: TaskStatus) => {
      setTasks((prev) =>
        prev.map((t) => (t.id === taskId ? { ...t, status: newStatus } : t))
      );
      setMessage(`タスクのステータスを「${STATUS_LABELS[newStatus]}」に変更しました。`);
    },
    []
  );

  // ── Delete Task ───────────────────────────────────────────────────────────
  const handleDeleteTask = useCallback((taskId: string) => {
    setTasks((prev) => prev.filter((t) => t.id !== taskId));
    setMessage("タスクを削除しました。");
  }, []);

  // ── Restart / Reset ───────────────────────────────────────────────────────
  const handleRestart = useCallback(() => {
    setProjects([]);
    setTasks([]);
    setActiveFilter("all");
    setSelectedMembers([]);
    setProjectName("");
    setTaskTitle("");
    setTaskDueDate("");
    setSelectedProjectId("");
    setTaskStatus("not_started");
    setMessage("すべてをリセットしました。");
  }, []);

  // ── Derived: filtered tasks ───────────────────────────────────────────────
  const filteredTasks =
    activeFilter === "all"
      ? tasks
      : tasks.filter((t) => t.status === activeFilter);

  // ── State snapshot for data-anvil-state ───────────────────────────────────
  const stateSnapshot = JSON.stringify({
    projects: projects.map((p) => ({ id: p.id, name: p.name })),
    tasks: tasks.map((t) => ({
      id: t.id,
      title: t.title,
      status: t.status,
      assignedTo: t.assignedTo,
      projectId: t.projectId,
    })),
    activeFilter,
    assignedMembers: selectedMembers,
    message,
  });

  return (
    <main
      data-anvil-state={stateSnapshot}
      className="min-h-screen bg-gray-50 p-6 max-w-4xl mx-auto"
    >
      <h1 className="text-2xl font-bold mb-6">
        プロジェクト管理ダッシュボード
      </h1>

      {/* ── Message / Notification ──────────────────────────────────────────── */}
      {message && (
        <div className="mb-4 p-3 bg-blue-50 border border-blue-200 rounded text-blue-800 text-sm">
          {message}
        </div>
      )}

      {/* ── Create Project ──────────────────────────────────────────────────── */}
      <section className="mb-6 bg-white p-4 rounded shadow">
        <h2 className="text-lg font-semibold mb-3">新規プロジェクト作成</h2>
        <div className="flex gap-2">
          <input
            type="text"
            placeholder="プロジェクト名を入力"
            value={projectName}
            onChange={(e) => setProjectName(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleCreateProject()}
            data-anvil-action="input"
            aria-label="プロジェクト名"
            className="flex-1 px-3 py-2 border rounded focus:outline-none focus:ring-2 focus:ring-blue-400"
          />
          <button
            onClick={handleCreateProject}
            data-anvil-action="primary"
            className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 transition-colors"
          >
            作成
          </button>
        </div>
      </section>

      {/* ── Project List ────────────────────────────────────────────────────── */}
      {projects.length > 0 && (
        <section className="mb-6 bg-white p-4 rounded shadow">
          <h2 className="text-lg font-semibold mb-3">
            プロジェクト一覧（{projects.length}件）
          </h2>
          <ul className="space-y-1">
            {projects.map((p) => (
              <li key={p.id} className="text-sm text-gray-700">
                📁 {p.name}
                <span className="ml-2 text-gray-400">
                  {tasks.filter((t) => t.projectId === p.id).length} タスク
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}

      {/* ── Add Task ────────────────────────────────────────────────────────── */}
      <section className="mb-6 bg-white p-4 rounded shadow">
        <h2 className="text-lg font-semibold mb-3">タスクを追加</h2>

        {/* Project selector */}
        <div className="mb-3">
          <label className="block text-sm font-medium mb-1">
            対象プロジェクト
          </label>
          <select
            value={selectedProjectId}
            onChange={(e) => setSelectedProjectId(e.target.value)}
            className="w-full px-3 py-2 border rounded focus:outline-none focus:ring-2 focus:ring-blue-400"
          >
            <option value="">— 選択してください —</option>
            {projects.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        </div>

        {/* Task title + due date */}
        <div className="flex gap-2 mb-3">
          <input
            type="text"
            placeholder="タスク名"
            value={taskTitle}
            onChange={(e) => setTaskTitle(e.target.value)}
            className="flex-1 px-3 py-2 border rounded focus:outline-none focus:ring-2 focus:ring-blue-400"
          />
          <input
            type="date"
            value={taskDueDate}
            onChange={(e) => setTaskDueDate(e.target.value)}
            className="px-3 py-2 border rounded focus:outline-none focus:ring-2 focus:ring-blue-400"
          />
        </div>

        {/* Status selector for new task */}
        <div className="mb-3">
          <label className="block text-sm font-medium mb-1">初期ステータス</label>
          <select
            value={taskStatus}
            onChange={(e) => setTaskStatus(e.target.value as TaskStatus)}
            className="w-full px-3 py-2 border rounded focus:outline-none focus:ring-2 focus:ring-blue-400"
          >
            {Object.entries(STATUS_LABELS).map(([val, label]) => (
              <option key={val} value={val}>
                {label}
              </option>
            ))}
          </select>
        </div>

        {/* Member assignment */}
        <div className="mb-3">
          <label className="block text-sm font-medium mb-1">
            チームメンバー割り当て
          </label>
          <div className="flex flex-wrap gap-2">
            {PREDEFINED_MEMBERS.map((m) => {
              const checked = selectedMembers.includes(m.id);
              return (
                <button
                  key={m.id}
                  onClick={() => toggleMember(m.id)}
                  data-anvil-action={checked ? "restart" : "primary"}
                  className={`px-3 py-1 rounded-full text-xs border transition-colors ${
                    checked
                      ? "bg-blue-600 text-white border-blue-600"
                      : "bg-white text-gray-600 border-gray-300 hover:border-blue-400"
                  }`}
                >
                  {m.name}（{m.role}）
                </button>
              );
            })}
          </div>
        </div>

        <button
          onClick={handleAddTask}
          data-anvil-action="primary"
          className="px-4 py-2 bg-green-600 text-white rounded hover:bg-green-700 transition-colors w-full"
        >
          タスクを追加
        </button>
      </section>

      {/* ── Status Filter ───────────────────────────────────────────────────── */}
      <section className="mb-4 bg-white p-4 rounded shadow">
        <h2 className="text-lg font-semibold mb-3">タスクフィルタ</h2>
        <div className="flex gap-2 flex-wrap">
          {(["all", "not_started", "in_progress", "completed"] as StatusFilter[]).map(
            (f) => (
              <button
                key={f}
                onClick={() => setActiveFilter(f)}
                data-anvil-action={f !== "all" ? "restart" : "primary"}
                className={`px-3 py-1.5 rounded-full text-sm border transition-colors ${
                  activeFilter === f
                    ? "bg-indigo-600 text-white border-indigo-600"
                    : "bg-white text-gray-600 border-gray-300 hover:border-indigo-400"
                }`}
              >
                {f === "all" ? "すべて" : STATUS_LABELS[f]}
              </button>
            )
          )}
        </div>
      </section>

      {/* ── Task List ───────────────────────────────────────────────────────── */}
      <section className="mb-6 bg-white p-4 rounded shadow">
        <h2 className="text-lg font-semibold mb-3">
          タスク一覧（{filteredTasks.length}件）
        </h2>

        {filteredTasks.length === 0 ? (
          <p className="text-sm text-gray-400">
            {activeFilter === "all"
              ? "タスクがありません。上記から追加してください。"
              : `${STATUS_LABELS[activeFilter as TaskStatus]}のタスクはありません。`}
          </p>
        ) : (
          <ul className="space-y-2">
            {filteredTasks.map((task) => {
              const project = projects.find((p) => p.id === task.projectId);
              return (
                <li
                  key={task.id}
                  className="border border-gray-200 rounded p-3 flex flex-col gap-2"
                >
                  <div className="flex items-center justify-between">
                    <span className="font-medium">{task.title}</span>
                    <span
                      className={`text-xs px-2 py-0.5 rounded ${STATUS_COLORS[task.status]}`}
                    >
                      {STATUS_LABELS[task.status]}
                    </span>
                  </div>
                  <div className="text-xs text-gray-500">
                    📅 期限: {task.dueDate} | 📁 {project?.name ?? "不明"}
                  </div>
                  {task.assignedTo.length > 0 && (
                    <div className="text-xs text-gray-600">
                      👤{" "}
                      {task.assignedTo
                        .map(
                          (mid) =>
                            PREDEFINED_MEMBERS.find((m) => m.id === mid)?.name
                        )
                        .filter(Boolean)
                        .join(", ")}
                    </div>
                  )}
                  <div className="flex items-center gap-2">
                    <select
                      value={task.status}
                      onChange={(e) =>
                        handleStatusChange(
                          task.id,
                          e.target.value as TaskStatus
                        )
                      }
                      className="text-xs px-2 py-1 border rounded"
                    >
                      {Object.entries(STATUS_LABELS).map(([val, label]) => (
                        <option key={val} value={val}>
                          {label}
                        </option>
                      ))}
                    </select>
                    <button
                      onClick={() => handleDeleteTask(task.id)}
                      data-anvil-action="restart"
                      className="text-xs text-red-500 hover:text-red-700"
                    >
                      削除
                    </button>
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </section>

      {/* ── Restart / Reset All ─────────────────────────────────────────────── */}
      <section className="text-center">
        <button
          onClick={handleRestart}
          data-anvil-action="restart"
          className="px-6 py-2 bg-gray-700 text-white rounded hover:bg-gray-800 transition-colors"
        >
          全データリセット
        </button>
      </section>
    </main>
  );
}
