"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import type { Staff, Shift, Role } from "@/lib/types";

// ─── Types ────────────────────────────────────────────────────────────────────

type LoadState = "loading" | "ready" | "error";
type Toast = { id: string; msg: string; type: "success" | "error" };

const ROLES: Role[] = ["責任者", "キッチン", "ホール"];

// ─── Helpers ──────────────────────────────────────────────────────────────────

function getMonday(d = new Date()): Date {
  const date = new Date(d);
  const day = date.getDay();
  const diff = day === 0 ? -6 : 1 - day;
  date.setDate(date.getDate() + diff);
  date.setHours(0, 0, 0, 0);
  return date;
}

function formatDate(d: Date): string {
  return d.toISOString().slice(0, 10);
}

function weekDates(monday: Date): string[] {
  return Array.from({ length: 7 }, (_, i) => {
    const d = new Date(monday);
    d.setDate(d.getDate() + i);
    return formatDate(d);
  });
}

function timeToMinutes(t: string): number {
  const [h, m] = t.split(":").map(Number);
  return h * 60 + m;
}

function workedMinutes(s: Shift): number {
  return timeToMinutes(s.endTime) - timeToMinutes(s.startTime) - s.breakMinutes;
}

function minutesToDisplay(min: number): string {
  const h = Math.floor(min / 60);
  const m = min % 60;
  return `${h}時間${m}分`;
}

const DAY_LABELS = ["月", "火", "水", "木", "金", "土", "日"];

// ─── Component ────────────────────────────────────────────────────────────────

