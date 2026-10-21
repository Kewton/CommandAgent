import { NextResponse } from "next/server";
import { getStaff, saveStaff, getShifts, saveShifts } from "@/lib/storage";
import type { Staff, Role } from "@/lib/types";

const VALID_ROLES: Role[] = ["責任者", "キッチン", "ホール"];

// GET /api/staff — return all staff (with sample seed already in data/staff.json).
export async function GET() {
  try {
    const staff = getStaff();
    return NextResponse.json({ ok: true, data: staff });
  } catch (err) {
    return NextResponse.json(
      { ok: false, error: "スタッフデータの読み込みに失敗しました。" },
      { status: 500 },
    );
  }
}

// POST /api/staff — create a new staff member.
export async function POST(request: Request) {
  try {
    const body = await request.json();
    const name: string = body.name;
    const role: string = body.role;

    if (!name || typeof name !== "string" || name.trim() === "") {
      return NextResponse.json(
        { ok: false, error: "氏名を入力してください。" },
        { status: 400 },
      );
    }

    if (!VALID_ROLES.includes(role as Role)) {
      return NextResponse.json(
        { ok: false, error: "役割を正しく選択してください。（責任者・キッチン・ホール）" },
        { status: 400 },
      );
    }

    const staff = getStaff();

    // Prevent duplicate names
    const dup = staff.find((s) => s.name === name.trim());
    if (dup) {
      return NextResponse.json(
        { ok: false, error: "同じ氏名のスタッフは既にいます。" },
        { status: 400 },
      );
    }

    const newStaff: Staff = {
      id: `staff-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
      name: name.trim(),
      role: role as Role,
    };

    const updated = [...staff, newStaff];
    saveStaff(updated);

    return NextResponse.json({ ok: true, data: newStaff }, { status: 201 });
  } catch {
    return NextResponse.json(
      { ok: false, error: "スタッフの登録処理中にエラーが発生しました。" },
      { status: 500 },
    );
  }
}
