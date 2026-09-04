"use client";

async function toggle() {
  const response = await fetch("/api/todos", { method: "PUT" });
  if (!response.ok) throw new Error("PUT failed");
}

async function remove() {
  const response = await fetch("/api/todos", { method: "DELETE" });
  if (!response.ok) throw new Error("DELETE failed");
}

export default function Page() {
  return <main><button onClick={toggle}>Toggle</button><button onClick={remove}>Delete</button></main>;
}
