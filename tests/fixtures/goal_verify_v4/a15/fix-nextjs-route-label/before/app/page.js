import { formatTask } from "../lib/label.mjs";

const tasks = ["01", "02", "03", "04", "05", "06", "07", "08", "09", "10"];

export default function Page() {
  return (
    <main>
      <h1>Recovery profile fixture</h1>
      {tasks.map((task) => (
        <p id={`result-${task}`} key={task}>{formatTask(task)}</p>
      ))}
    </main>
  );
}
