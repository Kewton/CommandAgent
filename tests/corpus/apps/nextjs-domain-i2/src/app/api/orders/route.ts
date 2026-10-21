import { NextRequest, NextResponse } from "next/server";
import {
  loadProducts,
  saveProducts,
  loadOrders,
  saveOrders,
  Product,
  Order,
  OrderStatus,
} from "@/lib/data";

// ---------------------------------------------------------------------------
// GET /api/orders  — 全受注取得（?status=, ?q= フィルタ対応）
// ---------------------------------------------------------------------------
export async function GET(request: NextRequest) {
  try {
    const orders = await loadOrders();
    const { searchParams } = new URL(request.url);
    const statusFilter = searchParams.get("status") as OrderStatus | null;
    const q = searchParams.get("q")?.trim().toLowerCase();

    let result = orders;

    // 状態フィルタ
    if (statusFilter && ["draft", "confirmed", "shipped", "cancelled"].includes(statusFilter)) {
      result = result.filter((o) => o.status === statusFilter);
    }

    // 商品名・顧客名で検索
    if (q) {
      const products = await loadProducts();
      result = result.filter((o) => {
        const customerMatch = o.customerName.toLowerCase().includes(q);
        const productMatch = o.items.some((item) => {
          const p = products.find((pr) => pr.id === item.productId);
          return p ? p.name.toLowerCase().includes(q) : false;
        });
        return customerMatch || productMatch;
      });
    }

    return NextResponse.json(result);
  } catch (err) {
    console.error("[orders/GET]", err);
    return NextResponse.json(
      { error: "受注の取得に失敗しました。" },
      { status: 500 }
    );
  }
}

