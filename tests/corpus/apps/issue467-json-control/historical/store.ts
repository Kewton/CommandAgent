import { promises as fs } from "node:fs";
import { join } from "node:path";
import { randomUUID } from "node:crypto";
import type {
  Assignee,
  Inquiry,
  Priority,
  Status,
  StateTransition,
} from "./types";

const DATA_DIR = join(process.cwd(), "data");
const DATA_FILE = join(DATA_DIR, "inquiries.json");

// ─── Module-level mutex for serialised writes ───────────────────────
let writeChain: Promise<unknown> = Promise.resolve();

// ─── Allowed transitions ────────────────────────────────────────────
const ALLOWED_TRANSITIONS: Record<Status, Status[]> = {
  new: ["in_progress"],
  in_progress: ["resolved"],
  resolved: ["in_progress", "closed"],
  closed: [],
};

// ─── Sample data ─────────────────────────────────────────────────────
function createSampleData(): { inquiries: Inquiry[]; assignees: Assignee[] } {
  const now = new Date().toISOString();

  const assignees: Assignee[] = [
    { id: "a1", name: "田中 太郎", email: "tanaka@example.com" },
    { id: "a2", name: "佐藤 花子", email: "sato@example.com" },
  ];

  const inquiries: Inquiry[] = [
    {
      id: "inq-001",
      subject: "ログインできない",
      content: "IDとパスワードを正確に入力してもログインできません。",
      requester: "山田 一郎",
      category: "アカウント",
      priority: "high",
      assigneeId: "a1",
      status: "in_progress",
      history: [
        { from: "" as Status, to: "new", resolvedAt: undefined, resolvedBy: undefined, resolution: undefined },
        { from: "new", to: "in_progress", resolvedAt: undefined, resolvedBy: "a1", resolution: undefined },
      ],
      createdAt: "2025-01-15T09:00:00.000Z",
      updatedAt: "2025-01-16T10:00:00.000Z",
      resolution: undefined,
    },
    {
      id: "inq-002",
      subject: "報告書の出力方法",
      content: "月次報告書をPDFで出力したいのですが、操作方法が分かりません。",
      requester: "鈴木 二郎",
      category: "操作方法",
      priority: "medium",
      assigneeId: "a2",
      status: "resolved",
      history: [
        { from: "" as Status, to: "new", resolvedAt: undefined, resolvedBy: undefined, resolution: undefined },
        { from: "new", to: "in_progress", resolvedAt: undefined, resolvedBy: "a2", resolution: undefined },
        { from: "in_progress", to: "resolved", resolvedAt: "2025-01-17T14:00:00.000Z", resolvedBy: "a2", resolution: "「報告書」メニューから出力できます。操作マニュアルも送付しました。" },
      ],
      createdAt: "2025-01-14T10:00:00.000Z",
      updatedAt: "2025-01-17T14:00:00.000Z",
      resolution: "「報告書」メニューから出力できます。操作マニュアルも送付しました。",
    },
    {
      id: "inq-003",
      subject: "新規ユーザー登録",
      content: "新入社員のアカウント登録をお願いします。",
      requester: "佐藤 三郎",
      category: "アカウント",
      priority: "low",
      assigneeId: undefined,
      status: "new",
      history: [
        { from: "" as Status, to: "new", resolvedAt: undefined, resolvedBy: undefined, resolution: undefined },
      ],
      createdAt: "2025-01-17T11:00:00.000Z",
      updatedAt: "2025-01-17T11:00:00.000Z",
      resolution: undefined,
    },
  ];

  return { inquiries, assignees };
}

// ─── Persistence helpers ─────────────────────────────────────────────

export async function readData(): Promise<{ inquiries: Inquiry[]; assignees: Assignee[] }> {
  try {
    const raw = await fs.readFile(DATA_FILE, "utf-8");
    const parsed = JSON.parse(raw);
    if (Array.isArray(parsed.inquiries) && Array.isArray(parsed.assignees)) {
      return { inquiries: parsed.inquiries, assignees: parsed.assignees };
    }
    // Valid JSON but not expected shape — initialise sample
    const sample = createSampleData();
    await writeData(sample);
    return sample;
  } catch (err: any) {
    if (err.code === "ENOENT") {
      const sample = createSampleData();
      await writeData(sample);
      return sample;
    }
    // Parse error or other read error — treat as 5xx upstream, re-initialise
    const sample = createSampleData();
    await writeData(sample);
    return sample;
  }
}

