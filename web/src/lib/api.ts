import { invoke } from "@tauri-apps/api/core";
import type {
  BleedingPrompt,
  DashboardSnapshot,
  DailyCheckinInput,
  PhaseProbabilities,
} from "./types";
import { EMPTY_SNAPSHOT } from "./types";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

const PERIOD_STORAGE_KEY = "miora.periodStarts.v1";
const PERIOD_END_STORAGE_KEY = "miora.periodEnds.v1";
const BLEEDING_STATE_KEY = "miora.bleedingState.v1";
const CHECKIN_STORAGE_KEY = "miora.checkins.v1";

type LocalBleedingState = {
  lastObservation: string | null;
  endDate: string | null;
  confirmed: boolean;
};

const BLEEDING_PRIOR = [0.25, 0.75, 1.5, 2.0, 1.8, 1.2, 0.6, 0.3, 0.1];
const MIN_CYCLE_LENGTH = 21;
const MAX_CYCLE_LENGTH = 45;

function runningInTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function toLocalIso(date: Date): string {
  return [
    date.getFullYear(),
    String(date.getMonth() + 1).padStart(2, "0"),
    String(date.getDate()).padStart(2, "0"),
  ].join("-");
}

function localDayDifference(start: string, end: string): number {
  const a = new Date(start + "T12:00:00");
  const b = new Date(end + "T12:00:00");
  return Math.round((b.getTime() - a.getTime()) / 86_400_000);
}

function addLocalDays(date: string, amount: number): string {
  const value = new Date(date + "T12:00:00");
  value.setDate(value.getDate() + amount);
  return toLocalIso(value);
}

function readStoredDates(): string[] {
  try {
    const raw = localStorage.getItem(PERIOD_STORAGE_KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed)
      ? [...new Set(parsed.filter((value): value is string => typeof value === "string"))].sort()
      : [];
  } catch {
    return [];
  }
}

function readPeriodEnds(): Record<string, string> {
  try {
    const raw = localStorage.getItem(PERIOD_END_STORAGE_KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : {};
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) return {};
    return Object.fromEntries(
      Object.entries(parsed).filter(
        ([start, end]) =>
          /^\d{4}-\d{2}-\d{2}$/.test(start) &&
          typeof end === "string" &&
          /^\d{4}-\d{2}-\d{2}$/.test(end),
      ),
    );
  } catch {
    return {};
  }
}

function readBleedingStates(): Record<string, LocalBleedingState> {
  try {
    const raw = localStorage.getItem(BLEEDING_STATE_KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : {};
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) return {};
    const entries = Object.entries(parsed).filter(
      ([start, value]) =>
        /^\d{4}-\d{2}-\d{2}$/.test(start) &&
        typeof value === "object" &&
        value !== null &&
        "confirmed" in value,
    );
    return Object.fromEntries(entries) as Record<string, LocalBleedingState>;
  } catch {
    return {};
  }
}

function isDailyCheckin(value: unknown): value is DailyCheckinInput {
  if (typeof value !== "object" || value === null) return false;
  const entry = value as Record<string, unknown>;
  return (
    typeof entry.date === "string" &&
    typeof entry.moodScore === "number" &&
    typeof entry.energyScore === "number" &&
    Array.isArray(entry.symptoms)
  );
}

function readStoredCheckins(): DailyCheckinInput[] {
  try {
    const raw = localStorage.getItem(CHECKIN_STORAGE_KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed) ? parsed.filter(isDailyCheckin) : [];
  } catch {
    return [];
  }
}

function cycleLengthDistribution(lengths: number[]): Map<number, number> {
  const counts = new Map<number, number>();
  for (const length of lengths) {
    if (length >= MIN_CYCLE_LENGTH && length <= MAX_CYCLE_LENGTH) {
      counts.set(length, (counts.get(length) ?? 0) + 1);
    }
  }

  const priorPerDay = 0.02;
  const denominator = lengths.filter(
    (length) => length >= MIN_CYCLE_LENGTH && length <= MAX_CYCLE_LENGTH,
  ).length + priorPerDay * (MAX_CYCLE_LENGTH - MIN_CYCLE_LENGTH + 1);
  const distribution = new Map<number, number>();

  for (let day = MIN_CYCLE_LENGTH; day <= MAX_CYCLE_LENGTH; day += 1) {
    distribution.set(day, ((counts.get(day) ?? 0) + priorPerDay) / denominator);
  }
  return distribution;
}

