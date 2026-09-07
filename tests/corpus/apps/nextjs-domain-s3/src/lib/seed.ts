import type { Staff, Shift } from "./types";

/**
 * Generate sample staff and shift data for the current week.
 * Used by API routes on first GET when the data store is empty.
 */

let idCounter = 0;
function nextId(prefix: string): string {
  idCounter++;
  return `${prefix}-${Date.now()}-${idCounter}`;
}

function getMonday(d = new Date()): Date {
  const date = new Date(d);
  const day = date.getDay();
  const diff = day === 0 ? -6 : 1 - day; // adjust to Monday
  date.setDate(date.getDate() + diff);
  date.setHours(0, 0, 0, 0);
  return date;
}

function dateStr(offset: number): string {
  const monday = getMonday();
  monday.setDate(monday.getDate() + offset);
  return monday.toISOString().split("T")[0];
}

export function seedData(): { staff: Staff[]; shifts: Shift[] } {
  const staffId1 = nextId("staff");
  const staffId2 = nextId("staff");
  const staffId3 = nextId("staff");

  const staff: Staff[] = [
    { id: staffId1, name: "田中 太郎", role: "責任者" },
    { id: staffId2, name: "佐藤 花子", role: "キッチン" },
    { id: staffId3, name: "鈴木 一郎", role: "ホール" },
  ];

  const shifts: Shift[] = [
    // Monday
    { id: nextId("shift"), staffId: staffId1, date: dateStr(0), startTime: "09:00", endTime: "18:00", breakMinutes: 60 },
    { id: nextId("shift"), staffId: staffId2, date: dateStr(0), startTime: "10:00", endTime: "15:00", breakMinutes: 0 },
    // Tuesday
    { id: nextId("shift"), staffId: staffId3, date: dateStr(1), startTime: "10:00", endTime: "17:00", breakMinutes: 60 },
    { id: nextId("shift"), staffId: staffId1, date: dateStr(1), startTime: "13:00", endTime: "20:00", breakMinutes: 60 },
    // Wednesday
    { id: nextId("shift"), staffId: staffId2, date: dateStr(2), startTime: "09:00", endTime: "14:00", breakMinutes: 0 },
    { id: nextId("shift"), staffId: staffId3, date: dateStr(2), startTime: "14:00", endTime: "20:00", breakMinutes: 60 },
    // Thursday
    { id: nextId("shift"), staffId: staffId1, date: dateStr(3), startTime: "09:00", endTime: "18:00", breakMinutes: 60 },
    // Friday
    { id: nextId("shift"), staffId: staffId2, date: dateStr(4), startTime: "10:00", endTime: "16:00", breakMinutes: 30 },
    { id: nextId("shift"), staffId: staffId3, date: dateStr(4), startTime: "12:00", endTime: "18:00", breakMinutes: 60 },
  ];

  return { staff, shifts };
}
