// Updating the app. The work is done natively (src-tauri/src/updates.rs on
// computers, Updater.kt on Android); this asks for it and follows along.
import { invoke, isTauri } from "@tauri-apps/api/core";
import { useSyncExternalStore } from "react";
import { currentPreferences } from "./preferences";
import { busy, downloadsByItself, type UpdateStatus } from "./updateSteps";

interface Source {
  status: () => Promise<UpdateStatus>;
  check: () => Promise<unknown>;
  download: () => Promise<unknown>;
  install: () => Promise<unknown>;
}

/** Added by the Android app; absent everywhere else. */
interface AndroidUpdates {
  status: () => string;
  check: () => void;
  download: () => void;
  install: () => void;
}

function source(): Source | null {
  const native = (window as { TourLedgerUpdates?: AndroidUpdates }).TourLedgerUpdates;
  if (native) {
    return {
      status: async () => JSON.parse(native.status()) as UpdateStatus,
      check: async () => native.check(),
      download: async () => native.download(),
      install: async () => native.install(),
    };
  }
  if (!isTauri()) return null;
  return {
    status: () => invoke<UpdateStatus>("update_status"),
    check: () => invoke("update_check"),
    download: () => invoke("update_download"),
    install: () => invoke("update_install"),
  };
}

let current: UpdateStatus | null = null;
const listeners = new Set<() => void>();
let following: Promise<void> | null = null;

async function read(from: Source): Promise<UpdateStatus> {
  current = await from.status();
  listeners.forEach((l) => l());
  return current;
}

/** Reads the status until nothing is going on. */
function follow(from: Source): Promise<void> {
  following ??= (async () => {
    try {
      while (busy((await read(from)).phase)) await new Promise((r) => setTimeout(r, 500));
    } finally {
      following = null;
    }
  })();
  return following;
}

/** Starts one step and follows it. A failure shows in the status. */
async function run(step: "check" | "download" | "install") {
  const from = source();
  if (!from) return;
  const started = from[step]();
  // Install on a computer only returns if it failed: follow without waiting.
  await new Promise((r) => setTimeout(r, 100));
  await Promise.all([follow(from), step === "install" ? undefined : started]);
  await read(from);
}

export async function checkForUpdate() {
  await run("check");
  if (current && downloadsByItself(current, currentPreferences().checkUpdates)) await run("download");
}

export const downloadUpdate = () => run("download");
export const installUpdate = () => run("install");

const DAY = 24 * 60 * 60 * 1000;

/** Looks once shortly after opening, then daily while the app stays open. */
export function startUpdates() {
  const from = source();
  if (!from) return;
  void read(from);
  let last = 0;
  const due = () => {
    if (!currentPreferences().checkUpdates || Date.now() - last < DAY) return;
    last = Date.now();
    // A background check that cannot reach GitHub says nothing.
    void checkForUpdate().catch(() => {});
  };
  setTimeout(due, 5000);
  setInterval(due, 60 * 60 * 1000);
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState !== "visible") return;
    // Android reports an install's outcome while the app was in the background.
    void read(from);
    due();
  });
}

/** Null until the app has answered, and where there is no updating. */
export function useUpdate(): UpdateStatus | null {
  const status = useSyncExternalStore(
    (changed) => {
      listeners.add(changed);
      return () => listeners.delete(changed);
    },
    () => current,
  );
  return status?.supported ? status : null;
}
