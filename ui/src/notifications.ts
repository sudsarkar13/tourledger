// Loan and EMI reminders on this device. Android is handed the coming ones, so
// they arrive with the app closed; on a computer they are shown while it is open.
import { invoke, isTauri } from "@tauri-apps/api/core";
import { isPermissionGranted, requestPermission, Schedule } from "@tauri-apps/plugin-notification";
import { api, isMobile, today } from "./api";
import type { Preferences } from "./preferences";
import { momentOf, numberOf, plan, pruned, wording, type Reminder, type Told } from "./reminderPlan";

export interface Message {
  id: number;
  title: string;
  body: string;
}

export interface Notifier {
  /** The system can hold a reminder for later. */
  schedules: boolean;
  granted: () => Promise<boolean>;
  ask: () => Promise<boolean>;
  send: (message: Message) => Promise<void>;
  schedule: (messages: (Message & { at: Date })[]) => Promise<void>;
  /** Numbers of those scheduled and not yet delivered. */
  pending: () => Promise<number[]>;
  cancel: (numbers: number[]) => Promise<void>;
}

const native: Notifier = {
  schedules: isMobile,
  granted: () => isPermissionGranted(),
  ask: async () => (await isPermissionGranted()) || (await requestPermission()) === "granted",
  send: (message) => invoke("plugin:notification|notify", { options: message }),
  // Only `batch` records what it schedules. Without that record the plugin can
  // neither cancel a reminder nor set it again after the phone restarts, and its
  // `notify` and `cancelAll` leave it empty.
  schedule: async (messages) => {
    const notifications = messages.map(({ at, ...message }) => {
      // May wake an idle phone, without asking for exact alarms.
      const full = { ...message, schedule: Schedule.at(at, false, true) };
      // Kept as written and read back by the plugin after a restart.
      return { ...full, sourceJson: JSON.stringify(full) };
    });
    if (notifications.length) await invoke("plugin:notification|batch", { notifications });
  },
  pending: async () =>
    isMobile ? (await invoke<{ id: number }[]>("plugin:notification|get_pending")).map((n) => n.id) : [],
  cancel: async (numbers) => {
    if (numbers.length) await invoke("plugin:notification|cancel", { notifications: numbers });
  },
};

let notifier: Notifier | null = isTauri() ? native : null;

/** For checking the page in a plain browser, which has no notifications here. */
export const setNotifier = (next: Notifier | null) => {
  notifier = next;
};
export const getNotifier = () => notifier;

const KEY = "tourledger.told";

function told(): Told {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "{}") as Told;
  } catch {
    return {};
  }
}

export const reminderList = (preferences: Preferences) =>
  api<Reminder[]>({
    op: "reminders",
    today: today(),
    lead_days: preferences.remindDays,
    missed: preferences.remindMissed,
  });

async function apply(preferences: Preferences) {
  const to = notifier;
  if (!to) return;
  const on = preferences.reminders && (await to.granted());
  // Switched off, nothing is listed, so everything waiting is dropped.
  const list = on ? await reminderList(preferences) : [];
  const now = new Date();
  const hour = preferences.remindHour;
  const pending = to.schedules ? await to.pending() : [];
  const next = plan(list, hour, now, pruned(told(), now), pending);
  const record = next.told;
  const keep = () => localStorage.setItem(KEY, JSON.stringify(record));
  await to.cancel(next.cancel);
  keep();
  const message = (r: Reminder) => ({ id: numberOf(r.key), ...wording(r, preferences.remindAmounts) });
  for (const r of next.show) {
    await to.send(message(r));
    record[r.key] = now.getTime();
    // Saved one by one, so a failure further on does not repeat this one.
    keep();
  }
  if (to.schedules && next.schedule.length) {
    await to.schedule(next.schedule.map((r) => ({ ...message(r), at: momentOf(r.on, hour) })));
    for (const r of next.schedule) record[r.key] = momentOf(r.on, hour).getTime();
    keep();
  }
}

let running: Promise<void> | null = null;
let again: Preferences | null = null;

/** One run at a time; a change arriving meanwhile gets a run of its own after. */
export function refreshReminders(preferences: Preferences): Promise<void> {
  if (running) {
    again = preferences;
    return running;
  }
  running = apply(preferences)
    // A reminder that cannot be set must not disturb the app.
    .catch((error) => console.error("reminders", error))
    .finally(() => {
      running = null;
      if (again) {
        const next = again;
        again = null;
        void refreshReminders(next);
      }
    });
  return running;
}
