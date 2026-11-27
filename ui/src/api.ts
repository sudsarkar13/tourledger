import { invoke, isTauri } from "@tauri-apps/api/core";
import type { ExportedFile } from "./types";

/** An error from the Rust side; `field` names the form field to mark. */
export class ApiError extends Error {
  field: string | null;
  constructor(message: string, field: string | null = null) {
    super(message);
    this.field = field;
  }
}

interface Failure {
  message: string;
  field?: string | null;
}

export async function api<T = unknown>(request: Record<string, unknown>): Promise<T> {
  if (isTauri()) {
    try {
      return await invoke<T>("api", { request });
    } catch (e) {
      const failure = e as Failure;
      throw new ApiError(failure.message ?? String(e), failure.field ?? null);
    }
  }
  // Plain browser: development only, through the bridge.
  let response: Response;
  try {
    response = await fetch("/api", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(request),
    });
  } catch {
    throw new ApiError("The development bridge is not running.");
  }
  const body = await response.json().catch(() => null);
  if (!response.ok) {
    const failure = (body ?? {}) as Failure;
    throw new ApiError(failure.message ?? "The development bridge is not running.", failure.field ?? null);
  }
  return body as T;
}

/** Local calendar date as YYYY-MM-DD (toISOString would give the UTC day). */
export function today(): string {
  const d = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** Returns false when the person cancelled the save dialog. */
export async function saveFile(file: ExportedFile): Promise<boolean> {
  if (isTauri()) {
    try {
      return await invoke<boolean>("save_file", {
        name: file.name,
        content: file.content,
        base64: file.base64,
      });
    } catch (e) {
      throw new ApiError((e as Failure).message ?? String(e));
    }
  }
  const data = file.base64 ? Uint8Array.from(atob(file.content), (c) => c.charCodeAt(0)) : file.content;
  const url = URL.createObjectURL(new Blob([data], { type: file.mime }));
  const link = document.createElement("a");
  link.href = url;
  link.download = file.name;
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
  return true;
}

export type LocalNetwork = "granted" | "needed" | "not_needed";

/** Added by the Android app (MainActivity.kt); absent everywhere else. */
export const android = (
  window as {
    TourLedgerAndroid?: { localNetwork(): LocalNetwork; requestLocalNetwork(): void; openAppSettings(): void };
  }
).TourLedgerAndroid;

export const isMobile = /android|iphone|ipad/i.test(navigator.userAgent);

/** Reads a QR code with the camera. Android only. */
export async function scanCode(): Promise<string | null> {
  const scanner = await import("@tauri-apps/plugin-barcode-scanner");
  let permission = await scanner.checkPermissions();
  if (permission !== "granted") permission = await scanner.requestPermissions();
  if (permission !== "granted") throw new ApiError("Allow camera access to scan the code.");
  // The camera shows behind the page, so the page must be see-through.
  document.documentElement.classList.add("scanning");
  try {
    const result = await scanner.scan({ windowed: true, formats: [scanner.Format.QRCode] });
    return result.content;
  } finally {
    document.documentElement.classList.remove("scanning");
  }
}

export async function cancelScan(): Promise<void> {
  const scanner = await import("@tauri-apps/plugin-barcode-scanner");
  await scanner.cancel();
}
