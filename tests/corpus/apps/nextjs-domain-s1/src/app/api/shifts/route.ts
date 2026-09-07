import { NextRequest, NextResponse } from "next/server";
import { promises as fs } from "fs";
import path from "path";

const DATA_DIR = path.join(process.cwd(), "data");
const SHIFTS_FILE = path.join(DATA_DIR, "shifts.json");

export interface Shift {
  id: string;
  staffId: string;
  date: string;
  startTime: string;
  endTime: string;
  breakMinutes: number;
}

function timeToMinutes(time: string): number {
  const [h, m] = time.split(":").map(Number);
  return h * 60 + m;
}

function isValidTime30(t: string): boolean {
  const [h, m] = t.split(":").map(Number);
  if (h < 0 || h > 23 || m < 0 || m > 59) return false;
  return m % 30 === 0;
}

async function readShifts(): Promise<Shift[]> {
  try {
    await fs.mkdir(DATA_DIR, { recursive: true });
    const content = await fs.readFile(SHIFTS_FILE, "utf-8");
    return JSON.parse(content) as Shift[];
  } catch (err: any) {
    if (err.code === "ENOENT") {
      const defaultShifts: Shift[] = [
        { id: "1", staffId: "1", date: "2026-06-01", startTime: "09:00", endTime: "17:00", breakMinutes: 60 },
        { id: "2", staffId: "2", date: "2026-06-01", startTime: "10:00", endTime: "14:00", breakMinutes: 0 },
        { id: "3", staffId: "3", date: "2026-06-01", startTime: "14:00", endTime: "18:00", breakMinutes: 0 },
        { id: "4", staffId: "1", date: "2026-06-02", startTime: "09:00", endTime: "13:00", breakMinutes: 0 },
        { id: "5", staffId: "2", date: "2026-06-02", startTime: "13:00", endTime: "17:00", breakMinutes: 30 },
      ];
      await fs.writeFile(SHIFTS_FILE, JSON.stringify(defaultShifts, null, 2), "utf-8");
      return defaultShifts;
    }
    throw err;
  }
}

async function writeShifts(shifts: Shift[]): Promise<void> {
  await fs.mkdir(DATA_DIR, { recursive: true });
  await fs.writeFile(SHIFTS_FILE, JSON.stringify(shifts, null, 2), "utf-8");
}

function validateShift(
  staffId: string,
  date: string,
  startTime: string,
  endTime: string,
  breakMinutes: number
): string | null {
  // Validate date format
  if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) {
    return "日付は YYYY-MM-DD の形式で指定してください";
  }

  // Validate time format and 30-minute alignment
  if (!/^\d{2}:\d{2}$/.test(startTime) || !/^\d{2}:\d{2}$/.test(endTime)) {
    return "時刻は HH:MM の形式で指定してください";
  }

  if (!isValidTime30(startTime) || !isValidTime30(endTime)) {
    return "時刻は30分単位で指定してください";
  }

  const startMin = timeToMinutes(startTime);
  const endMin = timeToMinutes(endTime);

  // End must be after start
  if (endMin <= startMin) {
    return "終了時刻は開始時刻より後にしてください";
  }

  // Break must be less than total shift time
  const shiftDuration = endMin - startMin;
  if (breakMinutes < 0) {
    return "休憩分数は0以上の値にしてください";
  }
  if (breakMinutes >= shiftDuration) {
    return "休憩分数は勤務時間（" + shiftDuration + "分）未満にしてください";
  }

  return null;
}

function checkOverlap(
  shifts: Shift[],
  staffId: string,
  date: string,
  startMin: number,
  endMin: number,
  excludeId?: string
): string | null {
  const sameDayStaffShifts = shifts.filter(
    (s) => s.staffId === staffId && s.date === date && s.id !== excludeId
  );

  for (const s of sameDayStaffShifts) {
    const sStart = timeToMinutes(s.startTime);
    const sEnd = timeToMinutes(s.endTime);
    // Overlap: start < otherEnd AND end > otherStart
    // Back-to-back (prev.end === next.start) is allowed
    if (startMin < sEnd && endMin > sStart) {
      return "同じスタッフの時間帯が重なっています（" + s.startTime + "〜" + s.endTime + "）";
    }
  }

  return null;
}

