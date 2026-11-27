// Hash routes: the back button works everywhere and nothing needs a server.
import { useSyncExternalStore } from "react";

const subscribe = (changed: () => void) => {
  window.addEventListener("hashchange", changed);
  return () => window.removeEventListener("hashchange", changed);
};

export function useRoute(): string[] {
  const hash = useSyncExternalStore(subscribe, () => window.location.hash);
  return hash.replace(/^#\/?/, "").split("/").filter(Boolean);
}

export function navigate(path: string) {
  window.location.hash = path;
}
