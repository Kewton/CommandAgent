import { NextRequest, NextResponse } from "next/server";
import {
  getDepartments,
  getDepartmentById,
  createDepartment,
  updateDepartment,
  deleteDepartment,
} from "@/lib/data";

// GET /api/departments
export async function GET() {
  try {
    const departments = await getDepartments();
    return NextResponse.json(departments);
  } catch (error) {
    console.error("Failed to get departments:", error);
    return NextResponse.json(
      { error: "部門の取得に失敗しました" },
      { status: 500 },
    );
  }
}

// POST /api/departments
export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    if (!body.name || typeof body.monthlyBudget !== "number") {
      return NextResponse.json(
        { error: "部門名と月次予算は必須です" },
        { status: 400 },
      );
    }
    const dept = await createDepartment({
      name: String(body.name),
      monthlyBudget: Number(body.monthlyBudget),
    });
    return NextResponse.json(dept, { status: 201 });
  } catch (error) {
    console.error("Failed to create department:", error);
    return NextResponse.json(
      { error: "部門の作成に失敗しました" },
      { status: 500 },
    );
  }
}

// PUT /api/departments — body: { id, name?, monthlyBudget? }
export async function PUT(request: NextRequest) {
  try {
    const body = await request.json();
    if (!body.id) {
      return NextResponse.json({ error: "idは必須です" }, { status: 400 });
    }
    const updates: Record<string, unknown> = {};
    if (body.name !== undefined) updates.name = String(body.name);
    if (body.monthlyBudget !== undefined)
      updates.monthlyBudget = Number(body.monthlyBudget);
    const dept = await updateDepartment(body.id, updates);
    return NextResponse.json(dept);
  } catch (error) {
    const msg =
      error instanceof Error && error.message.includes("見つかりません")
        ? error.message
        : "部門の更新に失敗しました";
    const status =
      error instanceof Error && error.message.includes("見つかりません") ? 404 : 500;
    console.error("Failed to update department:", error);
    return NextResponse.json({ error: msg }, { status });
  }
}

// DELETE /api/departments — body: { id }
export async function DELETE(request: NextRequest) {
  try {
    const body = await request.json();
    if (!body.id) {
      return NextResponse.json({ error: "idは必須です" }, { status: 400 });
    }
    await deleteDepartment(body.id);
    return NextResponse.json({ success: true });
  } catch (error) {
    console.error("Failed to delete department:", error);
    return NextResponse.json(
      { error: "部門の削除に失敗しました" },
      { status: 500 },
    );
  }
}
