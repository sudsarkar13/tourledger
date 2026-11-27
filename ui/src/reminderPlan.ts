// What a loan reminder says and when it is shown, kept free of the page so it can
// be tested. Which reminders exist is decided in crates/core/src/reminders.rs.
import { day, money } from "./format.ts";

export interface Reminder {
  key: string;
  loan_id: string;
  name: string;
  number: number;
  due: string;
  amount: number;
  /** Money lent: the other person pays me. */
  lent: boolean;
  on: string;
  kind: "before" | "due" | "missed";
}

/** The moment a reminder is shown: its day, at the chosen hour, local time. */
export function momentOf(on: string, hour: number): Date {
  const [year, month, date] = on.split("-").map(Number);
  return new Date(year, month - 1, date, hour);
}

/** Amounts stay out unless asked for: a locked screen shows notifications. */
export function wording(r: Reminder, amounts: boolean): { title: string; body: string } {
  const what = r.lent ? "repayment to you" : "EMI";
  const sum = amounts ? `${money(r.amount)} ` : "";
  switch (r.kind) {
    case "before":
      return { title: `${r.name}: ${what} due ${day(r.due)}`, body: `${sum}${sum ? "due" : "Due"} on ${day(r.due)}.` };
    case "due":
      return { title: `${r.name}: ${what} due today`, body: `${sum}${sum ? "due" : "Due"} today.` };
    default:
      return {
        title: `${r.name}: ${what} not marked paid`,
        body: `${sum}${sum ? "was" : "Was"} due ${day(r.due)}. Mark it paid if it has gone through.`,
      };
  }
}

/** A number for the system to know a reminder by, the same every time. */
export function numberOf(key: string): number {
  let hash = 0;
  for (let i = 0; i < key.length; i++) hash = (Math.imul(hash, 31) + key.charCodeAt(i)) | 0;
  return (hash >>> 1) || 1;
}

/** Keys already delivered, or handed to the system for a given moment. */
export type Told = Record<string, number>;

const isoDay = (d: Date) =>
  `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;

/** Those whose moment has passed, that still say something true, and were not shown. */
export function dueNow(list: Reminder[], hour: number, now: Date, told: Told): Reminder[] {
  const today = isoDay(now);
  return list.filter((r) => {
    if (momentOf(r.on, hour) > now) return false;
    if (told[r.key] !== undefined && told[r.key] <= now.getTime()) return false;
    // "Due 10 Dec" is stale on the day itself, and "due today" the day after.
    if (r.kind === "before") return today < r.due;
    if (r.kind === "due") return today === r.due;
    return true;
  });
}

/** Those still to come. Android holds only so many, so the nearest are kept. */
export const toSchedule = (list: Reminder[], hour: number, now: Date) =>
  list.filter((r) => momentOf(r.on, hour) > now).slice(0, 50);

/** Forgets what is long past, so the record does not grow for ever. */
export function pruned(told: Told, now: Date): Told {
  const old = now.getTime() - 90 * 24 * 60 * 60 * 1000;
  return Object.fromEntries(Object.entries(told).filter(([, at]) => at >= old));
}

/** How long past its time a reminder may still be on its way: they are not exact. */
const LATE = 60 * 60 * 1000;

export interface Plan {
  /** Handed to the system and now to be dropped. */
  cancel: number[];
  /** To show at once. */
  show: Reminder[];
  /** To hand to the system for later. */
  schedule: Reminder[];
  /** What is certainly past; add each one shown or scheduled as that succeeds. */
  told: Told;
}

/**
 * Brings what the system holds into line with the ledger. `pending` are the
 * numbers it was given and has not delivered; a computer holds none.
 */
export function plan(list: Reminder[], hour: number, now: Date, told: Told, pending: number[]): Plan {
  const held = new Set(pending);
  // A time still to come is only an intention: it is set again below, or dropped.
  const past: Told = Object.fromEntries(Object.entries(told).filter(([, at]) => at <= now.getTime()));
  const onTheWay = new Set<number>();
  for (const r of list) {
    const number = numberOf(r.key);
    if (momentOf(r.on, hour) > now || !held.has(number)) continue;
    const at = past[r.key];
    if (at !== undefined && now.getTime() - at < LATE) onTheWay.add(number);
    // Still waiting long after its time: the system dropped it.
    else delete past[r.key];
  }
  return {
    cancel: pending.filter((number) => !onTheWay.has(number)),
    show: dueNow(list, hour, now, past).filter((r) => !onTheWay.has(numberOf(r.key))),
    schedule: toSchedule(list, hour, now),
    told: past,
  };
}
