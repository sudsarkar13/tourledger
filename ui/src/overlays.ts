// Open sheets, newest last, so Android's back button closes the top one.
import { isTauri } from "@tauri-apps/api/core";
import { exit, onBackButtonPress } from "@tauri-apps/api/app";
import { useEffect, useRef } from "react";
import { isMobile } from "./api";

const stack: (() => void)[] = [];

export function useOverlay(close: () => void) {
  const latest = useRef(close);
  latest.current = close;
  useEffect(() => {
    const entry = () => latest.current();
    stack.push(entry);
    return () => {
      stack.splice(stack.indexOf(entry), 1);
    };
  }, []);
}

export function listenForBack() {
  if (!isTauri() || !isMobile) return;
  void onBackButtonPress(({ canGoBack }) => {
    const top = stack.at(-1);
    if (top) top();
    else if (canGoBack) window.history.back();
    else void exit(0);
  });
}
