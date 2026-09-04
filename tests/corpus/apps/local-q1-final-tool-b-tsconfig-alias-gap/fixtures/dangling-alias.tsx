import { readTasks } from "@/lib/tasks";

export default function Page() {
  return <main>{readTasks().length}</main>;
}