function bleedingDistribution(durations: number[]): Map<number, number> {
  const counts = new Map<number, number>();
  for (const duration of durations) {
    if (duration >= 2 && duration <= 10) {
      counts.set(duration, (counts.get(duration) ?? 0) + 1);
    }
  }

  const denominator =
    durations.filter((duration) => duration >= 2 && duration <= 10).length +
    BLEEDING_PRIOR.reduce((sum, weight) => sum + weight, 0);
  const distribution = new Map<number, number>();

  for (let day = 2; day <= 10; day += 1) {
    const prior = BLEEDING_PRIOR[day - 2];
    distribution.set(day, ((counts.get(day) ?? 0) + prior) / denominator);
  }
  return distribution;
}

function medianOfDistribution(distribution: Map<number, number>): number {
  const entries = [...distribution.entries()].sort((a, b) => a[0] - b[0]);
  let cumulative = 0;
  for (const [value, probability] of entries) {
    cumulative += probability;
    if (cumulative >= 0.5) return value;
  }
  return entries.at(-1)?.[0] ?? 5;
}

function makePhaseEstimate(
  periodStarts: string[],
  bleedingDurations: number[],
  today: string,
  confirmedCurrentBleedingDays: number | null,
  observedBleedingThroughDay: number | null,
): CyclePhaseSnapshot | null {
  const currentStart = [...periodStarts].filter((date) => date <= today).at(-1);
  if (!currentStart) return null;

  const cycleDay = localDayDifference(currentStart, today) + 1;
  const intervals = periodStarts
    .slice(1)
    .map((date, index) => localDayDifference(periodStarts[index], date))
    .filter((days) => days >= MIN_CYCLE_LENGTH && days <= MAX_CYCLE_LENGTH);

  const cycleDistribution = cycleLengthDistribution(intervals);
  const bleedDistribution = confirmedCurrentBleedingDays !== null
    ? new Map([[Math.max(1, Math.min(10, confirmedCurrentBleedingDays)), 1]])
    : bleedingDistribution(bleedingDurations);

  if (observedBleedingThroughDay !== null && cycleDay <= observedBleedingThroughDay) {
    return {
      code: "menstrual",
      label: "Menstrual phase",
      cycleDay,
      estimatedCycleLength: 0,
      completedIntervalsUsed: intervals.length,
      completedBleedingDurationsUsed: bleedingDurations.length,
      confidence: 1,
      probabilities: { menstrual: 1, follicular: 0, ovulationWindow: 0, luteal: 0 },
      isPersonalized: confirmedCurrentBleedingDays !== null,
      description: "Bleeding has been confirmed for this date.",
    };
  }

  const probabilities: PhaseProbabilities = {
    menstrual: 0,
    follicular: 0,
    ovulationWindow: 0,
    luteal: 0,
  };

  for (const [cycleLength, cycleWeight] of cycleDistribution) {
    for (const [bleedingDays, bleedingWeight] of bleedDistribution) {
      for (let lutealDays = 12; lutealDays <= 16; lutealDays += 1) {
        const weight = cycleWeight * bleedingWeight * 0.2;
        const ovulationDay = Math.max(1, cycleLength - lutealDays);

        if (cycleDay <= bleedingDays) {
          probabilities.menstrual += weight;
        } else if (cycleDay + 1 < ovulationDay) {
          probabilities.follicular += weight;
        } else if (cycleDay <= ovulationDay + 1) {
          probabilities.ovulationWindow += weight;
        } else {
          probabilities.luteal += weight;
        }
      }
    }
  }

  const total = Object.values(probabilities).reduce((sum, value) => sum + value, 0);
  if (total > 0) {
    probabilities.menstrual /= total;
    probabilities.follicular /= total;
    probabilities.ovulationWindow /= total;
    probabilities.luteal /= total;
  }

  const candidates: [keyof PhaseProbabilities, number, CyclePhaseSnapshot["code"], string, string][] = [
    ["menstrual", probabilities.menstrual, "menstrual", "Menstrual phase", "The model estimates that this day falls within the menstrual phase."],
    ["follicular", probabilities.follicular, "follicular", "Follicular phase", "The model estimates that this day is most likely in the follicular phase, before ovulation."],
    ["ovulationWindow", probabilities.ovulationWindow, "ovulation_window", "Estimated ovulation window", "The model estimates that this day is most likely near ovulation. Calendar history cannot confirm ovulation itself."],
    ["luteal", probabilities.luteal, "luteal", "Luteal phase", "The model estimates that this day is most likely in the luteal phase, after ovulation."],
  ];
  const mostLikely = candidates.reduce((best, candidate) => candidate[1] > best[1] ? candidate : best);
  const confidence = mostLikely[1];
  const learning = confidence < 0.55;
  const expectedCycleLength = intervals.length > 0
    ? [...cycleDistribution.entries()].reduce((sum, [day, probability]) => sum + day * probability, 0)
    : 0;

  return {
    code: learning ? "learning" : mostLikely[2],
    label: learning ? "Still learning her cycle" : mostLikely[3],
    cycleDay,
    estimatedCycleLength: Math.round(expectedCycleLength),
    completedIntervalsUsed: intervals.length,
    completedBleedingDurationsUsed: bleedingDurations.length,
    confidence,
    probabilities,
    isPersonalized: intervals.length > 0 && bleedingDurations.length > 0,
    description: learning
      ? "The phase probabilities are still spread across several possibilities. More confirmed cycle and bleeding observations will improve the estimate."
      : mostLikely[4],
  };
}

