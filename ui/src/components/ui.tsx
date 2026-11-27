// Every button, field, card, row and dialog in the app. Pages compose these and
// never style their own (docs/DESIGN.md).
import { Check, ChevronDown, ChevronRight, X, type LucideIcon } from "lucide-react";
import {
  useEffect,
  useId,
  useRef,
  useState,
  type ButtonHTMLAttributes,
  type CSSProperties,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
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

export interface Option {
  value: string;
  label: string;
  icon?: LucideIcon;
}

/** Every drop-down: a button that opens a glass menu of choices. */
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
  options: Option[];
  error?: string;
  hint?: ReactNode;
}) {
  // Where the button was when the menu opened; null while closed.
  const [anchor, setAnchor] = useState<DOMRect | null>(null);
  const button = useRef<HTMLButtonElement>(null);
  const chosen = options.find((o) => o.value === value);
  const open = () => setAnchor(button.current!.getBoundingClientRect());
  const close = () => setAnchor(null);
  // Focus goes back once the menu is gone: until then the page behind it is inert.
  // Without scrolling, or closing by scrolling away would jump straight back.
  const wasOpen = useRef(false);
  useEffect(() => {
    if (anchor) wasOpen.current = true;
    else if (wasOpen.current) button.current?.focus({ preventScroll: true });
  }, [anchor]);
  return (
    <>
      <Field label={label} error={error} hint={hint}>
        {(props) => (
          <button
            {...props}
            ref={button}
            type="button"
            className="select-button"
            aria-haspopup="listbox"
            aria-expanded={anchor !== null}
            aria-label={`${label}: ${chosen?.label ?? "nothing chosen"}`}
            onClick={open}
            onKeyDown={(e) => {
              if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
              e.preventDefault();
              open();
            }}
          >
            {chosen?.icon && <chosen.icon size={18} aria-hidden />}
            <span>{chosen?.label ?? "Choose"}</span>
            <ChevronDown className="select-chevron" size={18} aria-hidden />
          </button>
        )}
      </Field>
      {anchor &&
        // Outside the surrounding form, so nothing in here can submit it.
        createPortal(
          <Menu
            title={label}
            anchor={anchor}
            options={options}
            value={value}
            onClose={close}
            onPick={(picked) => {
              close();
              if (picked !== value) onChange(picked);
            }}
          />,
          document.body,
        )}
    </>
  );
}

/** Beside its button on a wide screen, above it when there is no room below. */
function placement(button: DOMRect): CSSProperties {
  const width = Math.min(Math.max(button.width, 240), window.innerWidth - 16);
  const left = Math.min(Math.max(8, button.left), window.innerWidth - width - 8);
  const below = window.innerHeight - button.bottom - 14;
  const above = button.top - 14;
  const down = below >= 280 || below >= above;
  const maxHeight = Math.min(360, down ? below : above);
  return down
    ? { left, width, maxHeight, top: button.bottom + 6 }
    : { left, width, maxHeight, bottom: window.innerHeight - button.top + 6 };
}

/** A list of choices: floating beside its button, or rising from the bottom on phones. */
function Menu({
  title,
  anchor,
  options,
  value,
  onPick,
  onClose,
}: {
  title: string;
  anchor: DOMRect;
  options: Option[];
  value: string;
  onPick: (value: string) => void;
  onClose: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const typed = useRef({ text: "", at: 0 });
  const [floating] = useState(() => window.matchMedia("(min-width: 640px)").matches);
  useOverlay(onClose);
  const items = () => [...(list.current?.querySelectorAll<HTMLButtonElement>("[role=option]") ?? [])];
  const move = (to: number) => {
    const item = items()[to];
    item?.focus();
    item?.scrollIntoView({ block: "nearest" });
  };

  useEffect(() => {
    if (!dialog.current!.open) dialog.current!.showModal();
    move(Math.max(0, options.findIndex((o) => o.value === value)));
    // Placed for this width: turning a phone or resizing the window closes it.
    // Height alone is left, as a keyboard sliding away changes it.
    const width = window.innerWidth;
    const resized = () => window.innerWidth !== width && onClose();
    // A floating menu is placed against its button: close it if that scrolls off.
    const away = (e: Event) => {
      if (floating && !dialog.current?.contains(e.target as Node)) onClose();
    };
    window.addEventListener("resize", resized);
    window.addEventListener("scroll", away, true);
    return () => {
      window.removeEventListener("resize", resized);
      window.removeEventListener("scroll", away, true);
    };
  }, []);

  const keys = (e: KeyboardEvent) => {
    const at = items().indexOf(document.activeElement as HTMLButtonElement);
    const last = options.length - 1;
    let to: number | null = null;
    if (e.key === "ArrowDown") to = Math.min(at + 1, last);
    else if (e.key === "ArrowUp") to = Math.max(at - 1, 0);
    else if (e.key === "Home") to = 0;
    else if (e.key === "End") to = last;
    else if (e.key === "Tab") {
      e.preventDefault();
      onClose();
    } else if (e.key.length === 1 && e.key !== " " && !e.ctrlKey && !e.metaKey && !e.altKey) {
      // Typing the first letters jumps to a choice, as in a system list.
      const now = Date.now();
      const text = (now - typed.current.at < 700 ? typed.current.text : "") + e.key.toLowerCase();
      typed.current = { text, at: now };
      const found = options.findIndex((o) => o.label.toLowerCase().startsWith(text));
      if (found >= 0) to = found;
    }
    if (to === null) return;
    e.preventDefault();
    move(to);
  };

  return (
    <dialog
      ref={dialog}
      className="menu"
      aria-label={title}
      style={floating ? placement(anchor) : undefined}
      onCancel={(e) => {
        e.preventDefault();
        e.stopPropagation();
        onClose();
      }}
      // The backdrop counts as the dialog itself; the choices fill it.
      onClick={(e) => e.target === e.currentTarget && onClose()}
      onKeyDown={keys}
    >
      <div className="menu-head">
        <h2>{title}</h2>
        <IconButton label="Close" icon={X} onClick={onClose} />
      </div>
      <div className="menu-list" ref={list} role="listbox" aria-label={title}>
        {options.map((o) => (
          <button
            key={o.value}
            type="button"
            role="option"
            className="menu-option"
            aria-selected={o.value === value}
            tabIndex={-1}
            onClick={() => onPick(o.value)}
          >
            {o.icon && (
              <i aria-hidden>
                <o.icon size={17} />
              </i>
            )}
            <span>{o.label}</span>
            {o.value === value && <Check size={18} aria-hidden />}
          </button>
        ))}
      </div>
    </dialog>
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
