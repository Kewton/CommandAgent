// Type definitions for the cafe shift scheduling app

export type Role = "責任者" | "キッチン" | "ホール";

export interface Staff {
  id: string;
  name: string;
  role: Role;
}

export interface Shift {
  id: string;
  staffId: string;
  date: string; // YYYY-MM-DD
  startTime: string; // HH:mm (30-min increments)
  endTime: string; // HH:mm (30-min increments)
  breakMinutes: number; // minutes of break
}

export type ShiftInput = Omit<Shift, "id">;

export interface ShiftStats {
  staffWeeklyHours: Record<string, number>;
  over40: boolean;
  dailyRoleCount: Record<string, Record<Role, number>>;
}
