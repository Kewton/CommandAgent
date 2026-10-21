import { NextResponse } from "next/server";
import { getStaff, saveStaff, getShifts, saveShifts } from "@/lib/store";
import type { Role } from "@/lib/types";

const VALID_ROLES: Role[] = ["責任者", "キッチン", "ホール"];

// PUT /api/staff/:id — update an existing staff member.
export async function PUT(
  request: Request,
  { params }: { params: { id: string } },
) {
  try {
    const { id } = params;
    const body = await request.json();
    const staff = getStaff();
    const index = staff.findIndex((s) => s.id === id);

    if (index === -1) {
      return NextResponse.json(
        { error: "スタッフが見つかりません。" },
        { status: 404 },
      );
    }

    let name = staff[index].name;
    let role = staff[index].role;

    if (body?.name !== undefined) {
      if (typeof body.name !== "string" || body.name.trim().length === 0) {
        return NextResponse.json(
          { error: "氏名は必須です。" },
          { status: 400 },
        );
      }
      name = body.name.trim();
    }
    if (body?.role !== undefined) {
      if (typeof body.role !== "string" || !VALID_ROLES.includes(body.role as Role)) {
        return NextResponse.json(
          { error: "役割は「責任者」「キッチン」「ホール」のいずれかである必要があります。" },
          { status: 400 },
        );
      }
      role = body.role as Role;
    }

    staff[index] = { ...staff[index], name, role };
    saveStaff(staff);

    return NextResponse.json({ staff: staff[index] });
  } catch (error) {
    return NextResponse.json(
      { error: "スタッフの更新に失敗しました。" },
      { status: 500 },
    );
  }
}

// DELETE /api/staff/:id — remove a staff member and cascade-delete their shifts.
export async function DELETE(
  _request: Request,
  { params }: { params: { id: string } },
) {
  try {
    const { id } = params;
    const staff = getStaff();
    const index = staff.findIndex((s) => s.id === id);

    if (index === -1) {
      return NextResponse.json(
        { error: "スタッフが見つかりません。" },
        { status: 404 },
      );
    }

    staff.splice(index, 1);
    saveStaff(staff);

    // Cascade delete the staff member's shifts.
    const shifts = getShifts();
    const remaining = shifts.filter((s) => s.staffId !== id);
    saveShifts(remaining);

    return NextResponse.json({ success: true });
  } catch (error) {
    return NextResponse.json(
      { error: "スタッフの削除に失敗しました。" },
      { status: 500 },
    );
  }
}
