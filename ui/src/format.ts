const rupees = new Intl.NumberFormat("en-IN", {
  style: "currency",
  currency: "INR",
  minimumFractionDigits: 0,
  maximumFractionDigits: 2,
});
const compact = new Intl.NumberFormat("en-IN", {
  style: "currency",
  currency: "INR",
  notation: "compact",
  maximumFractionDigits: 1,
});

export const money = (paise: number) =>
  // Whole rupees show no decimals; anything else shows two.
  paise % 100 === 0
    ? rupees.format(paise / 100)
    : rupees.format(paise / 100).replace(/(\.\d)$/, "$10");

export const shortMoney = (paise: number) => compact.format(paise / 100);

/** Typed rupees to paise, without floating point. Null when not a number. */
export function parseMoney(text: string): number | null {
  const clean = text
    .trim()
    .replace(/^(rs\.?|inr)/i, "")
    .replace(/(\/-|rs|rupees)$/i, "")
    .replace(/[,\s₹]/g, "");
  const match = /^(\d{1,12})(?:\.(\d{0,2}))?$/.exec(clean);
  if (!match) return null;
  return Number(match[1]) * 100 + Number((match[2] ?? "").padEnd(2, "0"));
}

export const moneyInput = (paise: number) =>
  paise ? (paise % 100 === 0 ? String(paise / 100) : (paise / 100).toFixed(2)) : "";

/** A decimal like "10.5" as an integer scaled by 10^places. */
export function parseDecimal(text: string, places: number): number | null {
  const match = new RegExp(`^(\\d{1,9})(?:\\.(\\d{0,${places}}))?$`).exec(text.trim());
  if (!match) return null;
  return Number(match[1]) * 10 ** places + Number((match[2] ?? "").padEnd(places, "0"));
}

export const decimalInput = (value: number, places: number) =>
  value ? String(value / 10 ** places) : "";

// Noon avoids a date moving across midnight in any time zone.
const asDate = (iso: string) => new Date(`${iso}T12:00:00`);

export const day = (iso: string) =>
  iso ? asDate(iso).toLocaleDateString("en-IN", { day: "numeric", month: "short" }) : "";

export const fullDay = (iso: string) =>
  iso
    ? asDate(iso).toLocaleDateString("en-IN", { day: "numeric", month: "short", year: "numeric" })
    : "";

export function dateRange(start: string, end: string): string {
  if (!start) return "";
  return start === end ? fullDay(start) : `${day(start)} – ${fullDay(end)}`;
}

export const monthName = (key: string, long = false) =>
  asDate(`${key}-01`).toLocaleDateString("en-IN", {
    month: long ? "long" : "short",
    ...(long ? { year: "numeric" } : {}),
  });

export function ago(ms: number, now = Date.now()): string {
  if (!ms) return "never";
  const seconds = Math.max(0, Math.round((now - ms) / 1000));
  if (seconds < 60) return "just now";
  if (seconds < 3600) return `${Math.floor(seconds / 60)} min ago`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)} h ago`;
  return new Date(ms).toLocaleDateString("en-IN", { day: "numeric", month: "short" });
}

export const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;
