export type Tab = "home" | "calendar" | "checkin" | "insights";

export type CyclePhaseCode =
  | "menstrual"
  | "follicular"
  | "ovulation_window"
  | "luteal"
  | "learning";

export interface PhaseProbabilities {
  menstrual: number;
  follicular: number;
  ovulationWindow: number;
  luteal: number;
}

export interface CyclePhaseSnapshot {
  code: CyclePhaseCode;
  label: string;
  cycleDay: number;
  estimatedCycleLength: number;
  completedIntervalsUsed: number;
  completedBleedingDurationsUsed: number;
  confidence: number;
  probabilities: PhaseProbabilities;
  isPersonalized: boolean;
  description: string;
}

export interface BleedingPrompt {
  periodStart: string;
  checkDate: string;
  predictedBleedingDurationDays: number;
  completedBleedingDurationsUsed: number;
}

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
  checkinDates: string[];
  phase: CyclePhaseSnapshot | null;
  bleedingPrompt: BleedingPrompt | null;
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
  checkinDates: [],
  phase: null,
  bleedingPrompt: null,
};
