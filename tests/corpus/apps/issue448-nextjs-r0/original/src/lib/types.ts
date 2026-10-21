// Domain types for the project management application

export type TaskStatus = "not_started" | "in_progress" | "completed";

export interface TeamMember {
  id: string;
  name: string;
  role: string;
}

export interface Task {
  id: string;
  title: string;
  dueDate: string;
  status: TaskStatus;
  assignedTo: string[];
}

export interface Project {
  id: string;
  name: string;
  tasks: Task[];
  members: TeamMember[];
}

export interface CreateProjectInput {
  name: string;
  members: TeamMember[];
}

export interface CreateTaskInput {
  projectId: string;
  title: string;
  dueDate: string;
  assignedTo: string[];
}

export interface AssignMemberInput {
  projectId: string;
  taskId: string;
  memberId: string;
}

// API response types
export interface ApiListProjects {
  projects: Project[];
}

export interface ApiListTasks {
  tasks: Task[];
  filters: {
    status?: TaskStatus;
  };
}

export interface ApiMutate {
  item: Project | Task;
}

// Helper: validate a due-date string, rejecting non-finite or empty dates
export function validateDueDate(dateStr: string): boolean {
  if (!dateStr || typeof dateStr !== "string" || dateStr.trim() === "") {
    return false;
  }
  const time = Date.parse(dateStr);
  return Number.isFinite(time);
}

// Helper: generate a unique ID using the Web Crypto API
export function generateId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  // Fallback for environments without crypto.randomUUID
  return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    const v = c === "x" ? r : (r & 0x3) | 0x8;
    return v.toString(16);
  });
}