export async function GET() {
  try {
    const shifts = await readShifts();
    return NextResponse.json({ shifts });
  } catch (err) {
    return NextResponse.json({ error: "シフトデータの読み込みに失敗しました" }, { status: 500 });
  }
}

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const { staffId, date, startTime, endTime, breakMinutes } = body;

    if (!staffId || !date || !startTime || !endTime) {
      return NextResponse.json(
        { error: "スタッフ、日付、開始時刻、終了時刻は必須です" },
        { status: 400 }
      );
    }

    const validationError = validateShift(
      staffId,
      date,
      String(startTime),
      String(endTime),
      Number(breakMinutes) || 0
    );
    if (validationError) {
      return NextResponse.json({ error: validationError }, { status: 400 });
    }

    const shifts = await readShifts();
    const startMin = timeToMinutes(String(startTime));
    const endMin = timeToMinutes(String(endTime));

    const overlapError = checkOverlap(shifts, staffId, date, startMin, endMin);
    if (overlapError) {
      return NextResponse.json({ error: overlapError }, { status: 409 });
    }

    const newId = String(Date.now());
    const newShift: Shift = {
      id: newId,
      staffId: String(staffId),
      date: String(date),
      startTime: String(startTime),
      endTime: String(endTime),
      breakMinutes: Number(breakMinutes) || 0,
    };
    shifts.push(newShift);
    await writeShifts(shifts);

    return NextResponse.json({ shift: newShift }, { status: 201 });
  } catch (err) {
    return NextResponse.json({ error: "シフトの登録に失敗しました" }, { status: 500 });
  }
}

export async function PUT(request: NextRequest) {
  try {
    const body = await request.json();
    const { id, staffId, date, startTime, endTime, breakMinutes } = body;

    if (!id || !staffId || !date || !startTime || !endTime) {
      return NextResponse.json(
        { error: "ID、スタッフ、日付、開始時刻、終了時刻は必須です" },
        { status: 400 }
      );
    }

    const validationError = validateShift(
      staffId,
      date,
      String(startTime),
      String(endTime),
      Number(breakMinutes) || 0
    );
    if (validationError) {
      return NextResponse.json({ error: validationError }, { status: 400 });
    }

    const shifts = await readShifts();
    const index = shifts.findIndex((s) => s.id === id);
    if (index === -1) {
      return NextResponse.json(
        { error: "指定されたシフトが見つかりません" },
        { status: 404 }
      );
    }

    const startMin = timeToMinutes(String(startTime));
    const endMin = timeToMinutes(String(endTime));

    const overlapError = checkOverlap(shifts, staffId, date, startMin, endMin, id);
    if (overlapError) {
      return NextResponse.json({ error: overlapError }, { status: 409 });
    }

    shifts[index] = {
      id,
      staffId: String(staffId),
      date: String(date),
      startTime: String(startTime),
      endTime: String(endTime),
      breakMinutes: Number(breakMinutes) || 0,
    };
    await writeShifts(shifts);

    return NextResponse.json({ shift: shifts[index] });
  } catch (err) {
    return NextResponse.json({ error: "シフトの更新に失敗しました" }, { status: 500 });
  }
}

export async function DELETE(request: NextRequest) {
  try {
    const body = await request.json();
    const { id } = body;

    if (!id) {
      return NextResponse.json({ error: "IDは必須です" }, { status: 400 });
    }

    const shifts = await readShifts();
    const index = shifts.findIndex((s) => s.id === id);
    if (index === -1) {
      return NextResponse.json(
        { error: "指定されたシフトが見つかりません" },
        { status: 404 }
      );
    }

    shifts.splice(index, 1);
    await writeShifts(shifts);

    return NextResponse.json({ success: true, id });
  } catch (err) {
    return NextResponse.json({ error: "シフトの削除に失敗しました" }, { status: 500 });
  }
}