export default function Page() {
  // Data
  const [staff, setStaff] = useState<Staff[]>([]);
  const [shifts, setShifts] = useState<Shift[]>([]);
  const [loadState, setLoadState] = useState<LoadState>("loading");
  const [loadError, setLoadError] = useState<string | null>(null);

  // Toast / banner
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [errorBanner, setErrorBanner] = useState<string | null>(null);

  // Staff form
  const [staffForm, setStaffForm] = useState({ name: "", role: "責任者" as Role });
  const [staffEditId, setStaffEditId] = useState<string | null>(null);
  const [staffFormError, setStaffFormError] = useState<string | null>(null);
  const [staffBusy, setStaffBusy] = useState(false);

  // Shift form
  const [shiftForm, setShiftForm] = useState({
    staffId: "",
    date: "",
    startTime: "09:00",
    endTime: "17:00",
    breakMinutes: 60,
  });
  const [shiftEditId, setShiftEditId] = useState<string | null>(null);
  const [shiftFormError, setShiftFormError] = useState<string | null>(null);
  const [shiftBusy, setShiftBusy] = useState(false);

  // Filters
  const [filterDate, setFilterDate] = useState<string>("");
  const [filterStaff, setFilterStaff] = useState<string>("");
  const [filterRole, setFilterRole] = useState<string>("");

  // Week
  const monday = useMemo(() => getMonday(), []);
  const weekDays = useMemo(() => weekDates(monday), [monday]);

  // ─── Load data ──────────────────────────────────────────────────────────────

  const loadData = useCallback(async () => {
    setLoadState("loading");
    setLoadError(null);
    try {
      const [staffRes, shiftsRes] = await Promise.all([
        fetch("/api/staff"),
        fetch("/api/shifts"),
      ]);
      const staffJson = await staffRes.json();
      const shiftsJson = await shiftsRes.json();
      if (!staffJson.ok || !shiftsJson.ok) {
        throw new Error("データ取得に失敗しました。");
      }
      setStaff(staffJson.data ?? []);
      setShifts(shiftsJson.data ?? []);
      setLoadState("ready");
    } catch (err) {
      setLoadError(err instanceof Error ? err.message : "不明なエラーが発生しました。");
      setLoadState("error");
    }
  }, []);

  useEffect(() => {
    loadData();
  }, [loadData]);

  // ─── Toast helpers ──────────────────────────────────────────────────────────

  const addToast = useCallback((msg: string, type: "success" | "error") => {
    const id = `toast-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
    setToasts((prev) => [...prev, { id, msg, type }]);
    setTimeout(() => setToasts((prev) => prev.filter((t) => t.id !== id)), 3000);
  }, []);

  // ─── Staff CRUD ─────────────────────────────────────────────────────────────

  const submitStaff = useCallback(async (e: React.FormEvent) => {
    e.preventDefault();
    setStaffFormError(null);

    const name = staffForm.name.trim();
    if (!name) {
      setStaffFormError("氏名を入力してください。");
      return;
    }

    setStaffBusy(true);
    try {
      let res: Response;
      if (staffEditId) {
        res = await fetch(`/api/staff/${staffEditId}`, {
          method: "PUT",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ name, role: staffForm.role }),
        });
      } else {
        res = await fetch("/api/staff", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ name, role: staffForm.role }),
        });
      }
      const json = await res.json();
      if (!json.ok) throw new Error(json.error ?? "保存に失敗しました。");

      // Reload
      const freshRes = await fetch("/api/staff");
      const freshJson = await freshRes.json();
      setStaff(freshJson.data ?? []);

      setStaffForm({ name: "", role: "責任者" });
      setStaffEditId(null);
      addToast(staffEditId ? "スタッフを更新しました。" : "スタッフを追加しました。", "success");
    } catch (err) {
      setStaffFormError(err instanceof Error ? err.message : "保存に失敗しました。");
      setErrorBanner("スタッフの保存に失敗しました。もう一度お試しください。");
      addToast("処理失敗: スタッフ保存", "error");
    } finally {
      setStaffBusy(false);
    }
  }, [staffForm, staffEditId, addToast]);

  const editStaff = useCallback((s: Staff) => {
    setStaffForm({ name: s.name, role: s.role });
    setStaffEditId(s.id);
    setStaffFormError(null);
  }, []);

  const cancelStaffEdit = useCallback(() => {
    setStaffForm({ name: "", role: "責任者" });
    setStaffEditId(null);
    setStaffFormError(null);
  }, []);

  const deleteStaff = useCallback(async (id: string) => {
    setStaffBusy(true);
    try {
      const res = await fetch(`/api/staff/${id}`, { method: "DELETE" });
      if (!res.ok) {
        const json = await res.json().catch(() => null);
        throw new Error(json?.error ?? "削除に失敗しました。");
      }
      const freshRes = await fetch("/api/staff");
      if (!freshRes.ok) throw new Error("スタッフ一覧の再取得に失敗しました。");
      const freshJson = await freshRes.json();
      setStaff(freshJson.data ?? []);
      addToast("スタッフを削除しました。", "success");
    } catch (err) {
      setErrorBanner("スタッフの削除に失敗しました。");
      addToast("処理失敗: スタッフ削除", "error");
    } finally {
      setStaffBusy(false);
    }
  }, [addToast]);

  // ─── Shift CRUD ─────────────────────────────────────────────────────────────

  const submitShift = useCallback(async (e: React.FormEvent) => {
    e.preventDefault();
    setShiftFormError(null);

    if (!shiftForm.staffId) {
      setShiftFormError("スタッフを選択してください。");
      return;
    }
    if (!shiftForm.date) {
      setShiftFormError("日付を選択してください。");
      return;
    }

    const startMin = timeToMinutes(shiftForm.startTime);
    const endMin = timeToMinutes(shiftForm.endTime);
    if (startMin % 30 !== 0 || endMin % 30 !== 0) {
      setShiftFormError("開始時刻・終了時刻は30分単位で入力してください。");
      return;
    }
    if (endMin <= startMin) {
      setShiftFormError("終了時刻は開始時刻より後にしてください。");
      return;
    }
    if (shiftForm.breakMinutes >= endMin - startMin) {
      setShiftFormError("休憩時間は勤務時間より短くしてください。");
      return;
    }

    setShiftBusy(true);
    try {
      let res: Response;
      const payload = {
        staffId: shiftForm.staffId,
        date: shiftForm.date,
        startTime: shiftForm.startTime,
        endTime: shiftForm.endTime,
        breakMinutes: shiftForm.breakMinutes,
      };
      if (shiftEditId) {
        res = await fetch(`/api/shifts/${shiftEditId}`, {
          method: "PUT",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(payload),
        });
      } else {
        res = await fetch("/api/shifts", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(payload),
        });
      }
      const json = await res.json();
      if (!json.ok) {
        throw new Error(json.error ?? "シフト保存に失敗しました。");
      }
      const freshRes = await fetch("/api/shifts");
      const freshJson = await freshRes.json();
      setShifts(freshJson.data ?? []);
      setShiftForm({ staffId: "", date: "", startTime: "09:00", endTime: "17:00", breakMinutes: 60 });
      setShiftEditId(null);
      addToast(shiftEditId ? "シフトを更新しました。" : "シフトを追加しました。", "success");
    } catch (err) {
      const msg = err instanceof Error ? err.message : "保存に失敗しました。";
      setShiftFormError(msg);
      setErrorBanner("シフトの保存に失敗しました。もう一度お試しください。");
      addToast("処理失敗: シフト保存", "error");
    } finally {
      setShiftBusy(false);
    }
  }, [shiftForm, shiftEditId, addToast]);

  const editShift = useCallback((s: Shift) => {
    setShiftForm({
      staffId: s.staffId,
      date: s.date,
      startTime: s.startTime,
      endTime: s.endTime,
      breakMinutes: s.breakMinutes,
    });
    setShiftEditId(s.id);
    setShiftFormError(null);
  }, []);

  const cancelShiftEdit = useCallback(() => {
    setShiftForm({ staffId: "", date: "", startTime: "09:00", endTime: "17:00", breakMinutes: 60 });
    setShiftEditId(null);
    setShiftFormError(null);
  }, []);

  const deleteShift = useCallback(async (id: string) => {
    setShiftBusy(true);
    try {
      const res = await fetch(`/api/shifts/${id}`, { method: "DELETE" });
      if (!res.ok) {
        const json = await res.json().catch(() => null);
        throw new Error(json?.error ?? "削除に失敗しました。");
      }
      const freshRes = await fetch("/api/shifts");
      if (!freshRes.ok) throw new Error("シフト一覧の再取得に失敗しました。");
      const freshJson = await freshRes.json();
      setShifts(freshJson.data ?? []);
      addToast("シフトを削除しました。", "success");
    } catch (err) {
      setErrorBanner("シフトの削除に失敗しました。");
      addToast("処理失敗: シフト削除", "error");
    } finally {
      setShiftBusy(false);
    }
  }, [addToast]);

  // ─── Filters ────────────────────────────────────────────────────────────────

  const filteredShifts = useMemo(() => {
    return shifts.filter((s) => {
      if (filterDate && s.date !== filterDate) return false;
      if (filterStaff && s.staffId !== filterStaff) return false;
      if (filterRole) {
        const st = staff.find((x) => x.id === s.staffId);
        if (!st || st.role !== filterRole) return false;
      }
      return true;
    });
  }, [shifts, staff, filterDate, filterStaff, filterRole]);

  // ─── Stats ──────────────────────────────────────────────────────────────────

  const staffWeeklyHours = useMemo(() => {
    const map: Record<string, number> = {};
    for (const s of shifts) {
      map[s.staffId] = (map[s.staffId] ?? 0) + workedMinutes(s);
    }
    return map;
  }, [shifts]);

  const over40 = useMemo(() => {
    const result: Record<string, boolean> = {};
    for (const [id, mins] of Object.entries(staffWeeklyHours)) {
      result[id] = mins > 40 * 60;
    }
    return result;
  }, [staffWeeklyHours]);

  const dayRoleHeadcount = useMemo(() => {
    const map: Record<string, Record<string, number>> = {};
    for (const s of shifts) {
      const st = staff.find((x) => x.id === s.staffId);
      const role = st?.role ?? "不明";
      if (!map[s.date]) map[s.date] = {};
      map[s.date][role] = (map[s.date][role] ?? 0) + 1;
    }
    return map;
  }, [shifts, staff]);

  // ─── Anvil state snapshot ───────────────────────────────────────────────────

  const anvilState = useMemo(() => {
    return JSON.stringify({
      staffCount: staff.length,
      shiftCount: shifts.length,
      filteredCount: filteredShifts.length,
      weekStart: formatDate(monday),
      loadState,
    });
  }, [staff, shifts, filteredShifts, monday, loadState]);

  // ─── Render helpers ─────────────────────────────────────────────────────────

  function renderStaffBadge(role: Role) {
    const cls: Record<Role, string> = {
      "責任者": "bg-purple-100 text-purple-800",
      "キッチン": "bg-amber-100 text-amber-800",
      "ホール": "bg-sky-100 text-sky-800",
    };
    return <span className={`inline-block rounded-full px-2 py-0.5 text-xs font-medium ${cls[role]}`}>{role}</span>;
  }

  // ─── JSX ────────────────────────────────────────────────────────────────────

  return (
    <div
      data-anvil-state={anvilState}
      className="mx-auto max-w-6xl px-4 py-6 md:py-8"
    >
      {/* Header */}
      <header className="mb-6 text-center">
        <h1 className="text-2xl md:text-3xl font-bold text-slate-800">🏪 カフェ週間シフト編成</h1>
        <p className="mt-1 text-sm text-slate-500">
          {formatDate(monday)} ~ {formatDate(weekDays[6] ? new Date(weekDays[6]) : monday)}
        </p>
      </header>

      {/* Error banner */}
      {errorBanner && (
        <div className="mb-4 rounded-lg bg-red-50 border border-red-200 p-3 text-sm text-red-700 flex items-center justify-between">
          <span>⚠️ {errorBanner}</span>
          <button onClick={() => setErrorBanner(null)} className="ml-2 text-red-400 hover:text-red-600">✕</button>
        </div>
      )}

      {/* Toasts */}
      <div className="fixed top-4 right-4 z-50 flex flex-col gap-2">
        {toasts.map((t) => (
          <div
            key={t.id}
            className={`rounded-lg px-4 py-2 text-sm shadow-lg ${t.type === "success" ? "bg-green-600 text-white" : "bg-red-600 text-white"}`}
          >
            {t.msg}
          </div>
        ))}
      </div>

      {/* Loading */}
      {loadState === "loading" && (
        <div className="card flex flex-col items-center justify-center py-16">
          <div className="h-8 w-8 animate-spin rounded-full border-4 border-indigo-200 border-t-indigo-600" />
          <p className="mt-3 text-sm text-slate-500">読み込み中...</p>
        </div>
      )}

      {loadState === "error" && (
        <div className="card py-12 text-center">
          <p className="text-red-600 mb-3">{loadError}</p>
          <button onClick={loadData} className="btn btn-primary" data-anvil-action="restart">
            再読み込み
          </button>
        </div>
      )}

      {loadState === "ready" && (
        <>
          {/* Grid layout */}
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            {/* Staff panel */}
            <section className="card">
              <h2 className="text-lg font-semibold mb-3">👥 スタッフ管理</h2>

              {/* Staff form */}
              <form onSubmit={submitStaff} className="mb-4 space-y-2">
                <div className="flex flex-wrap gap-2">
                  <input
                    type="text"
                    placeholder="氏名"
                    value={staffForm.name}
                    onChange={(e) => setStaffForm((f) => ({ ...f, name: e.target.value }))}
                    data-anvil-action="input"
                    data-anvil-state={JSON.stringify({ field: "staffName", value: staffForm.name })}
                    className="flex-1 min-w-[120px] rounded-md border border-slate-300 px-3 py-2 text-sm focus:ring-2 focus:ring-indigo-400 focus:outline-none"
                  />
                  <select
                    value={staffForm.role}
                    onChange={(e) => setStaffForm((f) => ({ ...f, role: e.target.value as Role }))}
                    className="rounded-md border border-slate-300 px-3 py-2 text-sm focus:ring-2 focus:ring-indigo-400 focus:outline-none"
                  >
                    {ROLES.map((r) => <option key={r} value={r}>{r}</option>)}
                  </select>
                </div>
                {staffFormError && <p className="text-xs text-red-600">{staffFormError}</p>}
                <div className="flex gap-2">
                  <button
                    type="submit"
                    disabled={staffBusy}
                    data-anvil-action="primary"
                    className="btn btn-primary"
                  >
                    {staffEditId ? "更新" : "追加"}
                  </button>
                  {staffEditId && (
                    <button type="button" onClick={cancelStaffEdit} className="btn bg-slate-200 text-slate-700">
                      取消
                    </button>
                  )}
                </div>
              </form>

              {/* Staff list */}
              {staff.length === 0 ? (
                <p className="text-sm text-slate-400 italic">スタッフがいません。追加してください。</p>
              ) : (
                <ul className="divide-y divide-slate-100">
                  {staff.map((s) => (
                    <li key={s.id} className="flex items-center justify-between py-2">
                      <div className="flex items-center gap-2">
                        <span className="font-medium text-sm">{s.name}</span>
                        {renderStaffBadge(s.role)}
                      </div>
                      <div className="flex gap-2">
                        <button onClick={() => editStaff(s)} className="text-xs text-indigo-600 hover:underline">編集</button>
                        <button onClick={() => deleteStaff(s.id)} className="text-xs text-red-600 hover:underline">削除</button>
                      </div>
                    </li>
                  ))}
                </ul>
              )}
            </section>

            {/* Shift form panel */}
            <section className="card">
              <h2 className="text-lg font-semibold mb-3">📋 シフト登録</h2>
              <form onSubmit={submitShift} className="space-y-2">
                <div className="grid grid-cols-2 gap-2">
                  <select
                    value={shiftForm.staffId}
                    onChange={(e) => setShiftForm((f) => ({ ...f, staffId: e.target.value }))}
                    className="rounded-md border border-slate-300 px-3 py-2 text-sm"
                  >
                    <option value="">-- 選択 --</option>
                    {staff.map((s) => <option key={s.id} value={s.id}>{s.name}（{s.role}）</option>)}
                  </select>
                  <input
                    type="date"
                    value={shiftForm.date}
                    onChange={(e) => setShiftForm((f) => ({ ...f, date: e.target.value }))}
                    className="rounded-md border border-slate-300 px-3 py-2 text-sm"
                  />
                </div>
                <div className="grid grid-cols-3 gap-2">
                  <input
                    type="time"
                    step={1800}
                    value={shiftForm.startTime}
                    onChange={(e) => setShiftForm((f) => ({ ...f, startTime: e.target.value }))}
                    className="rounded-md border border-slate-300 px-3 py-2 text-sm"
                  />
                  <input
                    type="time"
                    step={1800}
                    value={shiftForm.endTime}
                    onChange={(e) => setShiftForm((f) => ({ ...f, endTime: e.target.value }))}
                    className="rounded-md border border-slate-300 px-3 py-2 text-sm"
                  />
                  <input
                    type="number"
                    min={0}
                    max={480}
                    step={30}
                    value={shiftForm.breakMinutes}
                    onChange={(e) => setShiftForm((f) => ({ ...f, breakMinutes: Number(e.target.value) }))}
                    className="rounded-md border border-slate-300 px-3 py-2 text-sm"
                    placeholder="休憩(分)"
                  />
                </div>
                {shiftFormError && <p className="text-xs text-red-600">{shiftFormError}</p>}
                <div className="flex gap-2">
                  <button
                    type="submit"
                    disabled={shiftBusy}
                    data-anvil-action="primary"
                    className="btn bg-emerald-600 text-white hover:bg-emerald-700"
                  >
                    {shiftEditId ? "更新" : "登録"}
                  </button>
                  {shiftEditId && (
                    <button type="button" onClick={cancelShiftEdit} className="btn bg-slate-200 text-slate-700">
                      取消
                    </button>
                  )}
                </div>
              </form>
            </section>
          </div>

          {/* Weekly calendar / table */}
          <section className="card mt-6">
            <div className="flex flex-wrap items-center justify-between gap-2 mb-3">
              <h2 className="text-lg font-semibold">📅 週間カレンダー</h2>
              <div className="flex flex-wrap gap-2 text-sm">
                <input
                  type="date"
                  value={filterDate}
                  onChange={(e) => setFilterDate(e.target.value)}
                  className="rounded border border-slate-300 px-2 py-1 text-xs"
                />
                <select value={filterStaff} onChange={(e) => setFilterStaff(e.target.value)} className="rounded border border-slate-300 px-2 py-1 text-xs">
                  <option value="">全スタッフ</option>
                  {staff.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
                </select>
                <select value={filterRole} onChange={(e) => setFilterRole(e.target.value)} className="rounded border border-slate-300 px-2 py-1 text-xs">
                  <option value="">全役割</option>
                  {ROLES.map((r) => <option key={r} value={r}>{r}</option>)}
                </select>
                {(filterDate || filterStaff || filterRole) && (
                  <button
                    onClick={() => { setFilterDate(""); setFilterStaff(""); setFilterRole(""); }}
                    className="text-xs text-indigo-600 hover:underline"
                  >
                    リセット
                  </button>
                )}
              </div>
            </div>

            {filteredShifts.length === 0 ? (
              <p className="text-sm text-slate-400 italic py-4 text-center">
                条件に該当するシフトはありません。
              </p>
            ) : (
              <div className="overflow-x-auto">
                <table className="w-full text-sm border-collapse">
                  <thead>
                    <tr className="border-b border-slate-200">
                      <th className="text-left py-2 px-1 font-medium text-slate-500">日付</th>
                      <th className="text-left py-2 px-1 font-medium text-slate-500">スタッフ</th>
                      <th className="text-left py-2 px-1 font-medium text-slate-500">役割</th>
                      <th className="text-left py-2 px-1 font-medium text-slate-500">開始</th>
                      <th className="text-left py-2 px-1 font-medium text-slate-500">終了</th>
                      <th className="text-left py-2 px-1 font-medium text-slate-500">休憩</th>
                      <th className="text-left py-2 px-1 font-medium text-slate-500">実働</th>
                      <th className="text-left py-2 px-1 font-medium text-slate-500">操作</th>
                    </tr>
                  </thead>
                  <tbody>
                    {filteredShifts.map((s) => {
                      const st = staff.find((x) => x.id === s.staffId);
                      const mins = workedMinutes(s);
                      return (
                        <tr key={s.id} className="border-b border-slate-50 hover:bg-slate-50">
                          <td className="py-1.5 px-1">{s.date}</td>
                          <td className="py-1.5 px-1">{st?.name ?? "—"}</td>
                          <td className="py-1.5 px-1">{st ? renderStaffBadge(st.role) : "—"}</td>
                          <td className="py-1.5 px-1">{s.startTime}</td>
                          <td className="py-1.5 px-1">{s.endTime}</td>
                          <td className="py-1.5 px-1">{s.breakMinutes}分</td>
                          <td className="py-1.5 px-1 font-medium">{minutesToDisplay(mins)}</td>
                          <td className="py-1.5 px-1 whitespace-nowrap">
                            <button onClick={() => editShift(s)} className="text-xs text-indigo-600 hover:underline mr-2">編集</button>
                            <button onClick={() => deleteShift(s.id)} className="text-xs text-red-600 hover:underline">削除</button>
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            )}
          </section>

          {/* Summary: Weekly hours per staff */}
          <section className="card mt-6">
            <h2 className="text-lg font-semibold mb-3">⏱ 週間実働時間</h2>
            {staff.length === 0 ? (
              <p className="text-sm text-slate-400 italic">スタッフがいません。</p>
            ) : (
              <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
                {staff.map((s) => {
                  const mins = staffWeeklyHours[s.id] ?? 0;
                  const isOver = over40[s.id] ?? false;
                  return (
                    <div key={s.id} className={`rounded-lg border p-3 ${isOver ? "border-red-300 bg-red-50" : "border-slate-200 bg-white"}`}>
                      <div className="flex items-center justify-between">
                        <span className="font-medium text-sm">{s.name}</span>
                        {isOver && <span className="text-xs text-red-600 font-bold">⚠ 40h超過</span>}
                      </div>
                      <div className="mt-1 text-sm text-slate-600">{minutesToDisplay(mins)}</div>
                    </div>
                  );
                })}
              </div>
            )}
          </section>

          {/* Summary: Day × Role headcount */}
          <section className="card mt-6">
            <h2 className="text-lg font-semibold mb-3">👨‍🍳 日別・役割別 配置人数</h2>
            <div className="overflow-x-auto">
              <table className="w-full text-sm border-collapse">
                <thead>
                  <tr className="border-b border-slate-200">
                    <th className="text-left py-2 px-2 font-medium text-slate-500">日</th>
                    {ROLES.map((r) => <th key={r} className="text-center py-2 px-2 font-medium text-slate-500">{r}</th>)}
                    <th className="text-center py-2 px-2 font-medium text-slate-500">合計</th>
                  </tr>
                </thead>
                <tbody>
                  {weekDays.map((date, i) => {
                    const dayData = dayRoleHeadcount[date] ?? {};
                    const total = ROLES.reduce((sum, r) => sum + (dayData[r] ?? 0), 0);
                    return (
                      <tr key={date} className="border-b border-slate-50">
                        <td className="py-1.5 px-2 font-medium">{DAY_LABELS[i]} ({date.slice(5)})</td>
                        {ROLES.map((r) => (
                          <td key={r} className="text-center py-1.5 px-2">{dayData[r] ?? 0}人</td>
                        ))}
                        <td className="text-center py-1.5 px-2 font-bold">{total}人</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </section>
        </>
      )}

      {/* Footer */}
      <footer className="mt-8 text-center text-xs text-slate-400">
        カフェ週間シフト編成 v1.0 — データはサーバー側JSONに保存されます
      </footer>
    </div>
  );
}
