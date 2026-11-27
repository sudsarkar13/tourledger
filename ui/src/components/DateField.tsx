// Every date in the app: a large calendar with month and year choice.
import { CalendarDays, ChevronLeft, ChevronRight } from "lucide-react";
import { useState } from "react";
import { createPortal } from "react-dom";
import { DayPicker } from "react-day-picker";
import "react-day-picker/style.css";
import { today } from "../api";
import { dateRange } from "../format";
import { Button, Field, Select, Sheet } from "./ui";

// Noon keeps a date on the same day in every time zone.
const toDate = (iso: string) => new Date(`${iso}T12:00:00`);
const toIso = (d: Date) =>
  `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
const long = (iso: string) =>
  toDate(iso).toLocaleDateString("en-IN", { weekday: "short", day: "numeric", month: "short", year: "numeric" });

const MONTHS = Array.from({ length: 12 }, (_, i) => new Date(2000, i, 1).toLocaleDateString("en-IN", { month: "long" }));
const FIRST_YEAR = 1990;

function Calendar({
  value,
  event,
  optional,
  onPick,
}: {
  value: string;
  event?: { from: string; to: string };
  optional?: boolean;
  onPick: (iso: string) => void;
}) {
  // Opens on the chosen date, else on the event, else on this month.
  const [month, setMonth] = useState(() => toDate(value || event?.from || today()));
  const lastYear = Math.max(new Date().getFullYear() + 15, month.getFullYear());
  const move = (by: number) => setMonth(new Date(month.getFullYear(), month.getMonth() + by, 1, 12));
  const jump = (iso: string) => onPick(iso);
  const yesterday = () => {
    const d = toDate(today());
    d.setDate(d.getDate() - 1);
    return toIso(d);
  };

  return (
    <div className="calendar">
      <div className="calendar-head">
        <Select
          label="Month"
          value={String(month.getMonth())}
          onChange={(m) => setMonth(new Date(month.getFullYear(), Number(m), 1, 12))}
          options={MONTHS.map((label, i) => ({ value: String(i), label }))}
        />
        <Select
          label="Year"
          value={String(month.getFullYear())}
          onChange={(y) => setMonth(new Date(Number(y), month.getMonth(), 1, 12))}
          options={Array.from({ length: lastYear - FIRST_YEAR + 1 }, (_, i) => {
            const year = String(FIRST_YEAR + i);
            return { value: year, label: year };
          })}
        />
      </div>
      <div className="calendar-step">
        <Button icon={ChevronLeft} aria-label="Previous month" onClick={() => move(-1)}>
          Previous
        </Button>
        <Button icon={ChevronRight} aria-label="Next month" onClick={() => move(1)}>
          Next
        </Button>
      </div>
      <DayPicker
        mode="single"
        required
        selected={value ? toDate(value) : undefined}
        onSelect={(d: Date) => onPick(toIso(d))}
        month={month}
        onMonthChange={setMonth}
        hideNavigation
        showOutsideDays
        fixedWeeks
        weekStartsOn={1}
        modifiers={event ? { event: { from: toDate(event.from), to: toDate(event.to) } } : undefined}
        modifiersClassNames={{ event: "event-day" }}
      />
      {event && (
        <p className="muted calendar-key">
          <span className="event-swatch" aria-hidden /> Event: {dateRange(event.from, event.to)}. You can pick days before or
          after it for the journey.
        </p>
      )}
      <div className="actions">
        <Button onClick={() => jump(today())}>Today</Button>
        <Button onClick={() => jump(yesterday())}>Yesterday</Button>
        {event && <Button onClick={() => jump(event.from)}>Event start</Button>}
        {optional && value && (
          <Button variant="ghost" onClick={() => jump("")}>
            Clear date
          </Button>
        )}
      </div>
    </div>
  );
}

export function DateField({
  label,
  value,
  onChange,
  error,
  hint,
  event,
  optional,
}: {
  label: string;
  value: string;
  onChange: (iso: string) => void;
  error?: string;
  hint?: string;
  /** The tour's dates, marked on the calendar. */
  event?: { from: string; to: string };
  optional?: boolean;
}) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Field label={label} error={error} hint={hint}>
        {(props) => (
          <button {...props} type="button" className="date-button" aria-haspopup="dialog" onClick={() => setOpen(true)}>
            <span>{value ? long(value) : "Choose a date"}</span>
            <CalendarDays size={20} aria-hidden />
          </button>
        )}
      </Field>
      {open &&
        // Outside the surrounding form, so nothing in here can submit it.
        createPortal(
          <Sheet title={label} onClose={() => setOpen(false)}>
            <Calendar
              value={value}
              event={event}
              optional={optional}
              onPick={(iso) => {
                onChange(iso);
                setOpen(false);
              }}
            />
          </Sheet>,
          document.body,
        )}
    </>
  );
}
