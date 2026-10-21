"use client";

import { useCallback, useEffect, useMemo, useState } from "react";

// ─── Types ────────────────────────────────────────────────────────────────────
interface Staff {
  id: string;
  name: string;
  role: "責任者" | "キッチン" | "ホール";
}

interface Shift {
  id: string;
  staffId: string;
  date: string;
  start: string;
  end: string;
  breakMinutes: number;
}

type Status = "loading" | "ready" | "error";
type Banner = { type: "success" | "error" | "info"; message: string } | null;

// ─── Helpers ──────────────────────────────────────────────────────────────────
function timeToMinutes(t: string): number {
  const [h, m] = t.split(":").map(Number);
  return h * 60 + m;
}

function workMinutes(s: Shift): number {
  return timeToMinutes(s.end) - timeToMinutes(s.start) - s.breakMinutes;
}

function formatHours(mins: number): string {
  const h = Math.floor(mins / 60);
  const m = mins % 60;
  return `${h}時間${m > 0 ? `${m}分` : ""}`;
}

function getWeekDates(): string[] {
  const now = new Date();
  const day = now.getDay();
  const monday = new Date(now);
  monday.setDate(now.getDate() - ((day + 6) % 7));
  return Array.from({ length: 7 }, (_, i) => {
    const d = new Date(monday);
    d.setDate(monday.getDate() + i);
    return d.toISOString().slice(0, 10);
  });
}

