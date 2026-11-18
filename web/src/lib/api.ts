import { invoke } from "@tauri-apps/api/core";
import type {
  BleedingPrompt,
  CyclePhaseSnapshot,
  DashboardSnapshot,
  DailyCheckinInput,
  PhaseProbabilities,
  SymptomPatternSnapshot,
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

const BLEEDING_PRIOR = [0.05, 0.25, 0.75, 1.5, 2.0, 1.8, 1.2, 0.6, 0.3, 0.1, 0.05, 0.02, 0.01, 0.01];
const MIN_CYCLE_LENGTH = 21;
const MAX_CYCLE_LENGTH = 45;
const VALID_SYMPTOM_CODES = new Set([
  "cramps",
  "bloating",
  "headache",
  "fatigue",
  "mood_changes",
]);

function isIsoDate(value: unknown): value is string {
  if (typeof value !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
  const [year, month, day] = value.split("-").map(Number);
  if (year < 1 || year > 9999) return false;
  const parsed = new Date(0);
  parsed.setUTCFullYear(year, month - 1, day);
  parsed.setUTCHours(0, 0, 0, 0);
  return parsed.getUTCFullYear() === year &&
    parsed.getUTCMonth() === month - 1 &&
    parsed.getUTCDate() === day;
}

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
      ? [...new Set(parsed.filter(isIsoDate))].sort()
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
          isIsoDate(start) &&
          isIsoDate(end),
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
        isIsoDate(start) &&
        typeof value === "object" &&
        value !== null &&
        "confirmed" in value &&
        typeof value.confirmed === "boolean" &&
        "lastObservation" in value &&
        (value.lastObservation === null || isIsoDate(value.lastObservation)) &&
        "endDate" in value &&
        (value.endDate === null || isIsoDate(value.endDate)),
    );
    return Object.fromEntries(entries) as Record<string, LocalBleedingState>;
  } catch {
    return {};
  }
}

function isDailyCheckin(value: unknown): value is DailyCheckinInput {
  if (typeof value !== "object" || value === null) return false;
  const entry = value as Record<string, unknown>;
  if (
    !isIsoDate(entry.date) ||
    !Number.isInteger(entry.moodScore) ||
    !Number.isInteger(entry.energyScore) ||
    (entry.moodScore as number) < 1 ||
    (entry.moodScore as number) > 5 ||
    (entry.energyScore as number) < 1 ||
    (entry.energyScore as number) > 5 ||
    !Array.isArray(entry.symptoms)
  ) {
    return false;
  }

  const seen = new Set<string>();
  return entry.symptoms.every((symptom: unknown) => {
    if (typeof symptom !== "object" || symptom === null) return false;
    const item = symptom as Record<string, unknown>;
    if (
      typeof item.code !== "string" ||
      !VALID_SYMPTOM_CODES.has(item.code) ||
      !Number.isInteger(item.severity) ||
      (item.severity as number) < 1 ||
      (item.severity as number) > 3 ||
      seen.has(item.code)
    ) {
      return false;
    }
    seen.add(item.code);
    return true;
  });
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
    if (duration >= 1 && duration <= 14) {
      counts.set(duration, (counts.get(duration) ?? 0) + 1);
    }
  }

  const denominator =
    durations.filter((duration) => duration >= 1 && duration <= 14).length +
    BLEEDING_PRIOR.reduce((sum, weight) => sum + weight, 0);
  const distribution = new Map<number, number>();

  for (let day = 1; day <= 14; day += 1) {
    const prior = BLEEDING_PRIOR[day - 1];
    distribution.set(day, ((counts.get(day) ?? 0) + prior) / denominator);
  }
  return distribution;
}

