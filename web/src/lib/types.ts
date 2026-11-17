export type Tab = "home" | "calendar" | "checkin" | "insights";

export interface DashboardSnapshot {
  lastPeriodStart: string | null;
  cycleDay: number | null;
  expectedNextPeriodStart: string | null;
  earliestNextPeriodStart: string | null;
  latestNextPeriodStart: string | null;
  cyclesTracked: number;
  averageCycleLength: number | null;
  checkinsTracked: number;
  periodStarts: string[];
}

export interface SymptomInput {
  code: string;
  severity: number;
}

export interface DailyCheckinInput {
  date: string;
  moodScore: number;
  energyScore: number;
  symptoms: SymptomInput[];
}

export const EMPTY_SNAPSHOT: DashboardSnapshot = {
  lastPeriodStart: null,
  cycleDay: null,
  expectedNextPeriodStart: null,
  earliestNextPeriodStart: null,
  latestNextPeriodStart: null,
  cyclesTracked: 0,
  averageCycleLength: null,
  checkinsTracked: 0,
  periodStarts: [],
};
