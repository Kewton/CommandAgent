import { NextRequest, NextResponse } from "next/server";
import { getShifts, addShift, generateId, getStaff } from "@/lib/storage";
import { validateShift } from "@/lib/validation";
import type { Shift } from "@/lib/types";

// GET: シフト一覧（オプション: date, staffId, role のフィルタ）
export async function GET(request: NextRequest) {
  try {
    const { searchParams } = new URL(request.url);
    const date = searchParams.get("date");
    const staffId = searchParams.get("staffId");
    const role = searchParams.get("role");

    let shifts = await getShifts();

    // 日付フィルタ
    if (date) {
      shifts = shifts.filter((s) => s.date === date);
    }

    // スタッフIDフィルタ
    if (staffId) {
      shifts = shifts.filter((s) => s.staffId === staffId);
    }

    // 役割フィルタ
    if (role) {
      const staff = await getStaff();
      const staffIds = staff.filter((s) => s.role === role).map((s) => s.id);
      shifts = shifts.filter((s) => staffIds.includes(s.staffId));
    }

    return NextResponse.json({ shifts });
  } catch (err) {
    console.error("シフト取得エラー:", err);
    return NextResponse.json(
      { error: "シフトの取得に失敗しました。" },
      { status: 500 }
    );
  }
}

// POST: 新規シフト追加
export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const existingShifts = await getShifts();

    const result = validateShift(body, existingShifts);
    if (!result.valid) {
      return NextResponse.json(
        { error: result.errors[0], errors: result.errors },
        { status: 400 }
      );
    }

    const shift: Shift = {
      id: generateId("shift"),
      staffId: String(body.staffId),
      date: String(body.date),
      start: String(body.start),
      end: String(body.end),
      breakMinutes: Number(body.breakMinutes) || 0,
    };

    const updated = await addShift(shift);
    return NextResponse.json({ shifts: updated, created: shift }, { status: 201 });
  } catch (err) {
    console.error("シフト追加エラー:", err);
    return NextResponse.json(
      { error: "シフトの追加に失敗しました。" },
      { status: 500 }
    );
  }
}
