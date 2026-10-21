// 役割 enum
export type Role = '責任者' | 'キッチン' | 'ホール';

// スタッフ
export interface Staff {
  id: string;
  name: string;
  role: Role;
}

// シフト
export interface Shift {
  id: string;
  staffId: string;
  date: string; // YYYY-MM-DD
  start: string; // HH:MM
  end: string;   // HH:MM
  breakMinutes: number;
}

// 絞り込みパラメータ
export interface ShiftFilter {
  date?: string;
  staffId?: string;
  role?: Role;
}

// スタッフ作成・編集用の入力
export interface StaffInput {
  name: string;
  role: Role;
}

// シフト作成・編集用の入力
export interface ShiftInput {
  staffId: string;
  date: string;
  start: string;
  end: string;
  breakMinutes: number;
}

// 検証結果
export interface ValidationResult {
  valid: boolean;
  errors: string[];
}

// スタッフ週間実働時間
export interface StaffWeeklyHours {
  staffId: string;
  name: string;
  role: Role;
  totalMinutes: number;
  totalHours: number;
  over40: boolean;
}

// 日別・役割別配置人数
export interface DailyRoleCount {
  date: string;
  role: Role;
  count: number;
}
