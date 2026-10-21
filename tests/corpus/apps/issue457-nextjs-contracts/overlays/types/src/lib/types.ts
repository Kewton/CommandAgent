// ─── Core domain types ───────────────────────────────────────────────

export type TaskStatus = "not_started" | "in_progress" | "completed";

export interface Member {
  id: string;
  name: string;
  role: string;
}

export interface Project {
  id: string;
  name: string;
  description: string;
  createdAt: string; // ISO 8601
}

export interface Task {
  id: string;
  projectId: string;
  title: string;
  dueDate: string; // ISO 8601 date
  status: TaskStatus;
  assignee: Member | null;
  createdAt: string; // ISO 8601
}

// ─── Request body shapes ─────────────────────────────────────────────

export interface CreateProjectBody {
  name: string;
  description: string;
}

export interface CreateTaskBody {
  projectId: string;
  title: string;
  dueDate: string;
  status?: TaskStatus;
  assignee?: Member;
}

export interface UpdateTaskBody {
  title?: string;
  dueDate?: string;
  status?: TaskStatus;
  assignee?: Member | null;
}

// ─── Query / filter params ───────────────────────────────────────────

export interface TaskFilterParams {
  projectId?: string;
  status?: TaskStatus | "all";
}

// ─── Generic API response envelope ───────────────────────────────────

export interface ApiResponse<T> {
  data?: T;
  error?: string;
  details?: unknown;
}

// ─── Internal store error shape ──────────────────────────────────────

export interface StoreError {
  code: string;
  message: string;
  status?: number;
  details?: unknown;
}