// ---------------------------------------------------------------------------
// POST /api/orders  — 下書き受注の作成
// ---------------------------------------------------------------------------
export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const { customerName, items, status } = body;

    // バリデーション
    if (!customerName || String(customerName).trim() === "") {
      return NextResponse.json(
        { error: "顧客名は必須です。" },
        { status: 400 }
      );
    }

    if (!Array.isArray(items) || items.length < 1) {
      return NextResponse.json(
        { error: "商品明細は1件以上必要です。" },
        { status: 400 }
      );
    }

    const products = await loadProducts();

    // 各明細のバリデーション
    const validStatuses: OrderStatus[] = ["draft", "confirmed", "shipped", "cancelled"];
    const orderStatus: OrderStatus = validStatuses.includes(status) ? status : "draft";

    for (let i = 0; i < items.length; i++) {
      const item = items[i];
      if (!item.productId || !products.some((p) => p.id === item.productId)) {
        return NextResponse.json(
          { error: `明細${i + 1}: 存在しない商品が指定されています。` },
          { status: 400 }
        );
      }
      if (!item.quantity || Number(item.quantity) < 1 || !Number.isInteger(Number(item.quantity))) {
        return NextResponse.json(
          { error: `明細${i + 1}: 数量は1以上の整数で入力してください。` },
          { status: 400 }
        );
      }
    }

    // 金額計算
    let totalAmount = 0;
    for (const item of items) {
      const p = products.find((pr) => pr.id === item.productId);
      if (p) totalAmount += p.unitPrice * Number(item.quantity);
    }

    const newOrder: Order = {
      id: `ord_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
      customerName: String(customerName).trim(),
      items: items.map((item: any) => ({
        productId: item.productId,
        quantity: Number(item.quantity),
      })),
      status: orderStatus,
      createdAt: new Date().toISOString(),
      totalAmount,
    };

    const orders = await loadOrders();
    orders.push(newOrder);
    await saveOrders(orders);

    return NextResponse.json(newOrder, { status: 201 });
  } catch (err) {
    console.error("[orders/POST]", err);
    return NextResponse.json(
      { error: "受注の作成に失敗しました。" },
      { status: 500 }
    );
  }
}

// ---------------------------------------------------------------------------
// PUT /api/orders?id=xxx  — 状態遷移・下書き編集
// ---------------------------------------------------------------------------
export async function PUT(request: NextRequest) {
  try {
    const { searchParams } = new URL(request.url);
    const id = searchParams.get("id");

    if (!id) {
      return NextResponse.json({ error: "受注IDを指定してください。" }, { status: 400 });
    }

    const body = await request.json();
    const orders = await loadOrders();
    const idx = orders.findIndex((o) => o.id === id);

    if (idx === -1) {
      return NextResponse.json({ error: "指定された受注が見つかりません。" }, { status: 404 });
    }

    const order = orders[idx];
    const { status: newStatus, customerName, items } = body;

    // ---- 出荷済み・キャンセル済みの編集・キャンセルは拒否 ----
    if (order.status === "shipped") {
      return NextResponse.json(
        { error: "出荷済みの受注は編集・キャンセルできません。" },
        { status: 400 }
      );
    }
    if (order.status === "cancelled") {
      return NextResponse.json(
        { error: "キャンセル済みの受注は編集できません。" },
        { status: 400 }
      );
    }

    // ---- 状態遷移処理 ----
    if (newStatus) {
      const validTransitions: Record<OrderStatus, OrderStatus[]> = {
        draft: ["confirmed", "cancelled"],
        confirmed: ["shipped", "cancelled"],
        shipped: [],
        cancelled: [],
      };

      if (!validTransitions[order.status]?.includes(newStatus as OrderStatus)) {
        return NextResponse.json(
          { error: `「${order.status}」から「${newStatus}」への状態遷移はできません。` },
          { status: 400 }
        );
      }

      // draft -> confirmed: 在庫チェック・減算
      if (order.status === "draft" && newStatus === "confirmed") {
        const products = await loadProducts();
        const shortfall: { name: string; short: number }[] = [];

        for (const item of order.items) {
          const p = products.find((pr) => pr.id === item.productId);
          if (!p) {
            return NextResponse.json(
              { error: `商品が見つかりません。受注を編集してください。` },
              { status: 400 }
            );
          }
          if (p.stock < item.quantity) {
            shortfall.push({ name: p.name, short: item.quantity - p.stock });
          }
        }

        if (shortfall.length > 0) {
          const messages = shortfall
            .map((s) => `「${s.name}」: 不足${s.short}個`)
            .join("、");
          return NextResponse.json(
            {
              error: "在庫不足のため確定できません。",
              shortfall,
              detail: messages,
            },
            { status: 409 }
          );
        }

        // 在庫減算
        for (const item of order.items) {
          const p = products.find((pr) => pr.id === item.productId)!;
          p.stock -= item.quantity;
        }
        await saveProducts(products);
      }

      // confirmed -> cancelled: 在庫戻し
      if (order.status === "confirmed" && newStatus === "cancelled") {
        const products = await loadProducts();
        for (const item of order.items) {
          const p = products.find((pr) => pr.id === item.productId);
          if (p) p.stock += item.quantity;
        }
        await saveProducts(products);
      }

      // draft -> cancelled: 在庫变动なし
      order.status = newStatus as OrderStatus;
      orders[idx] = order;
      await saveOrders(orders);
      return NextResponse.json(orders[idx]);
    }

    // ---- 下書きの編集（顧客名・明細） ----
    if (order.status !== "draft") {
      return NextResponse.json(
        { error: "下書き以外の受注は編集できません。" },
        { status: 400 }
      );
    }

    const products = await loadProducts();

    if (customerName !== undefined) {
      if (!String(customerName).trim()) {
        return NextResponse.json({ error: "顧客名は必須です。" }, { status: 400 });
      }
      order.customerName = String(customerName).trim();
    }

    if (items !== undefined) {
      if (!Array.isArray(items) || items.length < 1) {
        return NextResponse.json(
          { error: "商品明細は1件以上必要です。" },
          { status: 400 }
        );
      }

      for (let i = 0; i < items.length; i++) {
        const item = items[i];
        if (!item.productId || !products.some((p) => p.id === item.productId)) {
          return NextResponse.json(
            { error: `明細${i + 1}: 存在しない商品が指定されています。` },
            { status: 400 }
          );
        }
        if (!item.quantity || Number(item.quantity) < 1 || !Number.isInteger(Number(item.quantity))) {
          return NextResponse.json(
            { error: `明細${i + 1}: 数量は1以上の整数で入力してください。` },
            { status: 400 }
          );
        }
      }

      order.items = items.map((item: any) => ({
        productId: item.productId,
        quantity: Number(item.quantity),
      }));

      let totalAmount = 0;
      for (const item of order.items) {
        const p = products.find((pr) => pr.id === item.productId);
        if (p) totalAmount += p.unitPrice * item.quantity;
      }
      order.totalAmount = totalAmount;
    }

    orders[idx] = order;
    await saveOrders(orders);
    return NextResponse.json(orders[idx]);
  } catch (err) {
    console.error("[orders/PUT]", err);
    return NextResponse.json(
      { error: "受注の更新に失敗しました。" },
      { status: 500 }
    );
  }
}

// ---------------------------------------------------------------------------
// DELETE /api/orders?id=xxx  — 下書き削除
// ---------------------------------------------------------------------------
export async function DELETE(request: NextRequest) {
  try {
    const { searchParams } = new URL(request.url);
    const id = searchParams.get("id");

    if (!id) {
      return NextResponse.json({ error: "受注IDを指定してください。" }, { status: 400 });
    }

    const orders = await loadOrders();
    const idx = orders.findIndex((o) => o.id === id);

    if (idx === -1) {
      return NextResponse.json({ error: "指定された受注が見つかりません。" }, { status: 404 });
    }

    // 出荷済み・キャンセル済み・確定済みの削除は不可
    if (orders[idx].status !== "draft") {
      return NextResponse.json(
        { error: "下書き以外の受注は削除できません。" },
        { status: 400 }
      );
    }

    orders.splice(idx, 1);
    await saveOrders(orders);

    return NextResponse.json({ success: true, id });
  } catch (err) {
    console.error("[orders/DELETE]", err);
    return NextResponse.json(
      { error: "受注の削除に失敗しました。" },
      { status: 500 }
    );
  }
}