// ─── Main Page Component ──────────────────────────────────────────────────────
export default function Page() {
  // Data
  const [staff, setStaff] = useState<Staff[]>([]);
  const [shifts, setShifts] = useState<Shift[]>([]);
  const [status, setStatus] = useState<Status>("loading");
  const [banner, setBanner] = useState<Banner>(null);
  const [serverError, setServerError] = useState<string | null>(null);

  // Staff form
  const [staffName, setStaffName] = useState("");
  const [staffRole, setStaffRole] = useState<Staff["role"]>("ホール");
  const [editingStaffId, setEditingStaffId] = useState<string | null>(null);
  const [staffInputError, setStaffInputError] = useState<string | null>(null);

  // Shift form
  const [shiftStaffId, setShiftStaffId] = useState("");
  const [shiftDate, setShiftDate] = useState(getWeekDates()[0] ?? "");
  const [shiftStart, setShiftStart] = useState("09:00");
  const [shiftEnd, setShiftEnd] = useState("17:00");
  const [shiftBreak, setShiftBreak] = useState("60");
  const [editingShiftId, setEditingShiftId] = useState<string | null>(null);
  const [shiftInputError, setShiftInputError] = useState<string | null>(null);

  // Filters
  const [filterDate, setFilterDate] = useState("");
  const [filterStaff, setFilterStaff] = useState("");
  const [filterRole, setFilterRole] = useState("");

  // ─── Fetch data on mount ───────────────────────────────────────────────────
  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        const [staffRes, shiftsRes] = await Promise.all([
          fetch("/api/staff"),
          fetch("/api/shifts"),
        ]);

        if (!staffRes.ok || !shiftsRes.ok) throw new Error("サーバーエラー");

        const staffData: Staff[] = await staffRes.json();
        const shiftsData: Shift[] = await shiftsRes.json();

        if (cancelled) return;

        setStaff(staffData);
        setShifts(shiftsData);
        setStatus("ready");
        setBanner({ type: "success", message: "データの読み込みが完了しました" });
      } catch {
        if (cancelled) return;
        setStatus("error");
        setServerError("データ取得に失敗しました。サーバーを再起動してください。");
      }
    }

    load();
    return () => { cancelled = true; };
  }, []);

  // ─── Staff CRUD ────────────────────────────────────────────────────────────
  const addStaff = async (e: React.FormEvent) => {
    e.preventDefault();
    setStaffInputError(null);
    setBanner(null);

    const name = staffName.trim();
    if (!name) {
      setStaffInputError("氏名を入力してください");
      return;
    }

    try {
      const res = await fetch("/api/staff", {
        method: editingShiftId !== null ? "PUT" : "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          name,
          role: staffRole,
          id: editingStaffId,
        }),
      });

      if (!res.ok) {
        const err = await res.json().catch(() => ({}));
        setStaffInputError(err.error ?? "保存に失敗しました");
        return;
      }

      const updated: Staff[] = await res.json();
      setStaff(updated);
      setStaffName("");
      setStaffRole("ホール");
      setEditingStaffId(null);
      setBanner({ type: "success", message: editingStaffId ? "スタッフ情報を更新しました" : "スタッフを追加しました" });
    } catch {
      setBanner({ type: "error", message: "処理中にエラーが発生しました" });
    }
  };

  const editStaff = (s: Staff) => {
    setEditingStaffId(s.id);
    setStaffName(s.name);
    setStaffRole(s.role);
    setStaffInputError(null);
  };

  const deleteStaff = async (id: string) => {
    try {
      const res = await fetch(`/api/staff/${id}`, { method: "DELETE" });
      if (!res.ok) {
        const err = await res.json().catch(() => ({}));
        setBanner({ type: "error", message: err.error ?? "削除に失敗しました" });
        return;
      }
      const updated: Staff[] = await res.json();
      setStaff(updated);
      setBanner({ type: "success", message: "スタッフを削除しました" });
    } catch {
      setBanner({ type: "error", message: "削除中にエラーが発生しました" });
    }
  };

  // ─── Shift CRUD ────────────────────────────────────────────────────────────
  const addShift = async (e: React.FormEvent) => {
    e.preventDefault();
    setShiftInputError(null);
    setBanner(null);

    const staffId = shiftStaffId;
    const date = shiftDate;
    const start = shiftStart;
    const end = shiftEnd;
    const breakMinutes = Number(shiftBreak);

    if (!staffId) { setShiftInputError("スタッフを選択してください"); return; }
    if (!date) { setShiftInputError("日付を入力してください"); return; }
    if (!start || !end) { setShiftInputError("開始・終了時刻を入力してください"); return; }
    if (breakMinutes < 0) { setShiftInputError("休憩分数は0以上で入力してください"); return; }

    const startMin = timeToMinutes(start);
    const endMin = timeToMinutes(end);
    if (endMin <= startMin) { setShiftInputError("終了時刻は開始時刻より後にしてください"); return; }
    if (endMin - startMin <= breakMinutes) { setShiftInputError("休憩分数は勤務時間未満にしてください"); return; }
    if (startMin % 30 !== 0) { setShiftInputError("開始時刻は30分単位で入力してください"); return; }
    if (endMin % 30 !== 0) { setShiftInputError("終了時刻は30分単位で入力してください"); return; }

    try {
      const res = await fetch("/api/shifts", {
        method: editingShiftId ? "POST" : "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ staffId, date, start, end, breakMinutes, id: editingShiftId }),
      });

      if (!res.ok) {
        const err = await res.json().catch(() => ({}));
        setShiftInputError(err.error ?? "シフトの登録に失敗しました");
        return;
      }

      const updated: Shift[] = await res.json();
      setShifts(updated);
      setEditingShiftId(null);
      setBanner({ type: "success", message: editingShiftId ? "シフトを更新しました" : "シフトを追加しました" });
    } catch {
      setBanner({ type: "error", message: "処理中にエラーが発生しました" });
    }
  };

  const editShift = (s: Shift) => {
    setEditingShiftId(s.id);
    setShiftStaffId(s.staffId);
    setShiftDate(s.date);
    setShiftStart(s.start);
    setShiftEnd(s.end);
    setShiftBreak(String(s.breakMinutes));
    setShiftInputError(null);
  };

  const deleteShift = async (id: string) => {
    try {
      const res = await fetch(`/api/shifts/${id}`, { method: "DELETE" });
      if (!res.ok) {
        const err = await res.json().catch(() => ({}));
        setBanner({ type: "error", message: err.error ?? "削除に失敗しました" });
        return;
      }
      const updated: Shift[] = await res.json();
      setShifts(updated);
      setEditingShiftId(null);
      setBanner({ type: "success", message: "シフトを削除しました" });
    } catch {
      setBanner({ type: "error", message: "削除中にエラーが発生しました" });
    }
  };

  // ─── Computed stats ────────────────────────────────────────────────────────
  const weekDates = useMemo(() => getWeekDates(), []);

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

  const staffStats = useMemo(() => {
    return staff.map((s) => {
      const staffShifts = shifts.filter((sh) => sh.staffId === s.id);
      const totalMinutes = staffShifts.reduce((acc, sh) => acc + workMinutes(sh), 0);
      return {
        staff: s,
        totalMinutes,
        over40: totalMinutes > 2400,
        shiftCount: staffShifts.length,
      };
    });
  }, [staff, shifts]);

  const dailyHeadcount = useMemo(() => {
    const map: Record<string, Record<string, number>> = {};
    weekDates.forEach((d) => {
      map[d] = { "責任者": 0, "キッチン": 0, "ホール": 0, "合計": 0 };
    });
    filteredShifts.forEach((sh) => {
      const st = staff.find((x) => x.id === sh.staffId);
      if (st && map[sh.date]) {
        map[sh.date][st.role] = (map[sh.date][st.role] ?? 0) + 1;
        map[sh.date]["合計"] = (map[sh.date]["合計"] ?? 0) + 1;
      }
    });
    return map;
  }, [filteredShifts, staff, weekDates]);

  const dayLabels = useMemo(() => {
    const names = ["日", "月", "火", "水", "木", "金", "土"];
    return weekDates.map((d) => {
      const dt = new Date(d + "T00:00:00");
      return `${dt.getMonth() + 1}/${dt.getDate()}(${names[dt.getDay()]})`;
    });
  }, [weekDates]);

  // ─── Anvil state snapshot ──────────────────────────────────────────────────
  const anvilState = JSON.stringify({
    filters: { filterDate, filterStaff, filterRole },
    visibleShifts: filteredShifts.length,
    totalStaff: staff.length,
    totalShifts: shifts.length,
    editingStaffId,
    editingShiftId,
    status,
  });

  // ─── Render ────────────────────────────────────────────────────────────────
  return (
    <div
      data-anvil-state={anvilState}
      className="min-h-screen bg-gray-50 text-gray-900 font-sans p-4 md:p-6 space-y-6"
    >
      {/* Header */}
      <header className="bg-white rounded-xl shadow-sm p-4 md:p-6">
        <h1 className="text-xl md:text-2xl font-bold text-gray-800">☕ カフェ週間シフト編成</h1>
        <p className="text-sm text-gray-500 mt-1">スタッフとシフトを管理する</p>
      </header>

      {/* Banners */}
      {banner && (
        <div
          className={`rounded-lg p-3 text-sm font-medium ${
            banner.type === "success" ? "bg-green-50 text-green-700 border border-green-200" :
            banner.type === "error" ? "bg-red-50 text-red-700 border border-red-200" :
            "bg-blue-50 text-blue-700 border border-blue-200"
          }`}
        >
          {banner.message}
          <button onClick={() => setBanner(null)} className="ml-2 text-xs underline">閉じる</button>
        </div>
      )}

      {serverError && (
        <div className="rounded-lg p-3 text-sm bg-red-50 text-red-700 border border-red-200">
          {serverError}
        </div>
      )}

      {status === "loading" && (
        <div className="text-center py-12 text-gray-500">
          <div className="animate-spin w-8 h-8 border-4 border-blue-200 border-t-blue-600 rounded-full mx-auto mb-3" />
          読み込み中...
        </div>
      )}

      {status === "error" && (
        <div className="text-center py-12">
          <p className="text-red-600 mb-4">データの読み込みに失敗しました</p>
          <button
            onClick={() => window.location.reload()}
            className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 transition"
          >
            再読み込み
          </button>
        </div>
      )}

      {status === "ready" && (
        <>
          {/* Staff Management */}
          <section className="bg-white rounded-xl shadow-sm p-4 md:p-6">
            <h2 className="text-lg font-semibold mb-4">スタッフ管理</h2>

            <form onSubmit={addStaff} className="flex flex-wrap items-end gap-3 mb-4">
              <div className="flex flex-col">
                <label className="text-xs text-gray-500 mb-1">氏名</label>
                <input
                  type="text"
                  value={staffName}
                  onChange={(e) => setStaffName(e.target.value)}
                  data-anvil-action="input"
                  placeholder="氏名を入力"
                  className="border rounded px-3 py-1.5 w-32 text-sm"
                />
              </div>
              <div className="flex flex-col">
                <label className="text-xs text-gray-500 mb-1">役割</label>
                <select
                  value={staffRole}
                  onChange={(e) => setStaffRole(e.target.value as Staff["role"])}
                  className="border rounded px-3 py-1.5 text-sm"
                >
                  <option value="責任者">責任者</option>
                  <option value="キッチン">キッチン</option>
                  <option value="ホール">ホール</option>
                </select>
              </div>
              <button
                type="submit"
                data-anvil-action="primary"
                className="bg-blue-600 text-white px-4 py-1.5 rounded text-sm font-medium hover:bg-blue-700 transition"
              >
                {editingStaffId ? "更新" : "追加"}
              </button>
              {editingStaffId && (
                <button
                  type="button"
                  onClick={() => { setEditingStaffId(null); setStaffName(""); setStaffRole("ホール"); setStaffInputError(null); }}
                  className="text-gray-500 text-sm hover:text-gray-700"
                >
                  キャンセル
                </button>
              )}
            </form>

            {staffInputError && (
              <p className="text-sm text-red-600 mb-3">{staffInputError}</p>
            )}

            {staff.length === 0 ? (
              <p className="text-gray-400 text-sm">スタッフがいません。追加してください。</p>
            ) : (
              <div className="overflow-x-auto">
                <table className="w-full text-sm">
                  <thead>
                    <tr className="border-b text-left text-gray-500">
                      <th className="py-2 pr-4">氏名</th>
                      <th className="py-2 pr-4">役割</th>
                      <th className="py-2 pr-4">週間実働時間</th>
                      <th className="py-2 pr-4">操作</th>
                    </tr>
                  </thead>
                  <tbody>
                    {staffStats.map((s) => (
                      <tr key={s.staff.id} className="border-b last:border-0">
                        <td className="py-2 pr-4 font-medium">{s.staff.name}</td>
                        <td className="py-2 pr-4">
                          <span className={`px-2 py-0.5 rounded-full text-xs ${
                            s.staff.role === "責任者" ? "bg-purple-100 text-purple-700" :
                            s.staff.role === "キッチン" ? "bg-orange-100 text-orange-700" :
                            "bg-blue-100 text-blue-700"
                          }`}>
                            {s.staff.role}
                          </span>
                        </td>
                        <td className="py-2 pr-4">
                          <span className={s.over40 ? "text-red-600 font-semibold" : ""}>
                            {formatHours(s.totalMinutes)}
                            {s.over40 && " ⚠️ 40時間超過"}
                          </span>
                        </td>
                        <td className="py-2 pr-4">
                          <button
                            onClick={() => editStaff(s.staff)}
                            className="text-blue-600 hover:underline mr-2 text-xs"
                          >
                            編集
                          </button>
                          <button
                            onClick={() => deleteStaff(s.staff.id)}
                            className="text-red-600 hover:underline text-xs"
                          >
                            削除
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </section>

          {/* Shift Management */}
          <section className="bg-white rounded-xl shadow-sm p-4 md:p-6">
            <h2 className="text-lg font-semibold mb-4">シフト管理</h2>

            <form onSubmit={addShift} className="grid grid-cols-2 md:grid-cols-6 gap-3 mb-4">
              <div className="flex flex-col">
                <label className="text-xs text-gray-500 mb-1">スタッフ</label>
                <select
                  value={shiftStaffId}
                  onChange={(e) => setShiftStaffId(e.target.value)}
                  data-anvil-action="input"
                  className="border rounded px-2 py-1.5 text-sm"
                >
                  <option value="">選択してください</option>
                  {staff.map((s) => (
                    <option key={s.id} value={s.id}>{s.name}（{s.role}）</option>
                  ))}
                </select>
              </div>
              <div className="flex flex-col">
                <label className="text-xs text-gray-500 mb-1">日付</label>
                <input
                  type="date"
                  value={shiftDate}
                  onChange={(e) => setShiftDate(e.target.value)}
                  className="border rounded px-2 py-1.5 text-sm"
                />
              </div>
              <div className="flex flex-col">
                <label className="text-xs text-gray-500 mb-1">開始</label>
                <input
                  type="time"
                  step={1800}
                  value={shiftStart}
                  onChange={(e) => setShiftStart(e.target.value)}
                  className="border rounded px-2 py-1.5 text-sm"
                />
              </div>
              <div className="flex flex-col">
                <label className="text-xs text-gray-500 mb-1">終了</label>
                <input
                  type="time"
                  step={1800}
                  value={shiftEnd}
                  onChange={(e) => setShiftEnd(e.target.value)}
                  className="border rounded px-2 py-1.5 text-sm"
                />
              </div>
              <div className="flex flex-col">
                <label className="text-xs text-gray-500 mb-1">休憩(分)</label>
                <input
                  type="number"
                  min={0}
                  value={shiftBreak}
                  onChange={(e) => setShiftBreak(e.target.value)}
                  className="border rounded px-2 py-1.5 text-sm w-20"
                />
              </div>
              <div className="flex items-end gap-2">
                <button
                  type="submit"
                  data-anvil-action="primary"
                  className="bg-green-600 text-white px-4 py-1.5 rounded text-sm font-medium hover:bg-green-700 transition"
                >
                  {editingShiftId ? "更新" : "追加"}
                </button>
                {editingShiftId && (
                  <button
                    type="button"
                    onClick={() => { setEditingShiftId(null); setShiftInputError(null); }}
                    className="text-gray-500 text-xs hover:text-gray-700"
                  >
                    キャンセル
                  </button>
                )}
              </div>
            </form>

            {shiftInputError && (
              <p className="text-sm text-red-600 mb-3">{shiftInputError}</p>
            )}

            {shifts.length === 0 ? (
              <p className="text-gray-400 text-sm">シフトがありません。追加してください。</p>
            ) : (
              <div className="overflow-x-auto">
                <table className="w-full text-sm">
                  <thead>
                    <tr className="border-b text-left text-gray-500">
                      <th className="py-2 pr-4">スタッフ</th>
                      <th className="py-2 pr-4">日付</th>
                      <th className="py-2 pr-4">開始</th>
                      <th className="py-2 pr-4">終了</th>
                      <th className="py-2 pr-4">休憩</th>
                      <th className="py-2 pr-4">実働</th>
                      <th className="py-2 pr-4">操作</th>
                    </tr>
                  </thead>
                  <tbody>
                    {shifts.sort((a, b) => a.date.localeCompare(b.date) || a.start.localeCompare(b.start)).map((s) => {
                      const st = staff.find((x) => x.id === s.staffId);
                      return (
                        <tr key={s.id} className="border-b last:border-0">
                          <td className="py-2 pr-4">{st ? st.name : "不明"}</td>
                          <td className="py-2 pr-4">{s.date}</td>
                          <td className="py-2 pr-4">{s.start}</td>
                          <td className="py-2 pr-4">{s.end}</td>
                          <td className="py-2 pr-4">{s.breakMinutes}分</td>
                          <td className="py-2 pr-4">{formatHours(workMinutes(s))}</td>
                          <td className="py-2 pr-4">
                            <button
                              onClick={() => editShift(s)}
                              className="text-blue-600 hover:underline mr-2 text-xs"
                            >
                              編集
                            </button>
                            <button
                              onClick={() => deleteShift(s.id)}
                              className="text-red-600 hover:underline text-xs"
                            >
                              削除
                            </button>
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            )}
          </section>

          {/* Filters */}
          <section className="bg-white rounded-xl shadow-sm p-4 md:p-6">
            <h2 className="text-lg font-semibold mb-4">絞り込み</h2>
            <div className="flex flex-wrap gap-3">
              <div className="flex flex-col">
                <label className="text-xs text-gray-500 mb-1">日付</label>
                <select
                  value={filterDate}
                  onChange={(e) => setFilterDate(e.target.value)}
                  className="border rounded px-2 py-1.5 text-sm"
                >
                  <option value="">すべて</option>
                  {weekDates.map((d, i) => (
                    <option key={d} value={d}>{dayLabels[i]}</option>
                  ))}
                </select>
              </div>
              <div className="flex flex-col">
                <label className="text-xs text-gray-500 mb-1">スタッフ</label>
                <select
                  value={filterStaff}
                  onChange={(e) => setFilterStaff(e.target.value)}
                  className="border rounded px-2 py-1.5 text-sm"
                >
                  <option value="">全員</option>
                  {staff.map((s) => (
                    <option key={s.id} value={s.id}>{s.name}</option>
                  ))}
                </select>
              </div>
              <div className="flex flex-col">
                <label className="text-xs text-gray-500 mb-1">役割</label>
                <select
                  value={filterRole}
                  onChange={(e) => setFilterRole(e.target.value)}
                  className="border rounded px-2 py-1.5 text-sm"
                >
                  <option value="">全役割</option>
                  <option value="責任者">責任者</option>
                  <option value="キッチン">キッチン</option>
                  <option value="ホール">ホール</option>
                </select>
              </div>
              {(filterDate || filterStaff || filterRole) && (
                <button
                  onClick={() => { setFilterDate(""); setFilterStaff(""); setFilterRole(""); }}
                  className="text-sm text-gray-500 hover:text-gray-700 self-end mb-1"
                >
                  条件をリセット
                </button>
              )}
            </div>
          </section>

          {/* Weekly Calendar */}
          <section className="bg-white rounded-xl shadow-sm p-4 md:p-6">
            <h2 className="text-lg font-semibold mb-4">週間カレンダー</h2>
            {filteredShifts.length === 0 ? (
              <p className="text-gray-400 text-sm">該当するシフトがありません。</p>
            ) : (
              <div className="overflow-x-auto">
                <table className="w-full text-sm border-collapse">
                  <thead>
                    <tr className="border-b-2 border-gray-200">
                      <th className="py-2 pr-4 text-left text-gray-500">日付</th>
                      <th className="py-2 pr-4 text-left text-gray-500">シフト一覧</th>
                    </tr>
                  </thead>
                  <tbody>
                    {weekDates.map((d, i) => {
                      const dayShifts = filteredShifts.filter((s) => s.date === d);
                      if (dayShifts.length === 0) return null;
                      return (
                        <tr key={d} className="border-b last:border-0">
                          <td className="py-2 pr-4 font-medium whitespace-nowrap">{dayLabels[i]}</td>
                          <td className="py-2">
                            <div className="flex flex-wrap gap-2">
                              {dayShifts.map((s) => {
                                const st = staff.find((x) => x.id === s.staffId);
                                return (
                                  <span
                                    key={s.id}
                                    className={`inline-flex items-center gap-1 px-2 py-1 rounded text-xs ${
                                      st?.role === "責任者" ? "bg-purple-100 text-purple-700" :
                                      st?.role === "キッチン" ? "bg-orange-100 text-orange-700" :
                                      "bg-blue-100 text-blue-700"
                                    }`}
                                  >
                                    {st?.name ?? "不明"}: {s.start}〜{s.end}（休憩{s.breakMinutes}分 / 実働{formatHours(workMinutes(s))}）
                                  </span>
                                );
                              })}
                            </div>
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            )}
          </section>

          {/* Stats: Daily Headcount by Role */}
          <section className="bg-white rounded-xl shadow-sm p-4 md:p-6">
            <h2 className="text-lg font-semibold mb-4">日別・役割別 配置人数</h2>
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b text-left text-gray-500">
                    <th className="py-2 pr-4">日付</th>
                    <th className="py-2 pr-4 text-center">責任者</th>
                    <th className="py-2 pr-4 text-center">キッチン</th>
                    <th className="py-2 pr-4 text-center">ホール</th>
                    <th className="py-2 pr-4 text-center">合計</th>
                  </tr>
                </thead>
                <tbody>
                  {weekDates.map((d, i) => (
                    <tr key={d} className="border-b last:border-0">
                      <td className="py-2 pr-4 font-medium">{dayLabels[i]}</td>
                      <td className="py-2 pr-4 text-center">{dailyHeadcount[d]?.["責任者"] ?? 0}</td>
                      <td className="py-2 pr-4 text-center">{dailyHeadcount[d]?.["キッチン"] ?? 0}</td>
                      <td className="py-2 pr-4 text-center">{dailyHeadcount[d]?.["ホール"] ?? 0}</td>
                      <td className="py-2 pr-4 text-center font-bold">{dailyHeadcount[d]?.["合計"] ?? 0}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </section>

          {/* Stats: Weekly Worked Hours */}
          <section className="bg-white rounded-xl shadow-sm p-4 md:p-6">
            <h2 className="text-lg font-semibold mb-4">スタッフ別 週間実働時間</h2>
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
              {staffStats.map((s) => (
                <div
                  key={s.staff.id}
                  className={`rounded-lg p-4 border ${s.over40 ? "border-red-300 bg-red-50" : "border-gray-200 bg-gray-50"}`}
                >
                  <div className="flex justify-between items-center">
                    <span className="font-medium">{s.staff.name}</span>
                    <span className="text-xs px-2 py-0.5 rounded-full bg-gray-200 text-gray-600">{s.staff.role}</span>
                  </div>
                  <div className={`mt-2 text-2xl font-bold ${s.over40 ? "text-red-600" : "text-gray-800"}`}>
                    {formatHours(s.totalMinutes)}
                  </div>
                  <div className="text-xs text-gray-500 mt-1">
                    登録シフト: {s.shiftCount}件
                    {s.over40 && <span className="text-red-600 font-medium ml-2">⚠️ 40時間超過</span>}
                  </div>
                </div>
              ))}
            </div>
          </section>
        </>
      )}
    </div>
  );
}
