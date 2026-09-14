"use client";
import { useState } from "react";

type Task = { title: string; deadline: string; member: string; project: string; status: string };

export default function Page() {
  const [projects, setProjects] = useState<string[]>([]);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [projectName, setProjectName] = useState("");
  const [project, setProject] = useState("");
  const [title, setTitle] = useState("");
  const [deadline, setDeadline] = useState("");
  const [member, setMember] = useState("田中");
  const [filter, setFilter] = useState("all");
  const visibleTasks = tasks.filter(task => filter === "all" || task.status === filter);
  return <main data-anvil-state={JSON.stringify({projects, tasks, projectName, title, filter})}>
    <h1>プロジェクト管理</h1>
    <form onSubmit={event => {
      event.preventDefault();
      if (!projectName.trim()) return;
      setProjects(previous => [...previous, projectName.trim()]);
      setProject(projectName.trim());
      setProjectName("");
    }}>
      <label>プロジェクト名<input required data-anvil-action="input" value={projectName}
        onChange={event => setProjectName(event.target.value)} /></label>
      <button data-anvil-action="primary">作成</button>
    </form>
    <ul>{projects.map((name, index) => <li key={index}>{name}</li>)}</ul>
    <form onSubmit={event => {
      event.preventDefault();
      if (!title.trim() || !deadline || !project) return;
      setTasks(previous => [...previous, {title, deadline, member, project, status: "未着手"}]);
      setTitle("");
    }}>
      <label>プロジェクト<select required value={project} onChange={event => setProject(event.target.value)}>
        <option value="">選択</option>{projects.map((name, index) => <option key={index}>{name}</option>)}
      </select></label>
      <label>タスク<input required value={title} onChange={event => setTitle(event.target.value)} /></label>
      <label>期限<input required type="date" value={deadline} onChange={event => setDeadline(event.target.value)} /></label>
      <label>チームメンバー<select value={member} onChange={event => setMember(event.target.value)}>
        <option>田中</option><option>佐藤</option>
      </select></label>
      <button>タスク追加</button>
    </form>
    <label>進捗で絞り込み<select value={filter} onChange={event => setFilter(event.target.value)}>
      <option value="all">すべて</option><option>未着手</option><option>進行中</option><option>完了</option>
    </select></label>
    <ul>{visibleTasks.map(task => <li key={tasks.indexOf(task)}>
      {task.project}: {task.title} / {task.deadline} / {task.member}
      <select aria-label="進捗" value={task.status} onChange={event =>
        setTasks(previous => previous.map(item => item === task ? {...item, status: event.target.value} : item))}>
        <option>未着手</option><option>進行中</option><option>完了</option>
      </select>
    </li>)}</ul>
  </main>;
}
