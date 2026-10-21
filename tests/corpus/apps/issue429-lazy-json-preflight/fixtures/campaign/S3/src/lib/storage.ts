import { promises as fs } from "fs";
import path from "path";
import { Staff, Shift } from "./types";

const DATA_DIR = path.join(process.cwd(), "data");
const STAFF_FILE = path.join(DATA_DIR, "staff.json");
const SHIFTS_FILE = path.join(DATA_DIR, "shifts.json");

// 初期サンプルデータ
function seedStaff(): Staff[] {
  return [
    { id: "staff-001", name: "田中 太郎", role: "責任者" },
    { id: "staff-002", name: "佐藤 花子", role: "キッチン" },
    { id: "staff-003", name: "鈴木 一郎", role: "ホール" },
  ];
}

function seedShifts(): Shift[] {
  // 今週の月〜日（ISO週）を基準にシフトを作成
  const now = new Date();
  const dayOfWeek = now.getDay(); // 0=日, 1=月, ...
  const mondayOffset = dayOfWeek === 0 ? -6 : 1 - dayOfWeek;
  const monday = new Date(now);
  monday.setDate(now.getDate() + mondayOffset);

  function dateStr(offset: number): string {
    const d = new Date(monday);
    d.setDate(monday.getDate() + offset);
    return d.toISOString().slice(0, 10);
  }

  return [
    { id: "shift-001", staffId: "staff-001", date: dateStr(0), start: "09:00", end: "18:00", breakMinutes: 60 },
    { id: "shift-002", staffId: "staff-002", date: dateStr(0), start: "10:00", end: "14:00", breakMinutes: 0 },
    { id: "shift-003", staffId: "staff-003", date: dateStr(0), start: "14:00", end: "22:00", breakMinutes: 60 },
    { id: "shift-004", staffId: "staff-001", date: dateStr(1), start: "09:00", end: "18:00", breakMinutes: 60 },
    { id: "shift-005", staffId: "staff-002", date: dateStr(1), start: "10:00", end: "19:00", breakMinutes: 60 },
    { id: "shift-006", staffId: "staff-003", date: dateStr(2), start: "11:00", end: "20:00", breakMinutes: 60 },
    { id: "shift-007", staffId: "staff-001", date: dateStr(3), start: "10:00", end: "15:00", breakMinutes: 0 },
    { id: "shift-008", staffId: "staff-002", date: dateStr(4), start: "09:30", end: "18:30", breakMinutes: 60 },
  ];
}

async function ensureDataDir(): Promise<void> {
  try {
    await fs.mkdir(DATA_DIR, { recursive: true });
  } catch {
    // already exists
  }
}

async function readFileOrThrow(filePath: string): Promise<string> {
  return fs.readFile(filePath, "utf-8");
}

async function writeFileSafe(filePath: string, content: string): Promise<void> {
  await fs.writeFile(filePath, content, "utf-8");
}

/**
 * スタッフ一覧を取得する。ファイル不存在時はサンプルデータで初期化。
 */
export async function getStaff(): Promise<Staff[]> {
  await ensureDataDir();
  try {
    const raw = await readFileOrThrow(STAFF_FILE);
    const data = JSON.parse(raw) as Staff[];
    if (!Array.isArray(data)) return seedStaff();
    return data;
  } catch {
    const seeded = seedStaff();
    await writeFileSafe(STAFF_FILE, JSON.stringify(seeded, null, 2));
    return seeded;
  }
}

/**
 * スタッフを新規追加する。
 */
export async function addStaff(staff: Staff): Promise<Staff[]> {
  const list = await getStaff();
  list.push(staff);
  await writeFileSafe(STAFF_FILE, JSON.stringify(list, null, 2));
  return list;
}

/**
 * スタッフを編集する。
 */
export async function updateStaff(id: string, updates: Partial<Staff>): Promise<Staff[]> {
  const list = await getStaff();
  const idx = list.findIndex((s) => s.id === id);
  if (idx === -1) throw new Error("スタッフが見つかりません。");
  list[idx] = { ...list[idx], ...updates, id };
  await writeFileSafe(STAFF_FILE, JSON.stringify(list, null, 2));
  return list;
}

/**
 * スタッフを削除する。関連シフトも削除。
 */
export async function deleteStaff(id: string): Promise<Staff[]> {
  const list = await getStaff();
  const filtered = list.filter((s) => s.id !== id);
  await writeFileSafe(STAFF_FILE, JSON.stringify(filtered, null, 2));

  // 関連シフトの削除
  const shifts = await getShifts();
  const filteredShifts = shifts.filter((s) => s.staffId !== id);
  await writeFileSafe(SHIFTS_FILE, JSON.stringify(filteredShifts, null, 2));

  return filtered;
}

/**
 * シフト一覧を取得する。ファイル不存在時はサンプルデータで初期化。
 */
export async function getShifts(): Promise<Shift[]> {
  await ensureDataDir();
  try {
    const raw = await readFileOrThrow(SHIFTS_FILE);
    const data = JSON.parse(raw) as Shift[];
    if (!Array.isArray(data)) return seedShifts();
    return data;
  } catch {
    const seeded = seedShifts();
    await writeFileSafe(SHIFTS_FILE, JSON.stringify(seeded, null, 2));
    return seeded;
  }
}

/**
 * シフトを新規追加する。
 */
export async function addShift(shift: Shift): Promise<Shift[]> {
  const list = await getShifts();
  list.push(shift);
  await writeFileSafe(SHIFTS_FILE, JSON.stringify(list, null, 2));
  return list;
}

/**
 * シフトを編集する。
 */
export async function updateShift(id: string, updates: Partial<Shift>): Promise<Shift[]> {
  const list = await getShifts();
  const idx = list.findIndex((s) => s.id === id);
  if (idx === -1) throw new Error("シフトが見つかりません。");
  list[idx] = { ...list[idx], ...updates, id };
  await writeFileSafe(SHIFTS_FILE, JSON.stringify(list, null, 2));
  return list;
}

/**
 * シフトを削除する。
 */
export async function deleteShift(id: string): Promise<Shift[]> {
  const list = await getShifts();
  const filtered = list.filter((s) => s.id !== id);
  await writeFileSafe(SHIFTS_FILE, JSON.stringify(filtered, null, 2));
  return filtered;
}

/**
 * IDを生成する。
 */
export function generateId(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}
