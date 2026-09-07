import { NextRequest, NextResponse } from "next/server";
import {
  getExpenses,
  getExpense,
  saveExpense,
  deleteExpense,
  approveExpense,
  rejectExpense,
} from "@/lib/db";

export async function PUT(
  request: NextRequest,
  { params }: { params: { id: string } }
) {
  try {
    const { id } = params;
    const body = await request.json();
    const action = body.action; // "update" | "approve" | "reject"

    // Check existence
    const existing = await getExpense(id);
    if (!existing) {
      return NextResponse.json({ error: "経費申請が見つかりません" }, { status: 404 });
    }

    if (action === "approve") {
      try {
        const updated = await approveExpense(id);
        return NextResponse.json(updated);
      } catch (err: any) {
        const msg = err?.message || "";
        if (msg.includes("BUDGET_OVERFLOW")) {
          try {
            const info = JSON.parse(msg);
            return NextResponse.json(
              {
                error: "予算超過のため承認できません",
                remaining: info.remaining,
                excess: info.excess,
              },
              { status: 409 }
            );
          } catch {
            return NextResponse.json({ error: "予算超過のため承認できません" }, { status: 409 });
          }
        }
        if (msg === "IMMUTABLE") {
          return NextResponse.json(
            { error: "承認済み・却下済みの申請は変更できません" },
            { status: 409 }
          );
        }
        return NextResponse.json({ error: "承認に失敗しました" }, { status: 500 });
      }
    }

    if (action === "reject") {
      const reason = body.rejectionReason;
      if (!reason || typeof reason !== "string" || reason.trim() === "") {
        return NextResponse.json({ error: "却下理由是必須です" }, { status: 400 });
      }
      try {
        const updated = await rejectExpense(id, reason.trim());
        return NextResponse.json(updated);
      } catch (err: any) {
        if (err?.message === "IMMUTABLE") {
          return NextResponse.json(
            { error: "承認済み・却下済みの申請は変更できません" },
            { status: 409 }
          );
        }
        return NextResponse.json({ error: "却下に失敗しました" }, { status: 500 });
      }
    }

    // Default: update fields
    const { applicant, departmentId, date, category, amount, purpose } = body;
    if (!applicant || !departmentId || !date || !category || typeof amount !== "number" || !purpose) {
      return NextResponse.json({ error: "必須項目が不足しています" }, { status: 400 });
    }

    try {
      const updated = await saveExpense({
        id,
        applicant: applicant.trim(),
        departmentId,
        date,
        category: category.trim(),
        amount,
        purpose: purpose.trim(),
      });
      return NextResponse.json(updated);
    } catch (err: any) {
      if (err?.message === "IMMUTABLE") {
        return NextResponse.json(
          { error: "承認済み・却下済みの申請は編集・削除できません" },
          { status: 409 }
        );
      }
      return NextResponse.json({ error: "更新に失敗しました" }, { status: 500 });
    }
  } catch (err) {
    return NextResponse.json({ error: "処理に失敗しました" }, { status: 500 });
  }
}

export async function DELETE(
  _request: NextRequest,
  { params }: { params: { id: string } }
) {
  try {
    const { id } = params;
    try {
      await deleteExpense(id);
      return NextResponse.json({ success: true });
    } catch (err: any) {
      if (err?.message === "IMMUTABLE") {
        return NextResponse.json(
          { error: "承認済み・却下済みの申請は削除できません" },
          { status: 409 }
        );
      }
      if (err?.message === "NOT_FOUND") {
        return NextResponse.json({ error: "経費申請が見つかりません" }, { status: 404 });
      }
      return NextResponse.json({ error: "削除に失敗しました" }, { status: 500 });
    }
  } catch (err) {
    return NextResponse.json({ error: "処理に失敗しました" }, { status: 500 });
  }
}
