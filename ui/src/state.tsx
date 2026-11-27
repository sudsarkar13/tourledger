import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { api, ApiError, today } from "./api";
import { dismissSplash } from "./splash";
import type { Kind, Snapshot, SyncStatus } from "./types";

interface Toast {
  id: number;
  text: string;
  tone: "ok" | "error";
}

interface Question {
  title: string;
  body: string;
  action: string;
  answer: (yes: boolean) => void;
}

interface App {
  data: Snapshot;
  sync: SyncStatus | null;
  setSync: (s: SyncStatus) => void;
  reload: () => Promise<void>;
  save: (kind: Kind, body: object, id?: string) => Promise<string>;
  /** Asks first, then deletes. Resolves true when it was removed. */
  remove: (kind: Kind, id: string, title: string, body: string) => Promise<boolean>;
  notify: (text: string, tone?: Toast["tone"]) => void;
  fail: (error: unknown) => void;
  confirm: (title: string, body: string, action: string) => Promise<boolean>;
  toasts: Toast[];
  question: Question | null;
}

const Context = createContext<App | null>(null);

export function useApp(): App {
  const value = useContext(Context);
  if (!value) throw new Error("useApp outside AppProvider");
  return value;
}

const POLL_MS = 3000;

export function AppProvider({ children }: { children: ReactNode }) {
  const [data, setData] = useState<Snapshot | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [sync, setSync] = useState<SyncStatus | null>(null);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [question, setQuestion] = useState<Question | null>(null);
  const seen = useRef({ revision: -1, day: "" });
  const nextToast = useRef(0);

  const reload = useCallback(async () => {
    const day = today();
    const snapshot = await api<Snapshot>({ op: "snapshot", today: day });
    seen.current = { revision: snapshot.revision, day };
    setData(snapshot);
    setLoadError(null);
  }, []);

  const notify = useCallback((text: string, tone: Toast["tone"] = "ok") => {
    const id = ++nextToast.current;
    setToasts((list) => [...list.slice(-2), { id, text, tone }]);
    // Long messages stay long enough to be read.
    const stay = tone === "error" ? 6000 : Math.max(2600, text.length * 70);
    setTimeout(() => setToasts((list) => list.filter((t) => t.id !== id)), stay);
  }, []);

  const fail = useCallback(
    (error: unknown) => notify(error instanceof Error ? error.message : String(error), "error"),
    [notify],
  );

  // Changes arrive from linked devices, and the date changes at midnight.
  useEffect(() => {
    let stopped = false;
    const poll = async () => {
      // Idle while hidden, but always do the first load.
      if (document.hidden && seen.current.revision >= 0) return;
      try {
        const status = await api<SyncStatus>({ op: "sync.status" });
        if (stopped) return;
        setSync(status);
        if (status.revision !== seen.current.revision || today() !== seen.current.day) await reload();
      } catch (error) {
        if (!stopped && seen.current.revision < 0) {
          setLoadError(error instanceof ApiError ? error.message : String(error));
        }
      }
    };
    // Coming back to the app, or onto a network, syncs at once.
    const wake = () => {
      if (!document.hidden) api({ op: "sync.wake" }).then(poll, () => {});
    };
    void poll();
    const timer = setInterval(poll, POLL_MS);
    document.addEventListener("visibilitychange", poll);
    document.addEventListener("visibilitychange", wake);
    window.addEventListener("online", wake);
    return () => {
      stopped = true;
      clearInterval(timer);
      document.removeEventListener("visibilitychange", poll);
      document.removeEventListener("visibilitychange", wake);
      window.removeEventListener("online", wake);
    };
  }, [reload]);

  // Said wherever the person is: the link ended from the other side.
  const droppedAt = sync?.unlinked_by?.at_ms;
  const droppedBy = sync?.unlinked_by?.name;
  useEffect(() => {
    if (droppedAt) notify(`${droppedBy} unlinked this device`);
  }, [droppedAt, droppedBy, notify]);

  const ready = data !== null || loadError !== null;
  useEffect(() => {
    if (ready) dismissSplash();
  }, [ready]);

  const confirm = useCallback(
    (title: string, body: string, action: string) =>
      new Promise<boolean>((resolve) => {
        setQuestion({
          title,
          body,
          action,
          answer: (yes) => {
            setQuestion(null);
            resolve(yes);
          },
        });
      }),
    [],
  );

  const save = useCallback(
    async (kind: Kind, body: object, id?: string) => {
      const result = await api<{ id: string }>({ op: "save", kind, id, body });
      await reload();
      return result.id;
    },
    [reload],
  );

  const remove = useCallback(
    async (kind: Kind, id: string, title: string, body: string) => {
      if (!(await confirm(title, body, "Delete"))) return false;
      try {
        await api({ op: "delete", kind, id });
        await reload();
        notify("Deleted");
        return true;
      } catch (error) {
        fail(error);
        return false;
      }
    },
    [confirm, reload, notify, fail],
  );

  if (!data) {
    // Until something is known, the launch screen covers the window.
    return loadError ? (
      <div className="boot" role="alert">
        <div>
          <h1>TourLedger could not open</h1>
          <p>{loadError}</p>
        </div>
      </div>
    ) : null;
  }

  return (
    <Context.Provider
      value={{ data, sync, setSync, reload, save, remove, notify, fail, confirm, toasts, question }}
    >
      {children}
    </Context.Provider>
  );
}
