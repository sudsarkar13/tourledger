// What to do about an update, kept free of the page so it can be tested.

export type UpdatePhase =
  | "idle"
  | "checking"
  | "up_to_date"
  | "available"
  | "downloading"
  | "ready"
  | "needs_permission"
  | "installing"
  | "failed";

export interface UpdateStatus {
  supported: boolean;
  current: string;
  phase: UpdatePhase;
  version: string | null;
  notes: string | null;
  received: number;
  total: number | null;
  error: string | null;
  checked_ms: number | null;
  /** A connection that costs by use: nothing downloads by itself. */
  metered: boolean;
}

/** Work is going on, so the status is worth reading again shortly. */
export const busy = (phase: UpdatePhase) => phase === "checking" || phase === "downloading" || phase === "installing";

/** Whether a found update should start downloading without being asked. */
export const downloadsByItself = (status: UpdateStatus, automatic: boolean) =>
  automatic && status.phase === "available" && !status.metered;

/** Whole percent downloaded, or null when the size is not known. */
export const percent = (status: UpdateStatus) =>
  status.total ? Math.min(100, Math.floor((status.received / status.total) * 100)) : null;

/** One line saying where things stand. */
export function describe(status: UpdateStatus): string {
  const version = status.version ? `Version ${status.version}` : "A new version";
  switch (status.phase) {
    case "checking":
      return "Looking for a newer version…";
    case "up_to_date":
      return "This is the newest version.";
    case "available":
      return status.metered ? `${version} is out. This connection is metered, so it waits for you.` : `${version} is out.`;
    case "downloading": {
      const done = percent(status);
      return done === null ? `Downloading ${version.toLowerCase()}…` : `Downloading ${version.toLowerCase()}: ${done}%`;
    }
    case "ready":
      return status.error
        ? `${version} could not be installed: ${status.error.replace(/\.?\s*$/, ".")} You can also get it from the releases page on GitHub.`
        : `${version} is ready to install. Your entries stay as they are.`;
    case "needs_permission":
      return "Allow TourLedger to install apps in the screen Android opened, then press Install.";
    case "installing":
      return "Installing…";
    case "failed":
      return status.error ?? "The update did not work.";
    default:
      return "Not checked yet.";
  }
}