export async function writeData(
  data: { inquiries: Inquiry[]; assignees: Assignee[] },
): Promise<void> {
  // Serialise all writers via a module-level promise chain (mutex)
  const task = writeChain.then(async () => {
    await fs.mkdir(DATA_DIR, { recursive: true });
    const tmp = DATA_FILE + `.tmp-${process.pid}-${Date.now()}`;
    const json = JSON.stringify(data, null, 2);
    await fs.writeFile(tmp, json, "utf-8");
    await fs.rename(tmp, DATA_FILE);
  });

  // Ensure chain never rejects so a single failure doesn't deadlock
  writeChain = task.catch(() => {});
  await task;
}

// ─── Transition helpers ──────────────────────────────────────────────

export function isAllowedTransition(from: Status, to: Status): boolean {
  return ALLOWED_TRANSITIONS[from]?.includes(to) ?? false;
}

// ─── Filtering ───────────────────────────────────────────────────────

export interface InquiryFilter {
  status?: Status;
  priority?: Priority;
  assigneeId?: string;
  keyword?: string;
}

export function filterInquiries(
  inquiries: Inquiry[],
  filter: InquiryFilter,
): Inquiry[] {
  return inquiries.filter((inq) => {
    if (filter.status && inq.status !== filter.status) return false;
    if (filter.priority && inq.priority !== filter.priority) return false;
    if (filter.assigneeId && inq.assigneeId !== filter.assigneeId) return false;
    if (filter.keyword) {
      const kw = filter.keyword.toLowerCase();
      const haystack = [
        inq.subject,
        inq.content,
        inq.requester,
        inq.category ?? "",
      ]
        .join(" ")
        .toLowerCase();
      if (!haystack.includes(kw)) return false;
    }
    return true;
  });
}

// ─── Counting helpers (no double-counting) ──────────────────────────

export function groupByStatus(inquiries: Inquiry[]): Record<Status, number> {
  const counts: Record<Status, number> = {
    new: 0,
    in_progress: 0,
    resolved: 0,
    closed: 0,
  };
  // Each inquiry is counted exactly once by its current status
  for (const inq of inquiries) {
    counts[inq.status]++;
  }
  return counts;
}

export function getInProgressByAssignee(
  inquiries: Inquiry[],
): Record<string, number> {
  const counts: Record<string, number> = {};
  // Only count in_progress; each inquiry contributes at most once
  for (const inq of inquiries) {
    if (inq.status === "in_progress" && inq.assigneeId) {
      counts[inq.assigneeId] = (counts[inq.assigneeId] ?? 0) + 1;
    }
  }
  return counts;
}

// ─── CRUD / Mutation helpers ─────────────────────────────────────────

export async function getInquiries(filter?: InquiryFilter): Promise<Inquiry[]> {
  const { inquiries } = await readData();
  if (!filter) return inquiries;
  return filterInquiries(inquiries, filter);
}

export async function getInquiry(id: string): Promise<Inquiry | null> {
  const { inquiries } = await readData();
  return inquiries.find((i) => i.id === id) ?? null;
}

export async function createInquiry(body: {
  subject: string;
  content: string;
  requester: string;
  category?: string;
  priority?: Priority;
  assigneeId?: string;
}): Promise<Inquiry> {
  // Validate required fields
  if (!body.subject?.trim()) throw { status: 400, error: "件名は必須です" };
  if (!body.content?.trim()) throw { status: 400, error: "内容は必須です" };
  if (!body.requester?.trim()) throw { status: 400, error: "依頼者は必須です" };

  const { inquiries, assignees } = await readData();
  const now = new Date().toISOString();

  const newInquiry: Inquiry = {
    id: randomUUID(),
    subject: body.subject.trim(),
    content: body.content.trim(),
    requester: body.requester.trim(),
    category: body.category?.trim() || undefined,
    priority: body.priority ?? "medium",
    assigneeId: body.assigneeId || undefined,
    status: "new",
    history: [{ from: "" as Status, to: "new", resolvedAt: undefined, resolvedBy: undefined, resolution: undefined }],
    createdAt: now,
    updatedAt: now,
    resolution: undefined,
  };

  inquiries.push(newInquiry);
  await writeData({ inquiries, assignees });
  return newInquiry;
}

