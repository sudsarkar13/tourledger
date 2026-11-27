// Look and feel, kept per device and never synced.
import { useSyncExternalStore } from "react";

export interface Preferences {
  theme: "system" | "light" | "dark";
  reduced: boolean;
  /** Which tours to show: a company id, "personal" or "unassigned". */
  company: string;
  /** Claim reports list every entry, not just totals by category. */
  reportDetail: boolean;
  /** Reports add how each payment was made and a note for those with no picture. */
  reportProof: boolean;
  /** Look for a newer version and download it. */
  checkUpdates: boolean;
  /** Loan instalment reminders on this device, and how they are given. */
  reminders: boolean;
  remindDays: number;
  remindHour: number;
  remindMissed: boolean;
  remindAmounts: boolean;
}

const KEY = "tourledger.preferences";
const defaults: Preferences = {
  theme: "system",
  reduced: false,
  company: "",
  reportDetail: false,
  reportProof: false,
  checkUpdates: true,
  reminders: false,
  remindDays: 3,
  remindHour: 9,
  remindMissed: true,
  remindAmounts: false,
};

function load(): Preferences {
  try {
    return { ...defaults, ...JSON.parse(localStorage.getItem(KEY) ?? "{}") };
  } catch {
    return defaults;
  }
}

let current = load();
const listeners = new Set<() => void>();

/** For code outside the page, which cannot use the hook. */
export const currentPreferences = () => current;

export function applyPreferences() {
  const root = document.documentElement;
  if (current.theme === "system") delete root.dataset.theme;
  else root.dataset.theme = current.theme;
  root.classList.toggle("reduced", current.reduced);
}

export function usePreferences() {
  const preferences = useSyncExternalStore(
    (changed) => {
      listeners.add(changed);
      return () => listeners.delete(changed);
    },
    () => current,
  );
  const update = (patch: Partial<Preferences>) => {
    current = { ...current, ...patch };
    localStorage.setItem(KEY, JSON.stringify(current));
    applyPreferences();
    listeners.forEach((l) => l());
  };
  return { preferences, update };
}
