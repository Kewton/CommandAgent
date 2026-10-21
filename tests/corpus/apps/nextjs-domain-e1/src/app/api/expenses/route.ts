import { NextRequest, NextResponse } from "next/server";
import {
  getExpenses,
  getExpense,
  createExpense,
  updateExpense,
  deleteExpense,
  approveExpense,
  rejectExpense,
  getFilteredExpenses,
  getDashboardStats,
} from "@/lib/data";

// GET /api/expenses — optional query params: month, departmentId, status, applicant
export async function GET(request: NextRequest) {
  try {
    const { searchParams } = new URL(request.url);
    const month = searchParams.get("month") || undefined;
    const departmentId = searchParams.get("departmentId") || undefined;
    const status = searchParams.get("status") || undefined;
    const applicant = searchParams.get("applicant") || undefined;

    const expenses = await getFilteredExpenses({
      month,
      departmentId,
      status: status as "申請中" | "承認" | "却下" | undefined,
      applicant,
    });
    return NextResponse.json(expenses);
  } catch (error) {
    console.error("Failed to get expenses:", error);
    return NextResponse.json(
      { error: "経費申請の取得に失敗しました" },
      { status: 500 },
    );
  }
}

// POST /api/expenses
export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const { applicant, departmentId, date, category, amount, purpose } = body;
    if (!applicant || !departmentId || !date || !category || !amount || !purpose) {
      return NextResponse.json(
        { error: "申請者、部門、利用日、カテゴリ、金額、目的は必須です" },
        { status: 400 },
      );
    }
    const expense = await createExpense({
      applicant: String(applicant),
      departmentId: String(departmentId),
      date: String(date),
      category: String(category),
      amount: Number(amount),
      purpose: String(purpose),
    });
    return NextResponse.json(expense, { status: 201 });
  } catch (error) {
    console.error("Failed to create expense:", error);
    return NextResponse.json(
      { error: "経費申請の作成に失敗しました" },
      { status: 500 },
    );
  }
}

// PUT /api/expenses — body: { id, ...updates } or { id, action: "approve" } or { id, action: "reject", reason }
export async function PUT(request: NextRequest) {
  try {
    const body = await request.json();
    if (!body.id) {
      return NextResponse.json({ error: "idは必須です" }, { status: 400 });
    }

    // Approval flow
    if (body.action === "approve") {
      const result = await approveExpense(body.id);
      if (result.ok) {
        return NextResponse.json(result.expense);
      }
      return NextResponse.json(
        {
          error: "BUDGET_EXCEEDED",
          remaining: result.remaining,
          overage: result.overage,
        },
        { status: 409 },
      );
    }

    // Rejection flow
    if (body.action === "reject") {
      if (!body.reason) {
        return NextResponse.json(
          { error: "却下理由は必須です" },
          { status: 400 },
        );
      }
      const expense = await rejectExpense(body.id, String(body.reason));
      return NextResponse.json(expense);
    }

    // Regular update (only 申請中 allowed)
    const updates: Record<string, unknown> = {};
    for (const key of ["applicant", "departmentId", "date", "category", "amount", "purpose"]) {
      if (body[key] !== undefined) updates[key] = body[key];
    }
    const expense = await updateExpense(body.id, updates);
    return NextResponse.json(expense);
  } catch (error) {
    const msg =
      error instanceof Error && error.message.includes("見つかりません")
        ? error.message
        : error instanceof Error && error.message.includes("申請中")
          ? error.message
          : "経費申請の更新に失敗しました";
    const status =
      error instanceof Error &&
      (error.message.includes("見つかりません") || error.message.includes("申請中"))
        ? 404
        : 500;
    console.error("Failed to update expense:", error);
    return NextResponse.json({ error: msg }, { status });
  }
}

// DELETE /api/expenses — body: { id }
export async function DELETE(request: NextRequest) {
  try {
    const body = await request.json();
    if (!body.id) {
      return NextResponse.json({ error: "idは必須です" }, { status: 400 });
    }
    await deleteExpense(body.id);
    return NextResponse.json({ success: true });
  } catch (error) {
    const msg =
      error instanceof Error && error.message.includes("申請中")
        ? error.message
        : "経費申請の削除に失敗しました";
    const status =
      error instanceof Error && error.message.includes("申請中") ? 400 : 500;
    console.error("Failed to delete expense:", error);
    return NextResponse.json({ error: msg }, { status });
  }
}
