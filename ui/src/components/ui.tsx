// Every button, field, card, row and dialog in the app. Pages compose these and
// never style their own (docs/DESIGN.md).
import { ChevronRight, X, type LucideIcon } from "lucide-react";
import { useEffect, useId, useRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import { money } from "../format";
import { useOverlay } from "../overlays";
import { useApp } from "../state";

export function Button({
  variant = "secondary",
  icon: Icon,
  children,
  className = "",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "secondary" | "ghost" | "danger";
  icon?: LucideIcon;
}) {
  return (
    <button type="button" {...props} className={`button ${variant} ${className}`.trim()}>
      {Icon && <Icon size={18} aria-hidden />}
      {children}
    </button>
  );
}

export function IconButton({
  label,
  icon: Icon,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { label: string; icon: LucideIcon }) {
  return (
    <button type="button" {...props} className="icon-button" aria-label={label} title={label}>
      <Icon size={20} aria-hidden />
    </button>
  );
}

export function PageHeader({
  title,
  context,
  back,
  children,
}: {
  title: string;
  context?: ReactNode;
  back?: { href: string; label: string };
  children?: ReactNode;
}) {
  return (
    <header className="page-header">
      <div>
        {back && (
          <a className="back" href={back.href}>
            ‹ {back.label}
          </a>
        )}
        <h1>{title}</h1>
        {context && <p className="muted">{context}</p>}
      </div>
      {children && <div className="actions">{children}</div>}
    </header>
  );
}

/** Pages of one area, shown as links under its title. */
export function Tabs({ label, current, items }: { label: string; current: string; items: { id: string; label: string; href: string }[] }) {
  return (
    <nav className="tabs" aria-label={label}>
      {items.map((i) => (
        <a key={i.id} href={i.href} aria-current={i.id === current ? "page" : undefined}>
          {i.label}
        </a>
      ))}
    </nav>
  );
}

export function Card({ title, action, children }: { title?: string; action?: ReactNode; children: ReactNode }) {
  return (
    <section className="card">
      {(title || action) && (
        <div className="card-head">
          {title && <h2>{title}</h2>}
          {action}
        </div>
      )}
      {children}
    </section>
  );
}

export function Money({ paise, tone }: { paise: number; tone?: "good" | "bad" }) {
  return <span className={`money ${tone ?? ""}`}>{money(paise)}</span>;
}

export function Stat({
  label,
  value,
  hint,
  tone,
}: {
  label: string;
  value: ReactNode;
  hint?: ReactNode;
  tone?: "good" | "bad";
}) {
  return (
    <div className="stat">
      <span className="muted">{label}</span>
      <strong className={[tone, typeof value === "string" && value.length > 11 ? "long" : ""].filter(Boolean).join(" ")}>
        {value}
      </strong>
      {hint && <small className="muted">{hint}</small>}
    </div>
  );
}

export function List({ children }: { children: ReactNode }) {
  return <ul className="list">{children}</ul>;
}

export function ListRow({
  icon: Icon,
  title,
  detail,
  value,
  note,
  href,
  onClick,
  children,
}: {
  icon?: LucideIcon;
  title: ReactNode;
  detail?: ReactNode;
  value?: ReactNode;
  note?: ReactNode;
  href?: string;
  onClick?: () => void;
  /** Extra controls shown at the end of the row. */
  children?: ReactNode;
}) {
  const body = (
    <>
      {Icon && (
        <span className="row-icon">
          <Icon size={18} aria-hidden />
        </span>
      )}
      <span className="row-text">
        <span className="row-title">{title}</span>
        {detail && <span className="muted row-detail">{detail}</span>}
      </span>
      {(value !== undefined || note) && (
        <span className="row-value">
          <span>{value}</span>
          {note && <small className="muted">{note}</small>}
        </span>
      )}
      {href && <ChevronRight size={18} className="muted" aria-hidden />}
    </>
  );
  return (
    <li>
      {href ? (
        <a className="row" href={href}>
          {body}
        </a>
      ) : onClick ? (
        <button type="button" className="row" onClick={onClick}>
          {body}
        </button>
      ) : (
        <div className="row">{body}</div>
      )}
      {children}
    </li>
  );
}

export function Badge({ tone = "neutral", children }: { tone?: "neutral" | "good" | "warn" | "bad"; children: ReactNode }) {
  return <span className={`badge ${tone}`}>{children}</span>;
}

export function EmptyState({ icon: Icon, title, children }: { icon: LucideIcon; title: string; children?: ReactNode }) {
  return (
    <div className="empty">
      <Icon size={28} aria-hidden />
      <p>{title}</p>
      {children}
    </div>
  );
}

export function Field({
  label,
  error,
  hint,
  children,
}: {
  label: string;
  error?: string;
  hint?: ReactNode;
  children: (props: { id: string; "aria-invalid": boolean; "aria-describedby"?: string }) => ReactNode;
}) {
  const id = useId();
  const message = error ?? hint;
  return (
    <div className="field">
      <label htmlFor={id}>{label}</label>
      {children({ id, "aria-invalid": !!error, "aria-describedby": message ? `${id}-note` : undefined })}
      {message && (
        <small id={`${id}-note`} className={error ? "error" : "muted"} role={error ? "alert" : undefined}>
          {message}
        </small>
      )}
    </div>
  );
}

export function Input({
  label,
  value,
  onChange,
  error,
  hint,
  type = "text",
  inputMode,
  placeholder,
  list,
  autoFocus,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  error?: string;
  hint?: ReactNode;
  type?: "text" | "email" | "tel";
  inputMode?: "decimal" | "numeric";
  placeholder?: string;
  list?: string[];
  autoFocus?: boolean;
}) {
  const listId = useId();
  return (
    <Field label={label} error={error} hint={hint}>
      {(props) => (
        <>
          <input
            {...props}
            type={type}
            inputMode={inputMode}
            value={value}
            placeholder={placeholder}
            list={list ? listId : undefined}
            data-autofocus={autoFocus || undefined}
            autoComplete="off"
            onChange={(e) => onChange(e.target.value)}
          />
          {list && (
            <datalist id={listId}>
              {list.map((item) => (
                <option key={item} value={item} />
              ))}
            </datalist>
          )}
        </>
      )}
    </Field>
  );
}

export function TextArea({
  label,
  value,
  onChange,
  error,
  placeholder,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  error?: string;
  placeholder?: string;
}) {
  return (
    <Field label={label} error={error}>
      {(props) => (
        <textarea {...props} rows={3} value={value} placeholder={placeholder} onChange={(e) => onChange(e.target.value)} />
      )}
    </Field>
  );
}

export function Select({
  label,
  value,
  onChange,
  options,
  error,
  hint,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  options: { value: string; label: string }[];
  error?: string;
  hint?: ReactNode;
}) {
  return (
    <Field label={label} error={error} hint={hint}>
      {(props) => (
        <select {...props} value={value} onChange={(e) => onChange(e.target.value)}>
          {options.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>
      )}
    </Field>
  );
}

export function Segmented<T extends string>({
  label,
  value,
  onChange,
  options,
}: {
  label: string;
  value: T;
  onChange: (value: T) => void;
  options: { value: T; label: string }[];
}) {
  const name = useId();
  return (
    <fieldset className="segmented">
      <legend>{label}</legend>
      <div>
        {options.map((o) => (
          <label key={o.value} className={o.value === value ? "selected" : ""}>
            <input type="radio" name={name} checked={o.value === value} onChange={() => onChange(o.value)} />
            {o.label}
          </label>
        ))}
      </div>
    </fieldset>
  );
}

export function Toggle({
  label,
  hint,
  checked,
  onChange,
  disabled,
}: {
  label: string;
  hint?: ReactNode;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <label className="toggle">
      <span>
        {label}
        {hint && <small className="muted">{hint}</small>}
      </span>
      <input type="checkbox" role="switch" checked={checked} disabled={disabled} onChange={(e) => onChange(e.target.checked)} />
    </label>
  );
}

/** A group of tappable choices; several may be on. */
export function Chips({
  label,
  options,
  selected,
  onToggle,
  error,
}: {
  label: string;
  options: { value: string; label: string }[];
  selected: string[];
  onToggle: (value: string) => void;
  error?: string;
}) {
  return (
    <fieldset className="chips">
      <legend>{label}</legend>
      <div>
        {options.map((o) => (
          <button
            key={o.value}
            type="button"
            className="chip"
            aria-pressed={selected.includes(o.value)}
            onClick={() => onToggle(o.value)}
          >
            {o.label}
          </button>
        ))}
      </div>
      {error && (
        <small className="error" role="alert">
          {error}
        </small>
      )}
    </fieldset>
  );
}

/** Bottom sheet on phones, centred dialog on wide screens. */
export function Sheet({ title, onClose, children }: { title: string; onClose: () => void; children: ReactNode }) {
  const ref = useRef<HTMLDialogElement>(null);
  useOverlay(onClose);
  useEffect(() => {
    const dialog = ref.current;
    if (dialog && !dialog.open) {
      dialog.showModal();
      // React's autoFocus runs before the dialog is open, so it is done here.
      dialog.querySelector<HTMLElement>("[data-autofocus]")?.focus();
    }
  }, []);
  return (
    <dialog
      ref={ref}
      className="sheet glass"
      aria-label={title}
      onCancel={(e) => {
        // A file picker dismissed inside the form also sends "cancel", which
        // bubbles here. Only the dialog's own means close.
        if (e.target !== e.currentTarget) return;
        e.preventDefault();
        onClose();
      }}
    >
      <div className="sheet-head">
        <h2>{title}</h2>
        <IconButton label="Close" icon={X} onClick={onClose} />
      </div>
      <div className="sheet-body">{children}</div>
    </dialog>
  );
}

export function Overlays() {
  const { toasts, question } = useApp();
  return (
    <>
      {question && (
        <Sheet title={question.title} onClose={() => question.answer(false)}>
          <p>{question.body}</p>
          <div className="form-actions">
            <Button onClick={() => question.answer(false)}>Cancel</Button>
            <Button variant="danger" onClick={() => question.answer(true)}>
              {question.action}
            </Button>
          </div>
        </Sheet>
      )}
      <Toasts toasts={toasts} />
    </>
  );
}

function Toasts({ toasts }: { toasts: { id: number; text: string; tone: string }[] }) {
  const ref = useRef<HTMLDivElement>(null);
  // The top layer, so a message shows above an open sheet.
  useEffect(() => {
    const box = ref.current;
    if (!box?.showPopover) return;
    if (box.matches(":popover-open")) box.hidePopover();
    if (toasts.length) box.showPopover();
  }, [toasts]);
  return (
    <div ref={ref} popover="manual" className="toasts" aria-live="polite">
      {toasts.map((t) => (
        <div key={t.id} className={`toast ${t.tone}`}>
          {t.text}
        </div>
      ))}
    </div>
  );
}
