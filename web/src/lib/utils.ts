import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}

export function localDateIso(date = new Date()): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return year + "-" + month + "-" + day;
}

export function formatDate(value: string | null, options?: Intl.DateTimeFormatOptions): string {
  if (!value) return "Not yet available";
  const parsed = new Date(value + "T12:00:00");
  if (Number.isNaN(parsed.getTime())) return "Unknown date";

  return new Intl.DateTimeFormat(undefined, {
    day: "numeric",
    month: "short",
    ...options,
  }).format(parsed);
}