function median(values: number[]): number | null {
  if (values.length === 0) return null;
  const sorted = [...values].sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0
    ? (sorted[middle - 1] + sorted[middle]) / 2
    : sorted[middle];
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
    ? new Map([[Math.max(1, confirmedCurrentBleedingDays), 1]])
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
      probabilities: { menstrual: 1, follicular: 0, ovulationWindow: 0, luteal: 0, uncertain: 0 },
      isPersonalized: intervals.length > 0 || confirmedCurrentBleedingDays !== null || bleedingDurations.length > 0,
      cycleHistoryAvailable: intervals.length > 0,
      bleedingHistoryAvailable: confirmedCurrentBleedingDays !== null || bleedingDurations.length > 0,
      description: "Bleeding has been confirmed for this date.",
    };
  }

  const probabilities: PhaseProbabilities = {
    menstrual: 0,
    follicular: 0,
    ovulationWindow: 0,
    luteal: 0,
    uncertain: 0,
  };

  for (const [cycleLength, cycleWeight] of cycleDistribution) {
    for (const [bleedingDays, bleedingWeight] of bleedDistribution) {
      for (let lutealDays = 12; lutealDays <= 16; lutealDays += 1) {
        const weight = cycleWeight * bleedingWeight * 0.2;
        const ovulationDay = Math.max(1, cycleLength - lutealDays);

        if (cycleDay <= bleedingDays) {
          probabilities.menstrual += weight;
        } else if (cycleDay > cycleLength) {
          probabilities.uncertain += weight;
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
    probabilities.uncertain /= total;
  }

  const candidates: [keyof PhaseProbabilities, number, CyclePhaseSnapshot["code"], string, string][] = [
    ["menstrual", probabilities.menstrual, "menstrual", "Menstrual phase", "The model estimates that this day falls within the menstrual phase."],
    ["follicular", probabilities.follicular, "follicular", "Follicular phase", "The model estimates that this day is most likely in the follicular phase, before ovulation."],
    ["ovulationWindow", probabilities.ovulationWindow, "ovulation_window", "Estimated ovulation window", "The model estimates that this day is most likely near ovulation. Calendar history cannot confirm ovulation itself."],
    ["luteal", probabilities.luteal, "luteal", "Luteal phase", "The model estimates that this day is most likely in the luteal phase, after ovulation."],
    ["uncertain", probabilities.uncertain, "uncertain", "Cycle timing uncertain", "The current cycle day is beyond the lengths represented by the model. More recent period observations are needed to estimate the phase."],
  ];
  const mostLikely = candidates.reduce((best, candidate) => candidate[1] >= best[1] ? candidate : best);
  const confidence = mostLikely[1];
  const learning = confidence < 0.55;
  const expectedCycleLength = intervals.length > 0
    ? [...cycleDistribution.entries()].reduce((sum, [day, probability]) => sum + day * probability, 0)
    : 0;

  return {
    code: learning ? "learning" : mostLikely[2],
    label: learning ? "Still learning your cycle" : mostLikely[3],
    cycleDay,
    estimatedCycleLength: Math.round(expectedCycleLength),
    completedIntervalsUsed: intervals.length,
    completedBleedingDurationsUsed: bleedingDurations.length,
    confidence,
    probabilities,
    isPersonalized: intervals.length > 0 || confirmedCurrentBleedingDays !== null || bleedingDurations.length > 0,
    cycleHistoryAvailable: intervals.length > 0,
    bleedingHistoryAvailable: confirmedCurrentBleedingDays !== null || bleedingDurations.length > 0,
    description: learning
      ? "The phase probabilities are still spread across several possibilities. More confirmed cycle and bleeding observations will improve the estimate."
      : mostLikely[4],
  };
}


function buildSymptomPatterns(
  checkins: DailyCheckinInput[],
  periodStarts: string[],
  targetCycleDay: number,
): SymptomPatternSnapshot[] {
  const codes = [
    ["cramps", "Cramps"],
    ["bloating", "Bloating"],
    ["headache", "Headache"],
    ["fatigue", "Fatigue"],
    ["mood_changes", "Mood changes"],
  ] as const;

  return codes.flatMap(([code, label]) => {
    const observations = checkins.flatMap((checkin) => {
      const start = periodStarts.filter((date) => date <= checkin.date).at(-1);
      if (!start) return [];
      const cycleDay = localDayDifference(start, checkin.date) + 1;
      if (cycleDay !== targetCycleDay) return [];
      return [{
        occurred: checkin.symptoms.some((symptom) => symptom.code === code),
      }];
    });

    if (observations.length === 0) return [];
    const positive = observations.filter((entry) => entry.occurred).length;
    return [{
      code,
      label,
      probability: (positive + 1) / (observations.length + 2),
      observationsUsed: observations.length,
    }];
  });
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
    const center = median(sorted) ?? 0;
    const deviations = sorted.map((length) => Math.abs(length - center));
    const mad = median(deviations) ?? 0;
    const radius = Math.max(1, Math.ceil(mad * 1.4826));
    expectedNextPeriodStart = addLocalDays(latest, Math.round(center));
    earliestNextPeriodStart = addLocalDays(latest, Math.max(0, Math.round(center) - radius));
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
  const confirmedCurrentDuration = latest && latestEnd && latestState?.confirmed !== false
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
  const symptomPatterns: SymptomPatternSnapshot[] = phase
    ? buildSymptomPatterns(checkins, periodStarts, phase.cycleDay)
    : [];

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
    symptomPatterns,
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

  const today = toLocalIso(new Date());
  if (!isIsoDate(date)) throw new Error("Choose a valid period start date.");
  if (date > today) throw new Error("A period start date cannot be in the future.");

  const starts = readStoredDates();
  const ends = readPeriodEnds();
  const states = readBleedingStates();
  const overlapsPrevious = starts.some((start) => start < date && ends[start] && ends[start] >= date);
  if (overlapsPrevious) {
    throw new Error("The new period start overlaps a previously recorded bleeding interval.");
  }

  if (endDate) {
    if (!isIsoDate(endDate)) throw new Error("Choose a valid last bleeding date.");
    if (endDate < date) throw new Error("The last bleeding day must be on or after the first day.");
    if (endDate > today) throw new Error("The last bleeding day cannot be in the future.");
    const previousObservation = [states[date]?.lastObservation, ends[date]]
      .filter((value): value is string => Boolean(value))
      .sort()
      .at(-1);
    if (previousObservation && endDate < previousObservation) {
      throw new Error("The last bleeding day cannot be earlier than a previously recorded bleeding observation.");
    }
    const nextStart = starts.find((start) => start > date);
    if (nextStart && endDate >= nextStart) {
      throw new Error("The last bleeding day must be before the next recorded period start.");
    }
  }

  if (!starts.includes(date)) {
    starts.push(date);
    starts.sort();
    localStorage.setItem(PERIOD_STORAGE_KEY, JSON.stringify(starts));
  }

  if (endDate) {
    ends[date] = endDate;
    localStorage.setItem(PERIOD_END_STORAGE_KEY, JSON.stringify(ends));
    states[date] = { lastObservation: endDate, endDate, confirmed: true };
    localStorage.setItem(BLEEDING_STATE_KEY, JSON.stringify(states));
  }

  return browserSnapshot();
}

export async function reportBleedingStatus(
  periodStart: string,
  date: string,
  stillBleeding: boolean,
  lastBleedingDate: string | null,
): Promise<DashboardSnapshot> {
  if (runningInTauri()) {
    return invoke<DashboardSnapshot>("report_bleeding_status", {
      periodStart,
      date,
      stillBleeding,
      lastBleedingDate,
    });
  }

  const today = toLocalIso(new Date());
  if (!isIsoDate(periodStart) || !isIsoDate(date) ||
      (lastBleedingDate !== null && !isIsoDate(lastBleedingDate))) {
    throw new Error("Choose valid bleeding dates.");
  }
  if (date !== today) throw new Error("Refresh the dashboard before answering the bleeding check.");

  const starts = readStoredDates();
  if (starts.at(-1) !== periodStart) {
    throw new Error("The current period changed. Refresh and try again.");
  }

  const states = readBleedingStates();
  const ends = readPeriodEnds();
  if (states[periodStart]?.confirmed === true) {
    throw new Error("This period has already been marked as finished.");
  }

  if (stillBleeding) {
    states[periodStart] = {
      lastObservation: today,
      endDate: today,
      confirmed: false,
    };
    ends[periodStart] = today;
  } else {
    if (!lastBleedingDate) {
      throw new Error("Select the last day you were bleeding.");
    }
    if (lastBleedingDate < periodStart || lastBleedingDate > date) {
      throw new Error("The last bleeding day must be between the period start and today's check.");
    }
    const previousObservation = states[periodStart]?.lastObservation;
    if (previousObservation && lastBleedingDate < previousObservation) {
      throw new Error("The last bleeding day cannot be earlier than a previously confirmed bleeding observation.");
    }
    const nextStart = readStoredDates().find((start) => start > periodStart);
    if (nextStart && lastBleedingDate >= nextStart) {
      throw new Error("The last bleeding day must be before the next recorded period start.");
    }
    states[periodStart] = {
      lastObservation: lastBleedingDate,
      endDate: lastBleedingDate,
      confirmed: true,
    };
    ends[periodStart] = lastBleedingDate;
  }

  localStorage.setItem(BLEEDING_STATE_KEY, JSON.stringify(states));
  localStorage.setItem(PERIOD_END_STORAGE_KEY, JSON.stringify(ends));
  return browserSnapshot();
}

export async function getDailyCheckin(date: string): Promise<DailyCheckinInput | null> {
  if (!isIsoDate(date)) throw new Error("Choose a valid check-in date.");
  if (runningInTauri()) {
    return invoke<DailyCheckinInput | null>("get_daily_checkin", { date });
  }
  return readStoredCheckins().find((entry) => entry.date === date) ?? null;
}

export async function saveDailyCheckin(
  input: DailyCheckinInput,
): Promise<DashboardSnapshot> {
  if (!isDailyCheckin(input)) {
    throw new Error("Check-in date, mood, energy, or symptom values are invalid.");
  }
  if (input.date > toLocalIso(new Date())) {
    throw new Error("A check-in date cannot be in the future.");
  }
  if (runningInTauri()) {
    return invoke<DashboardSnapshot>("save_daily_checkin", { input });
  }

  const checkins = readStoredCheckins().filter((entry) => entry.date !== input.date);
  checkins.push(input);
  checkins.sort((a, b) => a.date.localeCompare(b.date));
  localStorage.setItem(CHECKIN_STORAGE_KEY, JSON.stringify(checkins));
  return browserSnapshot();
}
