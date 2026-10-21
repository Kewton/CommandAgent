// Domain type definitions for the project management app.

/**
 * Task lifecycle status.
 * - not_started: 未着手
 * - in_progress: 進行中
 * - completed: 完了
 */
export type TaskStatus = 'not_started' | 'in_progress' | 'completed';

/** The set of valid task status values, useful for validation. */
export const TASK_STATUSES: readonly TaskStatus[] = [
  'not_started',
  'in_progress',
  'completed',
] as const;

/** Japanese display labels for task statuses. */
export const STATUS_LABELS: Record<TaskStatus, string> = {
  not_started: '未着手',
  in_progress: '進行中',
  completed: '完了',
};

/** A team member that tasks can be assigned to. */
export interface TeamMember {
  id: string;
  name: string;
}

/** A project that contains tasks. */
export interface Project {
  id: string;
  name: string;
  createdAt: string; // ISO timestamp
}

/** A task belonging to a project. */
export interface Task {
  id: string;
  projectId: string;
  title: string;
  dueDate: string; // ISO date string (YYYY-MM-DD or full ISO)
  assigneeId: string;
  status: TaskStatus;
  createdAt: string; // ISO timestamp
}

/** Hardcoded default team members for the small agency. */
export const TEAM_MEMBERS: TeamMember[] = [
  { id: 'tm-001', name: '田中 太郎' },
  { id: 'tm-002', name: '佐藤 花子' },
  { id: 'tm-003', name: '鈴木 一郎' },
  { id: 'tm-004', name: '高橋 恵子' },
];