export async function updateInquiry(
  id: string,
  patch: Partial<Pick<Inquiry, "subject" | "content" | "requester" | "category" | "priority">>,
): Promise<Inquiry> {
  const { inquiries, assignees } = await readData();
  const idx = inquiries.findIndex((i) => i.id === id);
  if (idx === -1) throw { status: 404, error: "問い合わせが見つかりません" };

  const inquiry = inquiries[idx];
  if (inquiry.status === "closed") {
    throw { status: 409, error: "終了した問い合わせは編集できません" };
  }

  // Validate required fields if being set
  if (patch.subject !== undefined && !patch.subject.trim()) {
    throw { status: 400, error: "件名は必須です" };
  }
  if (patch.content !== undefined && !patch.content.trim()) {
    throw { status: 400, error: "内容は必須です" };
  }
  if (patch.requester !== undefined && !patch.requester.trim()) {
    throw { status: 400, error: "依頼者は必須です" };
  }

  inquiries[idx] = {
    ...inquiry,
    ...patch,
    subject: patch.subject ? patch.subject.trim() : inquiry.subject,
    content: patch.content ? patch.content.trim() : inquiry.content,
    requester: patch.requester ? patch.requester.trim() : inquiry.requester,
    category: patch.category !== undefined ? patch.category.trim() || undefined : inquiry.category,
    updatedAt: new Date().toISOString(),
  };

  await writeData({ inquiries, assignees });
  return inquiries[idx];
}

export async function deleteInquiry(id: string): Promise<void> {
  const { inquiries, assignees } = await readData();
  const idx = inquiries.findIndex((i) => i.id === id);
  if (idx === -1) throw { status: 404, error: "問い合わせが見つかりません" };

  if (inquiries[idx].status === "closed") {
    throw { status: 409, error: "終了した問い合わせは削除できません" };
  }

  inquiries.splice(idx, 1);
  await writeData({ inquiries, assignees });
}

export async function transitionInquiry(
  id: string,
  to: Status,
  opts?: { assigneeId?: string; resolution?: string; resolvedBy?: string },
): Promise<Inquiry> {
  const { inquiries, assignees } = await readData();
  const idx = inquiries.findIndex((i) => i.id === id);
  if (idx === -1) throw { status: 404, error: "問い合わせが見つかりません" };

  const inquiry = inquiries[idx];
  const from = inquiry.status;

  // Cannot transition from closed
  if (from === "closed") {
    throw { status: 409, error: "終了した問い合わせは状態変更できません" };
  }

  // Validate transition
  if (!isAllowedTransition(from, to)) {
    throw {
      status: 400,
      error: `${from} → ${to} の遷移は許可されていません`,
    };
  }

  const now = new Date().toISOString();

  // If transitioning to in_progress, require assignee
  if (to === "in_progress") {
    const assigneeId = opts?.assigneeId || inquiry.assigneeId;
    if (!assigneeId) {
      throw { status: 400, error: "対応中にするには担当者の指定が必要です" };
    }
    inquiry.assigneeId = assigneeId;
  }

  // If transitioning to resolved, require resolution
  if (to === "resolved") {
    const resolution = opts?.resolution;
    if (!resolution?.trim()) {
      throw { status: 400, error: "解決にするには解決内容の記載が必要です" };
    }
    inquiry.resolution = resolution.trim();
  }

  const transition: StateTransition = {
    from,
    to,
    resolvedAt: to === "resolved" ? now : undefined,
    resolvedBy: opts?.resolvedBy || inquiry.assigneeId || undefined,
    resolution: to === "resolved" ? inquiry.resolution : undefined,
  };

  inquiry.history.push(transition);
  inquiry.status = to;
  inquiry.updatedAt = now;

  await writeData({ inquiries, assignees });
  return inquiry;
}

// ─── Assignee helpers ────────────────────────────────────────────────

export async function getAssignees(): Promise<Assignee[]> {
  const { assignees } = await readData();
  return assignees;
}

export async function createAssignee(body: {
  name: string;
  email?: string;
}): Promise<Assignee> {
  if (!body.name?.trim()) throw { status: 400, error: "担当者の名前は必須です" };

  const { inquiries, assignees } = await readData();
  const newAssignee: Assignee = {
    id: randomUUID(),
    name: body.name.trim(),
    email: body.email?.trim() || undefined,
  };

  assignees.push(newAssignee);
  await writeData({ inquiries, assignees });
  return newAssignee;
}

export async function getInProgressByAssigneeCounts(
  inquiries: Inquiry[],
  assignees: Assignee[],
): Promise<{ assignee: Assignee; count: number }[]> {
  const raw = getInProgressByAssignee(inquiries);
  return assignees.map((a) => ({
    assignee: a,
    count: raw[a.id] ?? 0,
  }));
}
