import fs from "fs";
import path from "path";
import type { Staff, Shift } from "./types";

const DATA_DIR = path.join(process.cwd(), "data");
const STAFF_FILE = path.join(DATA_DIR, "staff.json");
const SHIFTS_FILE = path.join(DATA_DIR, "shifts.json");

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
    if (!content.trim()) return [];
    const parsed = JSON.parse(content);
    if (!Array.isArray(parsed)) return [];
    return parsed as T[];
  } catch {
    return [];
  }
}

function writeJson<T>(filePath: string, data: T[]): void {
  ensureDataDir();
  fs.writeFileSync(filePath, JSON.stringify(data, null, 2), "utf-8");
}

export function getStaff(): Staff[] {
  ensureDataDir();
  return readJson<Staff>(STAFF_FILE);
}

export function saveStaff(staff: Staff[]): void {
  ensureDataDir();
  writeJson<Staff>(STAFF_FILE, staff);
}

export function getShifts(): Shift[] {
  ensureDataDir();
  return readJson<Shift>(SHIFTS_FILE);
}

export function saveShifts(shifts: Shift[]): void {
  ensureDataDir();
  writeJson<Shift>(SHIFTS_FILE, shifts);
}
