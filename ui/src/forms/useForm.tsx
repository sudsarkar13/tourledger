import { useEffect, useRef, useState, type FormEvent, type ReactNode } from "react";
import { ApiError } from "../api";
import { Button } from "../components/ui";
import { parseMoney } from "../format";

export type Errors = Record<string, string | undefined>;

export function useForm<T extends object>(initial: T) {
  const [values, setValues] = useState(initial);
  const [errors, setErrors] = useState<Errors>({});
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);

  const set = <K extends keyof T>(key: K, value: T[K]) => {
    setValues((v) => ({ ...v, [key]: value }));
    setErrors((e) => ({ ...e, [key as string]: undefined }));
  };

  /** Runs the save; an error from Rust is shown on its field. */
  const submit = (action: () => Promise<void>) => async (event: FormEvent) => {
    event.preventDefault();
    if (busy) return;
    setBusy(true);
    setMessage("");
    setErrors({});
    try {
      await action();
    } catch (error) {
      if (error instanceof ApiError && error.field) setErrors({ [error.field]: error.message });
      else setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  };

  return { values, setValues, set, errors, message, busy, submit };
}

/** Typed rupees to paise; empty means zero. Throws on anything else. */
export function amount(field: string, text: string): number {
  if (!text.trim()) return 0;
  const value = parseMoney(text);
  if (value === null) throw new ApiError("Enter an amount like 1250 or 1250.50.", field);
  return value;
}

export function Form({
  onSubmit,
  busy,
  message,
  submitLabel,
  onDelete,
  children,
}: {
  onSubmit: (event: FormEvent) => void;
  busy: boolean;
  message: string;
  submitLabel: string;
  onDelete?: () => void;
  children: ReactNode;
}) {
  const ref = useRef<HTMLFormElement>(null);
  const wasBusy = useRef(busy);
  // After a refused save, bring the first marked field into view.
  useEffect(() => {
    if (wasBusy.current && !busy) {
      ref.current?.querySelector<HTMLElement>('[aria-invalid="true"]')?.focus();
    }
    wasBusy.current = busy;
  }, [busy]);

  return (
    <form ref={ref} onSubmit={onSubmit} noValidate>
      {children}
      {message && (
        <p className="error" role="alert">
          {message}
        </p>
      )}
      <div className="form-actions">
        {onDelete && (
          <Button variant="ghost" onClick={onDelete} className="delete">
            Delete
          </Button>
        )}
        <button type="submit" className="button primary" disabled={busy}>
          {submitLabel}
        </button>
      </div>
    </form>
  );
}
