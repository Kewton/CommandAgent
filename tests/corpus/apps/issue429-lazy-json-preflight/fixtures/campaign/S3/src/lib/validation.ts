import { Role, Shift, ValidationResult } from "./types";

/**
 * HH:MM 文字列を分（0〜1439）に変換する。
 * 不正な形式の場合、null を返す。
 */
export function timeToMinutes(time: string): number | null {
  const m = /^(\d{1,2}):(\d{2})$/.exec(time.trim());
  if (!m) return null;
  const h = Number(m[1]);
  const min = Number(m[2]);
  if (h < 0 || h > 23 || min < 0 || min > 59) return null;
  return h * 60 + min;
}

/**
 * 分を HH:MM 文字列に変換する。
 */
export function minutesToTime(mins: number): string {
  const h = Math.floor(mins / 60);
  const m = mins % 60;
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`;
}

/**
 * 指定の値が 30 分の倍数か確認する。
 */
export function isMultipleOf30(value: number): boolean {
  return Number.isInteger(value / 30);
}

/**
 * 日付文字列が YYYY-MM-DD 形式か確認する。
 */
export function isValidDate(date: string): boolean {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(date.trim());
  if (!m) return false;
  const year = Number(m[1]);
  const month = Number(m[2]);
  const day = Number(m[3]);
  if (month < 1 || month > 12) return false;
  if (day < 1 || day > 31) return false;
  const d = new Date(year, month - 1, day);
  return d.getFullYear() === year && d.getMonth() === month - 1 && d.getDate() === day;
}

/**
 * 有効な役割か確認する。
 */
export function isValidRole(role: string): role is Role {
  return role === "責任者" || role === "キッチン" || role === "ホール";
}

/**
 * スタッフ名のバリデーション。
 */
export function validateStaffName(name: unknown): string | null {
  if (typeof name !== "string") return "氏名は必須です。";
  const trimmed = name.trim();
  if (trimmed.length === 0) return "氏名は必須です。";
  if (trimmed.length > 50) return "氏名は50文字以下で入力してください。";
  return null;
}

/**
 * スタッフ作成・編集のバリデーション。
 */
export function validateStaff(input: unknown): ValidationResult {
  const errors: string[] = [];
  if (typeof input !== "object" || input === null) {
    return { valid: false, errors: ["不正な入力です。"] };
  }
  const { name, role } = input as { name?: unknown; role?: unknown };

  const nameError = validateStaffName(name);
  if (nameError) errors.push(nameError);

  if (typeof role !== "string" || !isValidRole(role)) {
    errors.push("役割は「責任者」「キッチン」「ホール」のいずれかを選択してください。");
  }

  return { valid: errors.length === 0, errors };
}

/**
 * シフトのバリデーション。
 * 30分単位、終了>開始、休憩<勤務時間、同一スタッフの重複チェック。
 */
export function validateShift(
  input: unknown,
  existingShifts: Shift[],
  editingShiftId?: string
): ValidationResult {
  const errors: string[] = [];

  if (typeof input !== "object" || input === null) {
    return { valid: false, errors: ["不正な入力です。"] };
  }

  const { staffId, date, start, end, breakMinutes } = input as {
    staffId?: unknown;
    date?: unknown;
    start?: unknown;
    end?: unknown;
    breakMinutes?: unknown;
  };

  // staffId チェック
  if (typeof staffId !== "string" || staffId.trim().length === 0) {
    errors.push("スタッフを選択してください。");
  }

  // 日付チェック
  if (typeof date !== "string" || !isValidDate(date)) {
    errors.push("日付を正しく入力してください（YYYY-MM-DD）。");
  }

  // 開始時刻・終了時刻チェック
  const startMins = typeof start === "string" ? timeToMinutes(start) : null;
  const endMins = typeof end === "string" ? timeToMinutes(end) : null;

  if (startMins === null) {
    errors.push("開始時刻を正しく入力してください（HH:MM）。");
  }
  if (endMins === null) {
    errors.push("終了時刻を正しく入力してください（HH:MM）。");
  }

  // 30分単位のチェック
  if (startMins !== null && !isMultipleOf30(startMins)) {
    errors.push("開始時刻は30分単位で入力してください。");
  }
  if (endMins !== null && !isMultipleOf30(endMins)) {
    errors.push("終了時刻は30分単位で入力してください。");
  }

  // 終了 > 開始
  if (startMins !== null && endMins !== null && endMins <= startMins) {
    errors.push("終了時刻は開始時刻より後にしてください。");
  }

  // 休憩分数チェック
  let breakMins = 0;
  if (typeof breakMinutes !== "number" && typeof breakMinutes !== "string") {
    errors.push("休憩分数を入力してください。");
  } else {
    breakMins = Number(breakMinutes);
    if (isNaN(breakMins)) {
      errors.push("休憩分数は数値で入力してください。");
    } else if (breakMins < 0) {
      errors.push("休憩分数は0分以上で入力してください。");
    } else if (startMins !== null && endMins !== null && breakMins >= endMins - startMins) {
      errors.push("休憩分数は勤務時間より短くしてください。");
    }
  }

  // 30分単位の休憩チェック
  if (typeof breakMinutes === "number" && !isMultipleOf30(breakMins)) {
    errors.push("休憩分数は30分単位で入力してください。");
  }

  // 同一スタッフの重複チェック（同一日）
  if (
    typeof staffId === "string" &&
    staffId.trim().length > 0 &&
    typeof date === "string" &&
    isValidDate(date) &&
    startMins !== null &&
    endMins !== null
  ) {
    const sameDayShifts = existingShifts.filter(
      (s) =>
        s.staffId === staffId &&
        s.date === date &&
        s.id !== editingShiftId
    );

    for (const s of sameDayShifts) {
      const sStart = timeToMinutes(s.start);
      const sEnd = timeToMinutes(s.end);
      if (sStart === null || sEnd === null) continue;

      // 重複: start < sEnd && sStart < end（隣接 end==next start は許可）
      if (startMins < sEnd && sStart < endMins) {
        errors.push(
          `「${s.start}〜${s.end}」のシフトと重なります。時間帯を調整してください。`
        );
        break;
      }
    }
  }

  return { valid: errors.length === 0, errors };
}
