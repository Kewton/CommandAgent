// Shared domain types for the project management app

export interface Project {
  id: string;
  name: string;
  createdAt: string;
}

export interface Task {
  id: string;
  projectId: string;
  title: string;
  dueDate: string;
  assignedTo: string | null;
  status: 'not_started' | 'in_progress' | 'completed';
  createdAt: string;
}

export interface TeamMember {
  id: string;
  name: string;
}

// Request / response types

export interface CreateProjectRequest {
  name: string;
}

export interface CreateTaskRequest {
  projectId: string;
  title: string;
  dueDate: string;
  assignedTo?: string | null;
}

export interface UpdateTaskStatusRequest {
  status: 'not_started' | 'in_progress' | 'completed';
}

export interface ApiError {
  error: string;
  details?: unknown;
}
