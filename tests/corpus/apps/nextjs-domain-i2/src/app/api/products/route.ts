import { NextRequest, NextResponse } from "next/server";
import {
  loadProducts,
  saveProducts,
  Product,
} from "@/lib/data";

// ---------------------------------------------------------------------------
// GET /api/products  — 全商品取得 / 検索 (?q=)
// ---------------------------------------------------------------------------
export async function GET(request: NextRequest) {
  try {
    const products = await loadProducts();
    const { searchParams } = new URL(request.url);
    const q = searchParams.get("q")?.trim().toLowerCase();

    let result = products;
    if (q) {
      result = products.filter(
        (p) =>
          p.name.toLowerCase().includes(q) ||
          p.sku.toLowerCase().includes(q)
      );
    }

    return NextResponse.json(result);
  } catch (err) {
    console.error("[products/GET]", err);
    return NextResponse.json(
      { error: "商品の取得に失敗しました。" },
      { status: 500 }
    );
  }
}

// ---------------------------------------------------------------------------
// POST /api/products  — 新規商品作成
// ---------------------------------------------------------------------------
export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const { name, sku, unitPrice, stock, reorderPoint } = body;

    // 必須項目バリデーション
    const errors: string[] = [];
    if (!name || String(name).trim() === "") errors.push("商品名は必須です。");
    if (!sku || String(sku).trim() === "") errors.push("SKUは必須です。");
    if (unitPrice === undefined || unitPrice === null || Number(unitPrice) < 0 || isNaN(Number(unitPrice)))
      errors.push("単価は0以上の数値で入力してください。");
    if (stock === undefined || stock === null || Number(stock) < 0 || isNaN(Number(stock)))
      errors.push("現在庫は0以上の数値で入力してください。");
    if (reorderPoint === undefined || reorderPoint === null || Number(reorderPoint) < 0 || isNaN(Number(reorderPoint)))
      errors.push("発注点は0以上の数値で入力してください。");

    if (errors.length > 0) {
      return NextResponse.json({ error: errors.join(" "), fields: errors }, { status: 400 });
    }

    const products = await loadProducts();

    // SKU 重複チェック
    if (products.some((p) => p.sku === String(sku).trim())) {
      return NextResponse.json(
        { error: "このSKUはすでに存在します。別のSKUをお使いください。" },
        { status: 409 }
      );
    }

    const newProduct: Product = {
      id: `prod_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
      name: String(name).trim(),
      sku: String(sku).trim(),
      unitPrice: Number(unitPrice),
      stock: Number(stock),
      reorderPoint: Number(reorderPoint),
    };

    products.push(newProduct);
    await saveProducts(products);

    return NextResponse.json(newProduct, { status: 201 });
  } catch (err) {
    console.error("[products/POST]", err);
    return NextResponse.json(
      { error: "商品の作成に失敗しました。" },
      { status: 500 }
    );
  }
}

// ---------------------------------------------------------------------------
// PUT /api/products?id=xxx  — 商品編集
// ---------------------------------------------------------------------------
export async function PUT(request: NextRequest) {
  try {
    const { searchParams } = new URL(request.url);
    const id = searchParams.get("id");

    if (!id) {
      return NextResponse.json({ error: "商品IDを指定してください。" }, { status: 400 });
    }

    const body = await request.json();
    const { name, sku, unitPrice, stock, reorderPoint } = body;

    const errors: string[] = [];
    if (name !== undefined && !String(name).trim()) errors.push("商品名は必須です。");
    if (sku !== undefined && !String(sku).trim()) errors.push("SKUは必須です。");
    if (unitPrice !== undefined && (Number(unitPrice) < 0 || isNaN(Number(unitPrice))))
      errors.push("単価は0以上の数値で入力してください。");
    if (stock !== undefined && (Number(stock) < 0 || isNaN(Number(stock))))
      errors.push("現在庫は0以上の数値で入力してください。");
    if (reorderPoint !== undefined && (Number(reorderPoint) < 0 || isNaN(Number(reorderPoint))))
      errors.push("発注点は0以上の数値で入力してください。");

    if (errors.length > 0) {
      return NextResponse.json({ error: errors.join(" "), fields: errors }, { status: 400 });
    }

    const products = await loadProducts();
    const idx = products.findIndex((p) => p.id === id);

    if (idx === -1) {
      return NextResponse.json({ error: "指定された商品が見つかりません。" }, { status: 404 });
    }

    const existing = products[idx];

    // SKU 重複チェック（自身を除く）
    if (sku !== undefined && products.some((p) => p.sku === String(sku).trim() && p.id !== id)) {
      return NextResponse.json(
        { error: "このSKUはすでに存在します。別のSKUをお使いください。" },
        { status: 409 }
      );
    }

    products[idx] = {
      ...existing,
      name: name !== undefined ? String(name).trim() : existing.name,
      sku: sku !== undefined ? String(sku).trim() : existing.sku,
      unitPrice: unitPrice !== undefined ? Number(unitPrice) : existing.unitPrice,
      stock: stock !== undefined ? Number(stock) : existing.stock,
      reorderPoint: reorderPoint !== undefined ? Number(reorderPoint) : existing.reorderPoint,
    };

    await saveProducts(products);
    return NextResponse.json(products[idx]);
  } catch (err) {
    console.error("[products/PUT]", err);
    return NextResponse.json(
      { error: "商品の更新に失敗しました。" },
      { status: 500 }
    );
  }
}

// ---------------------------------------------------------------------------
// DELETE /api/products?id=xxx  — 商品削除
// ---------------------------------------------------------------------------
export async function DELETE(request: NextRequest) {
  try {
    const { searchParams } = new URL(request.url);
    const id = searchParams.get("id");

    if (!id) {
      return NextResponse.json({ error: "商品IDを指定してください。" }, { status: 400 });
    }

    const products = await loadProducts();
    const idx = products.findIndex((p) => p.id === id);

    if (idx === -1) {
      return NextResponse.json({ error: "指定された商品が見つかりません。" }, { status: 404 });
    }

    products.splice(idx, 1);
    await saveProducts(products);

    return NextResponse.json({ success: true, id });
  } catch (err) {
    console.error("[products/DELETE]", err);
    return NextResponse.json(
      { error: "商品の削除に失敗しました。" },
      { status: 500 }
    );
  }
}
