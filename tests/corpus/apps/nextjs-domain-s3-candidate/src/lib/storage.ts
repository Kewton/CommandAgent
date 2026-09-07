import fs from "fs";
import path from "path";
import type { Staff, Shift, Role, ShiftInput } from "./types";

// ─── File paths ───────────────────────────────────────────────────────────────
const DATA_DIR = path.join(process.cwd(), "data");
const STAFF_FILE = path.join(DATA_DIR, "staff.json");
const SHIFTS_FILE = path.join(DATA_DIR, "shifts.json");

// ─── Helpers ──────────────────────────────────────────────────────────────────
function ensureDataDir(): void {
  if (!fs.existsSync(DATA_DIR)) {
    fs.mkdirSync(DATA_DIR, { recursive: true });
  }
}

function readJson<T>(filePath: string): T[] {
  if (!fs.existsSync(filePath)) {
    return [];
  }
  try {
    const content = fs.readFileSync(filePath, "utf-8");
    const parsed = JSON.parse(content);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function writeJson<T>(filePath: string, data: T[]): void {
  ensureDataDir();
  fs.writeFileSync(filePath, JSON.stringify(data, null, 2), "utf-8");
}

// ─── ID generation ────────────────────────────────────────────────────────────
let idCounter = 0;
function generateId(prefix: string): string {
  idCounter++;
  return `${prefix}-${Date.now()}-${idCounter}`;
}

// ─── Seed data ────────────────────────────────────────────────────────────────
function getMonday(d = new Date()): string {
  const date = new Date(d);
  const day = date.getDay();
  const diff = day === 0 ? -6 : 1 - day;
  date.setDate(date.getDate() + diff);
  const y = date.getFullYear();
  const m = String(date.getMonth() + 1).padStart(2, "0");
  const dStr = String(date.getDate()).padStart(2, "0");
  return `${y}-${m}-${dStr}`;
}

function addDays(dateStr: string, days: number): string {
  const [y, m, d] = dateStr.split("-").map(Number);
  const date = new Date(y, m - 1, d + days);
  const yy = date.getFullYear();
  const mm = String(date.getMonth() + 1).padStart(2, "0");
  const dd = String(date.getDate()).padStart(2, "0");
  return `${yy}-${mm}-${dd}`;
}

function buildSeedStaff(): Staff[] {
  return [
    { id: "staff-001", name: "田中", role: "責任者" },
    { id: "staff-002", name: "佐藤", role: "キッチン" },
    { id: "staff-003", name: "鈴木", role: "ホール" },
  ];
}

function buildSeedShifts(): Shift[] {
  const monday = getMonday();
  return [
    // 田中（責任者）: Mon-Fri, 8:00-17:00, 60min break → 8h/day × 5 = 40h (exactly at threshold)
    { id: "shift-001", staffId: "staff-001", date: monday, startTime: "08:00", endTime: "17:00", breakMinutes: 60 },
    { id: "shift-002", staffId: "staff-001", date: addDays(monday, 1), startTime: "08:00", endTime: "17:00", breakMinutes: 60 },
    { id: "shift-003", staffId: "staff-001", date: addDays(monday, 2), startTime: "08:00", endTime: "17:00", breakMinutes: 60 },
    { id: "shift-004", staffId: "staff-001", date: addDays(monday, 3), startTime: "08:00", endTime: "17:00", breakMinutes: 60 },
    { id: "shift-005", staffId: "staff-001", date: addDays(monday, 4), startTime: "08:00", endTime: "17:00", breakMinutes: 60 },
    // 佐藤（キッチン）: Mon-Wed 9:00-18:00, 30min break → 8h × 3 = 24h
    { id: "shift-006", staffId: "staff-002", date: monday, startTime: "09:00", endTime: "18:00", breakMinutes: 30 },
    { id: "shift-007", staffId: "staff-002", date: addDays(monday, 1), startTime: "09:00", endTime: "18:00", breakMinutes: 30 },
    { id: "shift-008", staffId: "staff-002", date: addDays(monday, 2), startTime: "09:00", endTime: "18:00", breakMinutes: 30 },
    // 佐藤（キッチン）: Thu-Fri 10:00-19:30, 30min break → 9.5h × 2 = 19h; total = 43h (> 40h, demonstrates 40h exceed)
    { id: "shift-009", staffId: "staff-002", date: addDays(monday, 3), startTime: "10:00", endTime: "19:30", breakMinutes: 30 },
    { id: "shift-010", staffId: "staff-002", date: addDays(monday, 4), startTime: "10:00", endTime: "19:30", breakMinutes: 30 },
    // 鈴木（ホール）: Mon & Wed 11:00-15:00, 0min break → 4h × 2 = 8h
    { id: "shift-011", staffId: "staff-003", date: monday, startTime: "11:00", endTime: "15:00", breakMinutes: 0 },
    { id: "shift-012", staffId: "staff-003", date: addDays(monday, 2), startTime: "11:00", endTime: "15:00", breakMinutes: 0 },
  ];
}

// ─── Core read/write functions ────────────────────────────────────────────────
export function getStaff(): Staff[] {
  let data = readJson<Staff>(STAFF_FILE);
  if (data.length === 0) {
    data = buildSeedStaff();
    saveStaff(data);
  }
  return data;
}

export function saveStaff(list: Staff[]): void {
  writeJson<Staff>(STAFF_FILE, list);
}

export function getShifts(): Shift[] {
  let data = readJson<Shift>(SHIFTS_FILE);
  if (data.length === 0) {
    data = buildSeedShifts();
    saveShifts(data);
  }
  return data;
}

export function saveShifts(list: Shift[]): void {
  writeJson<Shift>(SHIFTS_FILE, list);
}

// ─── Validation ───────────────────────────────────────────────────────────────
export interface ValidationResult {
  valid: boolean;
  error?: string;
}

/**
 * Validate a shift input against all business rules:
 * - 30-minute granularity for start/end times
 * - end time must be after start time
 * - break minutes must be less than total work minutes
 * - No overlapping time ranges for the same staff on the same date
 * - Adjacent shifts (where previous end == next start) are allowed
 */
export function validateShift(
  input: ShiftInput,
  existingShifts: Shift[],
  excludeId?: string
): ValidationResult {
  const { staffId, date, startTime, endTime, breakMinutes } = input;

  // Validate 30-minute granularity
  const startMatch = startTime.match(/^(\d{2}):(\d{2})$/);
  const endMatch = endTime.match(/^(\d{2}):(\d{2})$/);

  if (!startMatch || !endMatch) {
    return { valid: false, error: "開始時刻・終了時刻は HH:mm 形式で指定してください。" };
  }

  const startMin = parseInt(startMatch[1], 10) * 60 + parseInt(startMatch[2], 10);
  const endMin = parseInt(endMatch[1], 10) * 60 + parseInt(endMatch[2], 10);

  if (startMin < 0 || startMin > 1439 || endMin < 0 || endMin > 1439) {
    return { valid: false, error: "時刻は 00:00〜23:30 の範囲で指定してください。" };
  }

  if (startMin % 30 !== 0 || endMin % 30 !== 0) {
    return { valid: false, error: "開始時刻・終了時刻は30分単位で指定してください。" };
  }

  // End must be after start
  if (endMin <= startMin) {
    return { valid: false, error: "終了時刻は開始時刻より後の時間である必要があります。" };
  }

  // Break must be less than total work minutes
  const workMinutes = endMin - startMin;
  if (breakMinutes < 0) {
    return { valid: false, error: "休憩分数は0分以上である必要があります。" };
  }
  if (breakMinutes >= workMinutes) {
    return { valid: false, error: "休憩分数は勤務時間未満である必要があります。" };
  }

  // Check for overlapping shifts for the same staff on the same date
  const overlapping = existingShifts.filter(
    (s) =>
      s.staffId === staffId &&
      s.date === date &&
      s.id !== excludeId
  );

  for (const other of overlapping) {
    const otherStart = parseInt(other.startTime.slice(0, 2), 10) * 60 + parseInt(other.startTime.slice(3), 10);
    const otherEnd = parseInt(other.endTime.slice(0, 2), 10) * 60 + parseInt(other.endTime.slice(3), 10);

    // Overlap: new start < other end AND new end > other start
    // Adjacent (new start == other end or new end == other start) is allowed
    if (startMin < otherEnd && endMin > otherStart) {
      return {
        valid: false,
        error: "同じスタッフの時間帯が重複しています。",
      };
    }
  }

  return { valid: true };
}

// ─── Stats helpers ────────────────────────────────────────────────────────────
export function computeNetMinutes(shift: Shift): number {
  const startMin = parseInt(shift.startTime.slice(0, 2), 10) * 60 + parseInt(shift.startTime.slice(3), 10);
  const endMin = parseInt(shift.endTime.slice(0, 2), 10) * 60 + parseInt(shift.endTime.slice(3), 10);
  return endMin - startMin - shift.breakMinutes;
}

export function computeWeeklyHours(shifts: Shift[]): Map<string, number> {
  const map = new Map<string, number>();
  for (const shift of shifts) {
    const net = computeNetMinutes(shift);
    map.set(shift.staffId, (map.get(shift.staffId) ?? 0) + net);
  }
  // Convert to hours
  for (const [key, value] of map) {
    map.set(key, value / 60);
  }
  return map;
}

// ─── CRUD operations ──────────────────────────────────────────────────────────
export function createStaff(input: Omit<Staff, "id">): Staff {
  const staff = getStaff();
  const newStaff: Staff = { ...input, id: generateId("staff") };
  staff.push(newStaff);
  saveStaff(staff);
  return newStaff;
}

export function updateStaff(id: string, input: Partial<Omit<Staff, "id">>): Staff | null {
  const staff = getStaff();
  const idx = staff.findIndex((s) => s.id === id);
  if (idx === -1) return null;
  staff[idx] = { ...staff[idx], ...input, id };
  saveStaff(staff);
  return staff[idx];
}

export function deleteStaff(id: string): boolean {
  const staff = getStaff();
  const idx = staff.findIndex((s) => s.id === id);
  if (idx === -1) return false;
  staff.splice(idx, 1);
  saveStaff(staff);
  // Also remove associated shifts
  const shifts = getShifts().filter((s) => s.staffId !== id);
  saveShifts(shifts);
  return true;
}

export function createShift(input: ShiftInput, existingShifts: Shift[] = getShifts()): Shift {
  const validation = validateShift(input, existingShifts);
  if (!validation.valid) {
    throw new Error(validation.error);
  }
  const shifts = existingShifts;
  const newShift: Shift = { ...input, id: generateId("shift") };
  shifts.push(newShift);
  saveShifts(shifts);
  return newShift;
}

export function updateShift(
  id: string,
  input: ShiftInput,
  existingShifts: Shift[] = getShifts()
): Shift | null {
  const idx = existingShifts.findIndex((s) => s.id === id);
  if (idx === -1) return null;

  const validation = validateShift(input, existingShifts, id);
  if (!validation.valid) {
    throw new Error(validation.error);
  }

  existingShifts[idx] = { ...input, id };
  saveShifts(existingShifts);
  return existingShifts[idx];
}

export function deleteShift(id: string): boolean {
  const shifts = getShifts();
  const idx = shifts.findIndex((s) => s.id === id);
  if (idx === -1) return false;
  shifts.splice(idx, 1);
  saveShifts(shifts);
  return true;
}
