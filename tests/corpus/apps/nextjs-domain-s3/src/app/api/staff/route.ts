import { NextResponse } from "next/server";
import { getStaff, saveStaff, getShifts, saveShifts } from "@/lib/store";
import { seedData } from "@/lib/seed";
import type { Staff, Role, Shift } from "@/lib/types";

const VALID_ROLES: Role[] = ["責任者", "キッチン", "ホール"];

// GET /api/staff — return all staff, seeding sample data if the store is empty.
export async function GET() {
  try {
    let staff = getStaff();
    if (staff.length === 0) {
      const { staff: seedStaff, shifts: seedShifts } = seedData();
      staff = seedStaff;
      saveStaff(staff);
      saveShifts(seedShifts);
    }
    return NextResponse.json({ staff });
  } catch (error) {
    return NextResponse.json(
      { error: "スタッフデータの取得に失敗しました。" },
      { status: 500 },
    );
  }
}

// POST /api/staff — create a new staff member with validation.
export async function POST(request: Request) {
  try {
    const body = await request.json();
    const name: unknown = body?.name;
    const role: unknown = body?.role;

    if (typeof name !== "string" || name.trim().length === 0) {
      return NextResponse.json(
        { error: "氏名は必須です。" },
        { status: 400 },
      );
    }
    if (typeof role !== "string" || !VALID_ROLES.includes(role as Role)) {
      return NextResponse.json(
        { error: "役割は「責任者」「キッチン」「ホール」のいずれかである必要があります。" },
        { status: 400 },
      );
    }

    const staff = getStaff();
    const id = `s_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
    const newStaff: Staff = {
      id,
      name: name.trim(),
      role: role as Role,
    };
    staff.push(newStaff);
    saveStaff(staff);

    return NextResponse.json({ staff: newStaff }, { status: 201 });
  } catch (error) {
    return NextResponse.json(
      { error: "スタッフの作成に失敗しました。" },
      { status: 500 },
    );
  }
}