function browserSnapshot(): DashboardSnapshot {
  const periodStarts = readStoredDates();
  const periodEnds = readPeriodEnds();
  const bleedingStates = readBleedingStates();
  const checkins = readStoredCheckins();
  const latest = periodStarts.at(-1) ?? null;
  const today = toLocalIso(new Date());

  const lengths = periodStarts
    .slice(1)
    .map((date, index) => localDayDifference(periodStarts[index], date))
    .filter((days) => days >= 15 && days <= 60);

  const averageCycleLength = lengths.length > 0
    ? lengths.reduce((sum, days) => sum + days, 0) / lengths.length
    : null;

  let expectedNextPeriodStart: string | null = null;
  let earliestNextPeriodStart: string | null = null;
  let latestNextPeriodStart: string | null = null;

  if (latest && lengths.length >= 2 && averageCycleLength !== null) {
    const sorted = [...lengths].sort((a, b) => a - b);
    const center = sorted[Math.floor(sorted.length / 2)];
    const deviations = sorted.map((length) => Math.abs(length - center)).sort((a, b) => a - b);
    const mad = deviations[Math.floor(deviations.length / 2)] ?? 0;
    const radius = Math.max(1, Math.ceil(mad * 1.4826));
    expectedNextPeriodStart = addLocalDays(latest, Math.round(center));
    earliestNextPeriodStart = addLocalDays(latest, Math.max(1, Math.round(center) - radius));
    latestNextPeriodStart = addLocalDays(latest, Math.round(center) + radius);
  }

  const completedDurations = periodStarts.flatMap((start) => {
    const end = periodEnds[start];
    const state = bleedingStates[start];
    if (!end || end < start || state?.confirmed === false) return [];
    const duration = localDayDifference(start, end) + 1;
    return duration >= 1 && duration <= 14 ? [duration] : [];
  });

  const latestState = latest ? bleedingStates[latest] : undefined;
  const latestEnd = latest ? periodEnds[latest] : undefined;
  const confirmedCurrentDuration = latestEnd && latestState?.confirmed !== false
    ? localDayDifference(latest, latestEnd) + 1
    : null;

  const observedThroughDay =
    latestState && !latestState.confirmed && latestState.lastObservation === today && latest
      ? localDayDifference(latest, latestState.lastObservation) + 1
      : null;

  const phase = makePhaseEstimate(
    periodStarts,
    completedDurations,
    today,
    confirmedCurrentDuration,
    observedThroughDay,
  );

  let bleedingPrompt: BleedingPrompt | null = null;
  const latestEndAlreadyConfirmed =
    Boolean(latestEnd) && latestState?.confirmed !== false;

  if (latest && !latestEndAlreadyConfirmed && latestState?.confirmed !== true) {
    const cycleDay = localDayDifference(latest, today) + 1;
    const durationPosterior = bleedingDistribution(completedDurations);
    const predictedDuration = medianOfDistribution(durationPosterior);
    const alreadyAskedToday = latestState?.lastObservation === today;

    if (cycleDay >= predictedDuration && !alreadyAskedToday) {
      bleedingPrompt = {
        periodStart: latest,
        checkDate: today,
        predictedBleedingDurationDays: predictedDuration,
        completedBleedingDurationsUsed: completedDurations.length,
      };
    }
  }

  return {
    ...EMPTY_SNAPSHOT,
    lastPeriodStart: latest,
    cycleDay: latest ? localDayDifference(latest, today) + 1 : null,
    expectedNextPeriodStart,
    earliestNextPeriodStart,
    latestNextPeriodStart,
    cyclesTracked: lengths.length,
    averageCycleLength,
    checkinsTracked: checkins.length,
    periodStarts,
    checkinDates: [...new Set(checkins.map((entry) => entry.date))].sort(),
    phase,
    bleedingPrompt,
  };
}

