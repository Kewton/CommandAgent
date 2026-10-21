"use client";

import { useMemo, useState } from "react";

type Status = "未着手" | "進行中" | "完了";
type TaskFilter = "all" | Status;

interface Project {
  id: string;
  name: string;
}

interface Task {
  id: string;
  projectId: string;
  name: string;
  deadline: string;
  assignee: string;
  status: Status;
}

const STATUS_ORDER: Status[] = ["未着手", "進行中", "完了"];
const STATUS_STYLES: Record<Status, string> = {
  未着手: "bg-gray-100 text-gray-700",
  進行中: "bg-blue-100 text-blue-700",
  完了: "bg-green-100 text-green-700",
};

function nextStatus(s: Status): Status {
  const idx = STATUS_ORDER.indexOf(s);
  return STATUS_ORDER[(idx + 1) % STATUS_ORDER.length];
}

export default function Page() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [filter, setFilter] = useState<TaskFilter>("all");
  const [selectedProjectId, setSelectedProjectId] = useState<string>("");
  const [lastAction, setLastAction] = useState<string>("init");
  const [viewMode] = useState<"active">("active");

  // Project creation form
  const [projectName, setProjectName] = useState("");

  // Task creation form
  const [taskName, setTaskName] = useState("");
  const [taskDeadline, setTaskDeadline] = useState("");
  const [taskAssignee, setTaskAssignee] = useState("");
  const [taskStatus, setTaskStatus] = useState<Status>("未着手");

  const handleCreateProject = (e: React.FormEvent) => {
    e.preventDefault();
    if (!projectName.trim()) return;
    const proj: Project = { id: `p-${Date.now()}`, name: projectName.trim() };
    setProjects((prev) => [...prev, proj]);
    setSelectedProjectId(proj.id);
    setProjectName("");
    setLastAction("created_project");
  };

  const handleAddTask = (e: React.FormEvent) => {
    e.preventDefault();
    if (!taskName.trim() || !selectedProjectId) return;
    const task: Task = {
      id: `t-${Date.now()}`,
      projectId: selectedProjectId,
      name: taskName.trim(),
      deadline: taskDeadline,
      assignee: taskAssignee.trim(),
      status: taskStatus,
    };
    setTasks((prev) => [...prev, task]);
    setTaskName("");
    setTaskDeadline("");
    setTaskAssignee("");
    setTaskStatus("未着手");
    setLastAction("added_task");
  };

  const handleCycleStatus = (taskId: string) => {
    setTasks((prev) =>
      prev.map((t) =>
        t.id === taskId ? { ...t, status: nextStatus(t.status) } : t
      )
    );
    setLastAction("cycled_status");
  };

  const handleResetAll = () => {
    setProjects([]);
    setTasks([]);
    setFilter("all");
    setSelectedProjectId("");
    setProjectName("");
    setTaskName("");
    setTaskDeadline("");
    setTaskAssignee("");
    setTaskStatus("未着手");
    setLastAction("reset");
  };

  const filteredTasks = useMemo(() => {
    if (filter === "all") return tasks;
    return tasks.filter((t) => t.status === filter);
  }, [tasks, filter]);

  const anvilState = useMemo(
    () =>
      JSON.stringify({
        projects: projects.length,
        tasks: filteredTasks.length,
        totalTasks: tasks.length,
        totalProjects: projects.length,
        filter,
        lastAction,
        viewMode,
        selectedProjectId,
      }),
    [projects, tasks, filteredTasks, filter, lastAction, viewMode, selectedProjectId]
  );

  return (
    <div
      className="min-h-screen bg-gray-50 p-6 max-w-4xl mx-auto"
      data-anvil-state={anvilState}
    >
      <h1 className="text-2xl font-bold mb-4">プロジェクト管理</h1>

      {/* Project Creation Form */}
      <section className="bg-white p-4 rounded-lg shadow mb-4">
        <h2 className="text-lg font-semibold mb-2">新規プロジェクト作成</h2>
        <form onSubmit={handleCreateProject} className="flex gap-2">
          <input
            type="text"
            value={projectName}
            onChange={(e) => setProjectName(e.target.value)}
            placeholder="プロジェクト名"
            className="flex-1 border rounded px-3 py-2"
            data-anvil-action="input"
          />
          <button
            type="submit"
            className="bg-blue-600 text-white px-4 py-2 rounded hover:bg-blue-700"
            data-anvil-action="primary"
          >
            作成
          </button>
        </form>
      </section>

      {/* Project Selector */}
      <section className="bg-white p-4 rounded-lg shadow mb-4">
        <h2 className="text-lg font-semibold mb-2">プロジェクト選択</h2>
        <select
          value={selectedProjectId}
          onChange={(e) => setSelectedProjectId(e.target.value)}
          className="border rounded px-3 py-2"
        >
          <option value="">-- 選択してください --</option>
          {projects.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
      </section>

      {/* Task Creation Form */}
      <section className="bg-white p-4 rounded-lg shadow mb-4">
        <h2 className="text-lg font-semibold mb-2">タスク追加</h2>
        <form onSubmit={handleAddTask} className="grid grid-cols-2 gap-2">
          <input
            type="text"
            value={taskName}
            onChange={(e) => setTaskName(e.target.value)}
            placeholder="タスク名"
            className="border rounded px-3 py-2"
          />
          <input
            type="date"
            value={taskDeadline}
            onChange={(e) => setTaskDeadline(e.target.value)}
            className="border rounded px-3 py-2"
          />
          <input
            type="text"
            value={taskAssignee}
            onChange={(e) => setTaskAssignee(e.target.value)}
            placeholder="担当メンバー"
            className="border rounded px-3 py-2"
          />
          <select
            value={taskStatus}
            onChange={(e) => setTaskStatus(e.target.value as Status)}
            className="border rounded px-3 py-2"
          >
            <option value="未着手">未着手</option>
            <option value="進行中">進行中</option>
            <option value="完了">完了</option>
          </select>
          <button
            type="submit"
            className="col-span-2 bg-green-600 text-white px-4 py-2 rounded hover:bg-green-700"
          >
            タスク追加
          </button>
        </form>
      </section>

      {/* Status Filter Bar */}
      <section className="bg-white p-4 rounded-lg shadow mb-4">
        <h2 className="text-lg font-semibold mb-2">進捗フィルター</h2>
        <div className="flex gap-2">
          {(["all", "未着手", "進行中", "完了"] as TaskFilter[]).map((f) => (
            <button
              key={f}
              onClick={() => setFilter(f)}
              className={`px-3 py-1 rounded text-sm ${
                filter === f ? "bg-blue-600 text-white" : "bg-gray-200"
              }`}
            >
              {f === "all" ? "すべて" : f}
            </button>
          ))}
        </div>
      </section>

      {/* Task List */}
      <section className="bg-white p-4 rounded-lg shadow mb-4">
        <h2 className="text-lg font-semibold mb-2">
          タスク一覧 ({filteredTasks.length}件)
        </h2>
        {filteredTasks.length === 0 ? (
          <p className="text-gray-500">タスクがありません</p>
        ) : (
          <ul className="space-y-2">
            {filteredTasks.map((t) => (
              <li
                key={t.id}
                className="flex items-center justify-between border p-3 rounded"
              >
                <div>
                  <p className="font-medium">{t.name}</p>
                  <p className="text-sm text-gray-500">
                    期限: {t.deadline || "未設定"} | 担当: {t.assignee || "未割り当て"}
                  </p>
                </div>
                <div className="flex items-center gap-2">
                  <span
                    className={`px-2 py-1 rounded text-xs ${STATUS_STYLES[t.status]}`}
                  >
                    {t.status}
                  </span>
                  <button
                    onClick={() => handleCycleStatus(t.id)}
                    className="text-xs bg-gray-200 px-2 py-1 rounded"
                  >
                    次へ
                  </button>
                </div>
              </li>
            ))}
          </ul>
        )}
      </section>

      {/* Reset All (restart flow) */}
      <section className="bg-white p-4 rounded-lg shadow">
        <button
          onClick={handleResetAll}
          className="bg-red-600 text-white px-4 py-2 rounded hover:bg-red-700"
          data-anvil-action="restart"
        >
          全てリセット
        </button>
      </section>
    </div>
  );
}
