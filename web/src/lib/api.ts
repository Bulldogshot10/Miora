import { invoke } from "@tauri-apps/api/core";
import type { DashboardSnapshot, DailyCheckinInput } from "./types";
import { EMPTY_SNAPSHOT } from "./types";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

const PERIOD_STORAGE_KEY = "miora.periodStarts.v1";
const CHECKIN_STORAGE_KEY = "miora.checkins.v1";

function runningInTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function readStoredDates(): string[] {
  try {
    const raw = localStorage.getItem(PERIOD_STORAGE_KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed)
      ? parsed.filter((value): value is string => typeof value === "string").sort()
      : [];
  } catch {
    return [];
  }
}

function readCheckinCount(): number {
  try {
    const raw = localStorage.getItem(CHECKIN_STORAGE_KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed) ? parsed.length : 0;
  } catch {
    return 0;
  }
}

function browserSnapshot(): DashboardSnapshot {
  const periodStarts = readStoredDates();
  const latest = periodStarts.at(-1) ?? null;
  const today = new Date();
  const todayIso = [
    today.getFullYear(),
    String(today.getMonth() + 1).padStart(2, "0"),
    String(today.getDate()).padStart(2, "0"),
  ].join("-");

  const cycleDay = latest
    ? Math.floor(
        (new Date(todayIso + "T12:00:00").getTime() -
          new Date(latest + "T12:00:00").getTime()) /
          86_400_000,
      ) + 1
    : null;

  const lengths = periodStarts
    .slice(1)
    .map((date, index) => {
      const previous = new Date(periodStarts[index] + "T12:00:00");
      const current = new Date(date + "T12:00:00");
      return Math.round((current.getTime() - previous.getTime()) / 86_400_000);
    })
    .filter((days) => days >= 15 && days <= 60);

  const averageCycleLength =
    lengths.length > 0
      ? lengths.reduce((sum, days) => sum + days, 0) / lengths.length
      : null;

  let expectedNextPeriodStart: string | null = null;
  let earliestNextPeriodStart: string | null = null;
  let latestNextPeriodStart: string | null = null;

  if (latest && lengths.length >= 2 && averageCycleLength !== null) {
    const expected = new Date(latest + "T12:00:00");
    expected.setDate(expected.getDate() + Math.round(averageCycleLength));
    expectedNextPeriodStart = toLocalIso(expected);

    const sorted = [...lengths].sort((a, b) => a - b);
    const median = sorted[Math.floor(sorted.length / 2)];
    const radius = Math.max(1, Math.ceil(Math.abs(averageCycleLength - median) * 1.5));
    const earliest = new Date(latest + "T12:00:00");
    const latestDate = new Date(latest + "T12:00:00");
    earliest.setDate(earliest.getDate() + Math.max(1, Math.round(averageCycleLength) - radius));
    latestDate.setDate(latestDate.getDate() + Math.round(averageCycleLength) + radius);
    earliestNextPeriodStart = toLocalIso(earliest);
    latestNextPeriodStart = toLocalIso(latestDate);
  }

  return {
    ...EMPTY_SNAPSHOT,
    lastPeriodStart: latest,
    cycleDay: cycleDay !== null && cycleDay > 0 ? cycleDay : null,
    expectedNextPeriodStart,
    earliestNextPeriodStart,
    latestNextPeriodStart,
    cyclesTracked: lengths.length,
    averageCycleLength,
    checkinsTracked: readCheckinCount(),
    periodStarts,
  };
}

function toLocalIso(date: Date): string {
  return [
    date.getFullYear(),
    String(date.getMonth() + 1).padStart(2, "0"),
    String(date.getDate()).padStart(2, "0"),
  ].join("-");
}

export async function getDashboardSnapshot(): Promise<DashboardSnapshot> {
  if (runningInTauri()) {
    return invoke<DashboardSnapshot>("get_dashboard_snapshot");
  }
  return browserSnapshot();
}

export async function logPeriodStart(date: string): Promise<DashboardSnapshot> {
  if (runningInTauri()) {
    return invoke<DashboardSnapshot>("log_period_start", { date });
  }

  const starts = readStoredDates();
  if (!starts.includes(date)) {
    starts.push(date);
    starts.sort();
    localStorage.setItem(PERIOD_STORAGE_KEY, JSON.stringify(starts));
  }
  return browserSnapshot();
}

export async function saveDailyCheckin(
  input: DailyCheckinInput,
): Promise<DashboardSnapshot> {
  if (runningInTauri()) {
    return invoke<DashboardSnapshot>("save_daily_checkin", { input });
  }

  try {
    const raw = localStorage.getItem(CHECKIN_STORAGE_KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : [];
    const checkins = Array.isArray(parsed) ? parsed : [];
    const updated = checkins.filter(
      (entry: unknown) =>
        typeof entry !== "object" ||
        entry === null ||
        !("date" in entry) ||
        entry.date !== input.date,
    );
    updated.push(input);
    localStorage.setItem(CHECKIN_STORAGE_KEY, JSON.stringify(updated));
  } catch {
    localStorage.setItem(CHECKIN_STORAGE_KEY, JSON.stringify([input]));
  }
  return browserSnapshot();
}