export async function getDashboardSnapshot(): Promise<DashboardSnapshot> {
  if (runningInTauri()) {
    return invoke<DashboardSnapshot>("get_dashboard_snapshot");
  }
  return browserSnapshot();
}

export async function logPeriodStart(
  date: string,
  endDate: string | null = null,
): Promise<DashboardSnapshot> {
  if (runningInTauri()) {
    return invoke<DashboardSnapshot>("log_period_start", { date, endDate });
  }

  const starts = readStoredDates();
  if (!starts.includes(date)) {
    starts.push(date);
    starts.sort();
    localStorage.setItem(PERIOD_STORAGE_KEY, JSON.stringify(starts));
  }

  if (endDate) {
    if (endDate < date) throw new Error("The last bleeding day must be on or after the first day.");
    const ends = readPeriodEnds();
    ends[date] = endDate;
    localStorage.setItem(PERIOD_END_STORAGE_KEY, JSON.stringify(ends));
    const states = readBleedingStates();
    states[date] = { lastObservation: endDate, endDate, confirmed: true };
    localStorage.setItem(BLEEDING_STATE_KEY, JSON.stringify(states));
  }

  return browserSnapshot();
}

export async function reportBleedingStatus(
  periodStart: string,
  date: string,
  stillBleeding: boolean,
): Promise<DashboardSnapshot> {
  if (runningInTauri()) {
    return invoke<DashboardSnapshot>("report_bleeding_status", {
      periodStart,
      date,
      stillBleeding,
    });
  }

  const today = toLocalIso(new Date());
  if (date !== today) throw new Error("Refresh the dashboard before answering the bleeding check.");

  const states = readBleedingStates();
  const ends = readPeriodEnds();
  const existing = states[periodStart] ?? {
    lastObservation: null,
    endDate: null,
    confirmed: false,
  };

  if (stillBleeding) {
    states[periodStart] = {
      lastObservation: today,
      endDate: today,
      confirmed: false,
    };
    ends[periodStart] = today;
  } else {
    const lastBleedingDay = existing.lastObservation ?? addLocalDays(today, -1);
    states[periodStart] = {
      lastObservation: existing.lastObservation,
      endDate: lastBleedingDay,
      confirmed: true,
    };
    ends[periodStart] = lastBleedingDay;
  }

  localStorage.setItem(BLEEDING_STATE_KEY, JSON.stringify(states));
  localStorage.setItem(PERIOD_END_STORAGE_KEY, JSON.stringify(ends));
  return browserSnapshot();
}

export async function getDailyCheckin(date: string): Promise<DailyCheckinInput | null> {
  if (runningInTauri()) {
    return invoke<DailyCheckinInput | null>("get_daily_checkin", { date });
  }
  return readStoredCheckins().find((entry) => entry.date === date) ?? null;
}

export async function saveDailyCheckin(
  input: DailyCheckinInput,
): Promise<DashboardSnapshot> {
  if (runningInTauri()) {
    return invoke<DashboardSnapshot>("save_daily_checkin", { input });
  }

  const checkins = readStoredCheckins().filter((entry) => entry.date !== input.date);
  checkins.push(input);
  checkins.sort((a, b) => a.date.localeCompare(b.date));
  localStorage.setItem(CHECKIN_STORAGE_KEY, JSON.stringify(checkins));
  return browserSnapshot();
}
