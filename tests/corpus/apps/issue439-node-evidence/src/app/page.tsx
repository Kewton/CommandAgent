"use client";

import { useState, useCallback, useMemo } from "react";

// ─── Types ────────────────────────────────────────────────────────────────────

type TaskStatus = "未着手" | "進行中" | "完了";
type FilterValue = "すべて" | TaskStatus;

interface Member {
  id: number;
  name: string;
}

interface Task {
  id: number;
  name: string;
  deadline: string;
  assigneeId: number | null;
  status: TaskStatus;
  projectId: number;
}

interface Project {
  id: number;
  name: string;
  createdAt: string;
}

const STATUS_LIST: TaskStatus[] = ["未着手", "進行中", "完了"];
const FILTER_LIST: FilterValue[] = ["すべて", "未着手", "進行中", "完了"];

let nextTaskId = 1;
let nextProjectId = 1;
let nextMemberId = 1;

// ─── Component ────────────────────────────────────────────────────────────────

export default function Page() {
  const [projects, setProjects] = useState<Project[]>([
    { id: nextProjectId++, name: "デフォルトプロジェクト", createdAt: new Date().toISOString() },
  ]);
  const [selectedProjectId, setSelectedProjectId] = useState<number>(1);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [members, setMembers] = useState<Member[]>([
    { id: nextMemberId++, name: "田中 太郎" },
    { id: nextMemberId++, name: "鈴木 花子" },
    { id: nextMemberId++, name: "佐藤 次郎" },
  ]);
  const [filter, setFilter] = useState<FilterValue>("すべて");

  // Form state
  const [projectName, setProjectName] = useState("");
  const [taskName, setTaskName] = useState("");
  const [taskDeadline, setTaskDeadline] = useState("");
  const [taskAssigneeId, setTaskAssigneeId] = useState<number | "">("");

  // ─── Handlers ───────────────────────────────────────────────────────────────

  const handleCreateProject = useCallback((e: React.FormEvent) => {
    e.preventDefault();
    if (!projectName.trim()) return;
    const newProject: Project = {
      id: nextProjectId++,
      name: projectName.trim(),
      createdAt: new Date().toISOString(),
    };
    setProjects((prev) => [...prev, newProject]);
    setProjectName("");
  }, [projectName]);

  const handleAddTask = useCallback((e: React.FormEvent) => {
    e.preventDefault();
    if (!taskName.trim()) return;
    const newTask: Task = {
      id: nextTaskId++,
      name: taskName.trim(),
      deadline: taskDeadline || "",
      assigneeId: taskAssigneeId === "" ? null : Number(taskAssigneeId),
      status: "未着手",
      projectId: selectedProjectId,
    };
    setTasks((prev) => [...prev, newTask]);
    setTaskName("");
    setTaskDeadline("");
    setTaskAssigneeId("");
  }, [taskName, taskDeadline, taskAssigneeId, selectedProjectId]);

  const handleCycleStatus = useCallback((taskId: number) => {
    setTasks((prev) =>
      prev.map((t) => {
        if (t.id !== taskId) return t;
        const idx = STATUS_LIST.indexOf(t.status);
        return { ...t, status: STATUS_LIST[(idx + 1) % STATUS_LIST.length] };
      })
    );
  }, []);

  const handleAddMember = useCallback((e: React.FormEvent) => {
    e.preventDefault();
    // No new member form here; members are pre-populated
  }, []);

  const handleResetAll = useCallback(() => {
    setTasks([]);
    setFilter("すべて");
    setTaskName("");
    setTaskDeadline("");
    setTaskAssigneeId("");
    setProjectName("");
  }, []);

  // ─── Computed ───────────────────────────────────────────────────────────────

  const filteredTasks = useMemo(() => {
    return tasks.filter((t) => {
      if (t.projectId !== selectedProjectId) return false;
      if (filter !== "すべて" && t.status !== filter) return false;
      return true;
    });
  }, [tasks, selectedProjectId, filter]);

  const taskCounts = useMemo(() => {
    const counts: Record<TaskStatus, number> = { "未着手": 0, "進行中": 0, "完了": 0 };
    tasks
      .filter((t) => t.projectId === selectedProjectId)
      .forEach((t) => {
        counts[t.status]++;
      });
    return counts;
  }, [tasks, selectedProjectId]);

  const memberName = useCallback(
    (id: number | null): string => {
      if (id === null) return "未割り当て";
      const m = members.find((mem) => mem.id === id);
      return m ? m.name : "不明";
    },
    [members]
  );

  // ─── data-anvil-state snapshot ──────────────────────────────────────────────
  // Includes filter (immediately responds to user input) and task counts.
  const anvilState = useMemo(
    () =>
      JSON.stringify({
        projectsCount: projects.length,
        tasksCount: filteredTasks.length,
        taskCounts,
        filter,
        selectedProjectId,
        membersCount: members.length,
      }),
    [projects.length, filteredTasks.length, taskCounts, filter, selectedProjectId, members.length]
  );

  // ─── Render ─────────────────────────────────────────────────────────────────

  return (
    <section
      data-anvil-state={anvilState}
      className="min-h-screen bg-gray-50 p-6 max-w-4xl mx-auto"
    >
      <h1 className="text-2xl font-bold mb-6">プロジェクト管理</h1>

      {/* ─── Project Creation ─── */}
      <div className="bg-white rounded-lg shadow p-4 mb-6">
        <h2 className="text-lg font-semibold mb-3">プロジェクト作成</h2>
        <form onSubmit={handleCreateProject} className="flex gap-2 items-end">
          <div className="flex-1">
            <label className="block text-sm text-gray-600 mb-1">プロジェクト名</label>
            <input
              type="text"
              value={projectName}
              onChange={(e) => setProjectName(e.target.value)}
              placeholder="新しいプロジェクト名"
              className="w-full border border-gray-300 rounded px-3 py-2"
            />
          </div>
          <button
            type="submit"
            className="bg-blue-600 text-white px-4 py-2 rounded hover:bg-blue-700"
          >
            作成
          </button>
        </form>
      </div>

      {/* ─── Project Selector ─── */}
      <div className="bg-white rounded-lg shadow p-4 mb-6">
        <h2 className="text-lg font-semibold mb-3">プロジェクト選択</h2>
        <select
          value={selectedProjectId}
          onChange={(e) => setSelectedProjectId(Number(e.target.value))}
          className="border border-gray-300 rounded px-3 py-2"
        >
          {projects.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
      </div>

      {/* ─── Task Addition ─── */}
      <div className="bg-white rounded-lg shadow p-4 mb-6">
        <h2 className="text-lg font-semibold mb-3">タスク追加</h2>
        <form onSubmit={handleAddTask} className="space-y-3">
          <div>
            <label className="block text-sm text-gray-600 mb-1">タスク名</label>
            <input
              type="text"
              value={taskName}
              onChange={(e) => setTaskName(e.target.value)}
              placeholder="タスクのタイトル"
              data-anvil-action="input"
              className="w-full border border-gray-300 rounded px-3 py-2"
            />
          </div>
          <div className="flex gap-3">
            <div className="flex-1">
              <label className="block text-sm text-gray-600 mb-1">期限</label>
              <input
                type="date"
                value={taskDeadline}
                onChange={(e) => setTaskDeadline(e.target.value)}
                className="w-full border border-gray-300 rounded px-3 py-2"
              />
            </div>
            <div className="flex-1">
              <label className="block text-sm text-gray-600 mb-1">担当メンバー</label>
              <select
                value={taskAssigneeId}
                onChange={(e) =>
                  setTaskAssigneeId(e.target.value === "" ? "" : Number(e.target.value))
                }
                className="w-full border border-gray-300 rounded px-3 py-2"
              >
                <option value="">未割り当て</option>
                {members.map((m) => (
                  <option key={m.id} value={m.id}>
                    {m.name}
                  </option>
                ))}
              </select>
            </div>
          </div>
          <button
            type="submit"
            data-anvil-action="primary"
            className="bg-green-600 text-white px-6 py-2 rounded hover:bg-green-700 font-medium"
          >
            タスクを追加
          </button>
        </form>
      </div>

      {/* ─── Status Filter ─── */}
      <div className="bg-white rounded-lg shadow p-4 mb-6">
        <div className="flex items-center justify-between mb-3">
          <h2 className="text-lg font-semibold">タスク一覧</h2>
          <button
            onClick={handleResetAll}
            data-anvil-action="restart"
            className="text-sm text-red-600 hover:underline"
          >
            すべてリセット
          </button>
        </div>
        <div className="flex gap-2 mb-4">
          {FILTER_LIST.map((f) => (
            <button
              key={f}
              onClick={() => setFilter(f)}
              className={`px-3 py-1 rounded text-sm font-medium ${
                filter === f
                  ? "bg-indigo-600 text-white"
                  : "bg-gray-200 text-gray-700 hover:bg-gray-300"
              }`}
            >
              {f}
              {f !== "すべて" && (
                <span className="ml-1 text-xs opacity-75">({taskCounts[f as TaskStatus]})</span>
              )}
            </button>
          ))}
        </div>

        {/* Task List */}
        {filteredTasks.length === 0 ? (
          <p className="text-gray-400 text-sm">タスクがありません。</p>
        ) : (
          <ul className="space-y-2">
            {filteredTasks.map((task) => (
              <li
                key={task.id}
                className="flex items-center justify-between border border-gray-200 rounded p-3"
              >
                <div>
                  <p className="font-medium">{task.name}</p>
                  <p className="text-xs text-gray-500">
                    {task.deadline ? `期限: ${task.deadline}` : "期限なし"} ・ 担当:{" "}
                    {memberName(task.assigneeId)}
                  </p>
                </div>
                <button
                  onClick={() => handleCycleStatus(task.id)}
                  className={`px-3 py-1 rounded text-xs font-medium ${
                    task.status === "未着手"
                      ? "bg-gray-200 text-gray-700"
                      : task.status === "進行中"
                        ? "bg-yellow-200 text-yellow-800"
                        : "bg-green-200 text-green-800"
                  }`}
                >
                  {task.status}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      {/* ─── Team Members ─── */}
      <div className="bg-white rounded-lg shadow p-4">
        <h2 className="text-lg font-semibold mb-3">チームメンバー</h2>
        <ul className="grid grid-cols-2 gap-2">
          {members.map((m) => (
            <li key={m.id} className="text-sm border border-gray-200 rounded px-3 py-2">
              {m.name}
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}
