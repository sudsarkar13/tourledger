// Look and feel, kept per device and never synced.
import { useSyncExternalStore } from "react";

export interface Preferences {
  theme: "system" | "light" | "dark";
  reduced: boolean;
  /** Which tours to show: a company id, "personal" or "unassigned". */
  company: string;
  /** Claim reports list every entry, not just totals by category. */
  reportDetail: boolean;
}

const KEY = "tourledger.preferences";
const defaults: Preferences = { theme: "system", reduced: false, company: "", reportDetail: false };

function load(): Preferences {
  try {
    return { ...defaults, ...JSON.parse(localStorage.getItem(KEY) ?? "{}") };
  } catch {
    return defaults;
  }
}

let current = load();
const listeners = new Set<() => void>();

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
