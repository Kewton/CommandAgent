// 在庫・受注管理アプリのサーバー側データ層
// 商品と受注はプロジェクトルート配下の data/ 配下に JSON ファイルとして保存し、
// ブラウザ再読み込み・サーバー再起動後も保持する。localStorage は使用しない。

import { promises as fs } from "fs";
import path from "path";

// ---------------------------------------------------------------------------
// 型定義
// ---------------------------------------------------------------------------

export type OrderStatus = "draft" | "confirmed" | "shipped" | "cancelled";

export interface Product {
  id: string;
  name: string;
  sku: string;
  unitPrice: number;
  stock: number;
  reorderPoint: number;
}

export interface OrderItem {
  productId: string;
  quantity: number;
}

export interface Order {
  id: string;
  customerName: string;
  items: OrderItem[];
  status: OrderStatus;
  createdAt: string; // ISO 8601
  totalAmount: number;
}

// ---------------------------------------------------------------------------
// 保存先ディレクトリ / ファイル
// ---------------------------------------------------------------------------

// プロジェクトルート（next の cwd）を基準に data/ を作る。
// fs/promises の同期 API ではなく async のみ利用する。
const DATA_DIR = path.resolve(process.cwd(), "data");
const PRODUCTS_FILE = path.join(DATA_DIR, "products.json");
const ORDERS_FILE = path.join(DATA_DIR, "orders.json");

let initialized = false;

// ---------------------------------------------------------------------------
// ユーティリティ
// ---------------------------------------------------------------------------

function generateId(): string {
  // 簡易な UUID v4 相当。crypto を使わず Date + 乱数で生成する。
  return (
    Date.now().toString(36) +
    "-" +
    Math.random().toString(36).slice(2, 10)
  );
}

async function ensureDataDir(): Promise<void> {
  await fs.mkdir(DATA_DIR, { recursive: true });
}

// ---------------------------------------------------------------------------
// シードデータ
// ---------------------------------------------------------------------------

function buildSeedProducts(): Product[] {
  const now = new Date().toISOString();
  return [
    {
      id: "prod-0001",
      name: "オーガニック抹茶ティー 100g",
      sku: "GR-0001",
      unitPrice: 880,
      stock: 24,
      reorderPoint: 10,
    },
    {
      id: "prod-0002",
      name: "北海道バタークッキー 12枚入",
      sku: "GR-0002",
      unitPrice: 420,
      stock: 6,
      reorderPoint: 12,
    },
    {
      id: "prod-0003",
      name: "無添加みそ 500g",
      sku: "GR-0003",
      unitPrice: 650,
      stock: 18,
      reorderPoint: 8,
    },
    {
      id: "prod-0004",
      name: "国産米 5kg",
      sku: "GR-0004",
      unitPrice: 2200,
      stock: 4,
      reorderPoint: 6,
    },
    {
      id: "prod-0005",
      name: "蜂蜜入りはちみつ 300g",
      sku: "GR-0005",
      unitPrice: 980,
      stock: 30,
      reorderPoint: 10,
    },
  ].map((p) => ({ ...p, createdAt: now } as unknown as Product));
}

function buildSeedOrders(products: Product[]): Order[] {
  const now = new Date();
  const iso = (daysAgo: number) =>
    new Date(now.getTime() - daysAgo * 24 * 60 * 60 * 1000).toISOString();

  const mk = (
    id: string,
    customerName: string,
    items: OrderItem[],
    status: OrderStatus,
    daysAgo: number
  ): Order => {
    const total = items.reduce((sum, it) => {
      const p = products.find((x) => x.id === it.productId);
      return sum + (p ? p.unitPrice * it.quantity : 0);
    }, 0);
    return {
      id,
      customerName,
      items,
      status,
      createdAt: iso(daysAgo),
      totalAmount: total,
    };
  };

  return [
    mk(
      "ord-1001",
      "田中 太郎",
      [
        { productId: "prod-0001", quantity: 2 },
        { productId: "prod-0005", quantity: 1 },
      ],
      "draft",
      1
    ),
    mk(
      "ord-1002",
      "佐藤 花子",
      [{ productId: "prod-0003", quantity: 3 }],
      "confirmed",
      3
    ),
    mk(
      "ord-1003",
      "鈴木 一郎",
      [
        { productId: "prod-0004", quantity: 1 },
        { productId: "prod-0001", quantity: 1 },
      ],
      "shipped",
      7
    ),
    mk(
      "ord-1004",
      "高橋 恵美",
      [{ productId: "prod-0002", quantity: 4 }],
      "cancelled",
      10
    ),
  ];
}

// ---------------------------------------------------------------------------
// 初期化（find-or-init）
// ---------------------------------------------------------------------------

/**
 * data/ 配下の商品・受注 JSON を、まだ存在しない場合のみシードで初期化する。
 * すでに存在する場合は上書きしない（サーバー再起動後も保持）。
 */
export async function ensureSeedData(): Promise<void> {
  if (initialized) return;

  await ensureDataDir();

  try {
    await fs.access(PRODUCTS_FILE);
  } catch {
    const seed = buildSeedProducts();
    await fs.writeFile(PRODUCTS_FILE, JSON.stringify(seed, null, 2), "utf-8");
  }

  try {
    await fs.access(ORDERS_FILE);
  } catch {
    // 受注シードは商品シードの最新状態を参照したいので、存在しない場合のみ作る。
    let products: Product[] = [];
    try {
      const raw = await fs.readFile(PRODUCTS_FILE, "utf-8");
      products = JSON.parse(raw) as Product[];
    } catch {
      products = buildSeedProducts();
    }
    const orders = buildSeedOrders(products);
    await fs.writeFile(ORDERS_FILE, JSON.stringify(orders, null, 2), "utf-8");
  }

  initialized = true;
}

// ---------------------------------------------------------------------------
// 商品 CRUD 用の読込/保存
// ---------------------------------------------------------------------------

export async function loadProducts(): Promise<Product[]> {
  await ensureSeedData();
  try {
    const raw = await fs.readFile(PRODUCTS_FILE, "utf-8");
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed as Product[];
  } catch {
    return [];
  }
}

export async function saveProducts(products: Product[]): Promise<void> {
  await ensureSeedData();
  await fs.writeFile(
    PRODUCTS_FILE,
    JSON.stringify(products, null, 2),
    "utf-8"
  );
}

// ---------------------------------------------------------------------------
// 受注 CRUD 用の読込/保存
// ---------------------------------------------------------------------------

export async function loadOrders(): Promise<Order[]> {
  await ensureSeedData();
  try {
    const raw = await fs.readFile(ORDERS_FILE, "utf-8");
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed as Order[];
  } catch {
    return [];
  }
}

export async function saveOrders(orders: Order[]): Promise<void> {
  await ensureSeedData();
  await fs.writeFile(
    ORDERS_FILE,
    JSON.stringify(orders, null, 2),
    "utf-8"
  );
}

// ---------------------------------------------------------------------------
// 補助: ID 生成（API ルートが新規作成時に使う）
// ---------------------------------------------------------------------------

export { generateId };

// 明示的に localStorage を使用しないことを示すための宣言。
// ブラウザ実行時のみ存在する API を参照しないこと。
export const STORAGE_BACKEND = "server-json-file" as const;
