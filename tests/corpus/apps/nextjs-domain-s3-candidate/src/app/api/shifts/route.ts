import { NextResponse } from "next/server";
import { getStaff, getShifts, saveShifts } from "@/lib/store";
import type { Shift } from "@/lib/types";

// Validate a candidate shift against business rules.
// Returns an error string when invalid, or null when valid.
function validateShift(candidate: Shift, all: Shift[], editingId?: string): string | null {
  // Basic field presence.
  if (!candidate.staffId || !candidate.date || !candidate.startTime || !candidate.endTime) {
    return "すべての項目を入力してください。";
  }

  // Times must be HH:MM format.
  const timeRegex = /^([01]\d|2[0-3]):([0-5]\d)$/;
  if (!timeRegex.test(candidate.startTime) || !timeRegex.test(candidate.endTime)) {
    return "時刻は HH:MM の形式で入力してください。";
  }

  const [sh, sm] = candidate.startTime.split(":").map(Number);
  const [eh, em] = candidate.endTime.split(":").map(Number);
  const startMinutes = sh * 60 + sm;
  const endMinutes = eh * 60 + em;

  // 30-minute increments.
  if (sm % 30 !== 0 || em % 30 !== 0) {
    return "開始時刻・終了時刻は30分単位で入力してください。";
  }

  // End must be after start.
  if (endMinutes <= startMinutes) {
    return "終了時刻は開始時刻より後である必要があります。";
  }

  const workingMinutes = endMinutes - startMinutes;

  // Break must be non-negative and less than working time.
  const breakMinutes = candidate.breakMinutes ?? 0;
  if (typeof breakMinutes !== "number" || breakMinutes < 0) {
    return "休憩時間は0分以上の数値で入力してください。";
  }
  if (breakMinutes > workingMinutes) {
    return "休憩時間は勤務時間より短くなければなりません。";
  }
  if (breakMinutes % 30 !== 0) {
    return "休憩時間は30分単位で入力してください。";
  }

  // Overlap check for the same staff on the same day.
  // Back-to-back (previous end === next start) is allowed.
  const sameDayShifts = all.filter(
    (s) => s.staffId === candidate.staffId && s.date === candidate.date && s.id !== editingId,
  );
  for (const other of sameDayShifts) {
    const [osh, osm] = other.startTime.split(":").map(Number);
    const [oeh, oem] = other.endTime.split(":").map(Number);
    const otherStart = osh * 60 + osm;
    const otherEnd = oeh * 60 + oem;
    // Overlap when start < otherEnd AND otherStart < end.
    // Adjacent (start === otherEnd or otherStart === end) is allowed.
    if (startMinutes < otherEnd && otherStart < endMinutes) {
      return "同じスタッフのシフトが重複しています。";
    }
  }

  return null;
}

// GET /api/shifts — return all shifts.
export async function GET() {
  try {
    const shifts = getShifts();
    return NextResponse.json({ shifts });
  } catch (error) {
    return NextResponse.json(
      { error: "シフトデータの取得に失敗しました。" },
      { status: 500 },
    );
  }
}

// POST /api/shifts — create a new shift with full validation.
export async function POST(request: Request) {
  try {
    const body = await request.json();
    const staff = getStaff();
    const shifts = getShifts();

    const candidate: Shift = {
      id: `sh_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
      staffId: String(body?.staffId ?? ""),
      date: String(body?.date ?? ""),
      startTime: String(body?.startTime ?? ""),
      endTime: String(body?.endTime ?? ""),
      breakMinutes: typeof body?.breakMinutes === "number" ? body.breakMinutes : 0,
    };

    // Verify the staff exists.
    if (!staff.some((s) => s.id === candidate.staffId)) {
      return NextResponse.json(
        { error: "有効なスタッフを選択してください。" },
        { status: 400 },
      );
    }

    const validationError = validateShift(candidate, shifts);
    if (validationError) {
      return NextResponse.json({ error: validationError }, { status: 400 });
    }

    shifts.push(candidate);
    saveShifts(shifts);

    return NextResponse.json({ shift: candidate }, { status: 201 });
  } catch (error) {
    return NextResponse.json(
      { error: "シフトの作成に失敗しました。" },
      { status: 500 },
    );
  }
}
