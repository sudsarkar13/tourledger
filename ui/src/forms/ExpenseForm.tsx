import { useEffect, useRef, useState } from "react";
import { api, ApiError, today } from "../api";
import { BillsField } from "../components/Bills";
import type { Picture } from "../pictures";
import { Chips, Input, Segmented, Select, Sheet, Toggle } from "../components/ui";
import { decimalInput, moneyInput, parseDecimal } from "../format";
import { categoryOptions, isWorkTrip } from "../labels";
import { useApp } from "../state";
import type { Claim, Expense, Tour } from "../types";
import { DateField } from "../components/DateField";
import { amount, Form, useForm } from "./useForm";

const TRANSPORT = ["local", "train", "flight", "bus", "fuel"];
const MODES = ["Auto", "Cab", "Metro", "E-rickshaw", "Bus", "Train", "Flight", "Own vehicle", "Walk"];
const METHODS = ["", "Cash", "UPI", "Debit card", "Credit card", "Net banking"];

function defaultDate(tour?: Tour) {
  const now = today();
  if (!tour || (now >= tour.start_date && now <= tour.end_date)) return now;
  return tour.start_date;
}

export function ExpenseForm({
  expense,
  tourId = "",
  preset,
  onSaved,
  onClose,
}: {
  expense?: Expense;
  tourId?: string;
  preset?: Partial<Expense>;
  onSaved?: () => void;
  onClose: () => void;
}) {
  const { data, save, remove, notify, reload } = useApp();
  const start = { ...preset, ...expense };
  const tourOf = (id: string) => data.tours.find((t) => t.id === id);
  const initialTour = start.tour_id ?? tourId;
  const claimFor = (id: string): Claim => (isWorkTrip(tourOf(id)) ? "reimbursable" : "personal");

  const form = useForm({
    tour_id: initialTour,
    date: start.date ?? defaultDate(tourOf(initialTour)),
    category: start.category ?? "local",
    description: start.description ?? "",
    from: start.from ?? "",
    to: start.to ?? "",
    mode: start.mode ?? "",
    amount: moneyInput(start.amount ?? 0),
    foreign: !!start.currency && start.currency !== "INR",
    currency: start.currency && start.currency !== "INR" ? start.currency : "",
    rate: decimalInput(start.rate_ppm ?? 0, 6),
    paid_by: start.paid_by ?? "me",
    claim: start.claim ?? claimFor(initialTour),
    rejected: start.rejected ?? false,
    payment_method: start.payment_method ?? "",
    bill_no: start.bill_no ?? "",
    split_with: start.split_with ?? [],
    local_only: start.local_only ?? false,
  });
  const v = form.values;
  const [keepQuick, setKeepQuick] = useState(false);
  // Until a category is picked by hand (or the entry already has one), it
  // follows the note, so "powder tea" is not filed as travel.
  const [picked, setPicked] = useState(!!start.category);
  const [fromNote, setFromNote] = useState<"" | "words" | "history">("");
  const [pictures, setPictures] = useState<Picture[]>([]);
  // Set once a new expense is stored, so retrying after a failed picture does
  // not create a second expense.
  const savedId = useRef(expense?.id);
  const tour = tourOf(v.tour_id);
  const transport = TRANSPORT.includes(v.category);
  const stage = !tour
    ? undefined
    : v.date < tour.start_date
      ? "Before the event: counted as the journey"
      : v.date > tour.end_date
        ? "After the event: counted as the return"
        : undefined;
  const companyPaid = v.paid_by === "company";
  // A claim is made per tour.
  const reimbursable = !!v.tour_id && v.claim === "reimbursable";
  useEffect(() => {
    if (picked) return;
    let stale = false;
    api<{ category: string | null; from: "" | "words" | "history" }>({ op: "suggest_category", text: v.description })
      .then((s) => {
        if (stale) return;
        // A route with no telling word is a ride; an unknown note is "other".
        const route = !!(v.from.trim() || v.to.trim());
        const category = s.category ?? (route || !v.description.trim() ? "local" : "other");
        setFromNote(s.category ? s.from : "");
        form.setValues((c) => (c.category === category ? c : { ...c, category }));
      })
      .catch(() => {
        // Only a convenience: the category can still be picked by hand.
      });
    return () => {
      stale = true;
    };
  }, [picked, v.description, v.from, v.to]);

  // Saving a device-only journey as a quick entry would sync its details.
  const deviceOnly = tour ? tour.local_only : v.local_only;
  // On a claim this records who it was for; otherwise it also divides the cost.
  const canSplit = data.people.length > 0;

  // People on the tour first; anyone already chosen stays visible.
  const sharers = data.people.filter(
    (p) => !tour || !tour.participants.length || tour.participants.includes(p.id) || v.split_with.includes(p.id),
  );

  // An error inside the collapsed section must not stay hidden.
  const hiddenError = !!(form.errors.currency || form.errors.rate_ppm);
  const [showMore, setShowMore] = useState(!!start.currency && start.currency !== "INR");

  const submit = form.submit(async () => {
    const rate = v.foreign ? parseDecimal(v.rate, 6) : 0;
    if (rate === null) throw new ApiError("Enter the exchange rate, like 88.45.", "rate_ppm");
    const body = {
      tour_id: v.tour_id,
      date: v.date,
      category: v.category,
      description: v.description.trim(),
      from: transport ? v.from.trim() : "",
      to: transport ? v.to.trim() : "",
      mode: transport ? v.mode.trim() : "",
      amount: amount("amount", v.amount),
      currency: v.foreign ? v.currency.trim().toUpperCase() : "",
      rate_ppm: rate,
      paid_by: v.paid_by,
      claim: v.tour_id ? v.claim : "personal",
      rejected: reimbursable && v.rejected,
      payment_method: v.payment_method,
      bill_no: v.bill_no.trim(),
      split_with: canSplit ? v.split_with : [],
      local_only: !v.tour_id && v.local_only,
    };
    savedId.current = await save("expense", body, savedId.current);
    // One at a time; each saved one leaves the list, so none is sent twice.
    for (let i = 0; i < pictures.length; i++) {
      await api({ op: "add_attachment", expense_id: savedId.current, data: pictures[i].data });
      setPictures(pictures.slice(i + 1));
    }
    if (pictures.length) await reload();
    if (keepQuick && !deviceOnly) {
      const route = body.from && body.to ? `${body.from} → ${body.to}` : "";
      await save("quick", {
        label: route || body.description || "Quick entry",
        category: body.category,
        from: body.from,
        to: body.to,
        mode: body.mode,
        amount: body.amount,
        claim: body.claim,
      });
    }
    notify(expense ? "Expense updated" : "Expense added");
    onSaved?.();
    onClose();
  });

  const onDelete =
    expense &&
    (async () => {
      if (await remove("expense", expense.id, "Delete this expense?", "It is removed from linked devices too.")) {
        onClose();
      }
    });

  return (
    <Sheet title={expense ? "Edit expense" : "Add expense"} onClose={onClose}>
      <Form
        onSubmit={submit}
        busy={form.busy}
        message={form.message}
        submitLabel={expense ? "Save changes" : "Add expense"}
        onDelete={onDelete}
      >
        {!expense && data.quick.length > 0 && (
          <Chips
            label="Quick entries"
            options={data.quick.map((q) => ({ value: q.id, label: q.label }))}
            selected={[]}
            onToggle={(id) => {
              const q = data.quick.find((x) => x.id === id);
              if (!q) return;
              setPicked(true);
              form.setValues((current) => ({
                ...current,
                category: q.category,
                from: q.from,
                to: q.to,
                mode: q.mode,
                amount: moneyInput(q.amount),
                claim: tour ? claimFor(current.tour_id) : q.claim,
              }));
            }}
          />
        )}
        <Input
          label="What was it? (note)"
          value={v.description}
          onChange={(x) => form.set("description", x)}
          placeholder="Powder tea, lunch, auto to station…"
        />
        <div className="grid-2">
          <Input
            label={v.foreign ? `Amount (${v.currency || "foreign"})` : "Amount (₹)"}
            value={v.amount}
            onChange={(x) => form.set("amount", x)}
            error={form.errors.amount}
            inputMode="decimal"
            autoFocus={!expense}
          />
          <DateField label="Date" event={tour && { from: tour.start_date, to: tour.end_date }} hint={stage} value={v.date} onChange={(x) => form.set("date", x)} error={form.errors.date} />
          <Select
            label="Category"
            value={v.category}
            onChange={(x) => {
              setPicked(true);
              setFromNote("");
              form.set("category", x);
            }}
            options={categoryOptions}
            error={form.errors.category}
            hint={
              fromNote === "history"
                ? "As you filed this note before"
                : fromNote === "words"
                  ? "Chosen from your note"
                  : undefined
            }
          />
          <Select
            label="Tour"
            value={v.tour_id}
            onChange={(x) => form.setValues((c) => ({ ...c, tour_id: x, claim: claimFor(x) }))}
            error={form.errors.tour_id}
            options={[
              { value: "", label: "Not part of a tour" },
              ...data.tours.map((t) => ({ value: t.id, label: t.name })),
            ]}
          />
        </div>
        {transport && (
          <div className="grid-3">
            <Input label="From" value={v.from} onChange={(x) => form.set("from", x)} placeholder="Home" />
            <Input label="To" value={v.to} onChange={(x) => form.set("to", x)} placeholder="Railway station" />
            <Input label="By" value={v.mode} onChange={(x) => form.set("mode", x)} list={MODES} placeholder="Auto" />
          </div>
        )}
        <div className="grid-2">
          <Select
            label="Paid by"
            value={v.paid_by}
            onChange={(x) => form.set("paid_by", x)}
            error={form.errors.paid_by}
            options={[
              { value: "me", label: "Me" },
              { value: "company", label: "Company (booked or paid)" },
              ...data.people.map((p) => ({ value: p.id, label: p.name })),
            ]}
          />
          {v.paid_by === "me" && v.tour_id && (
            <Segmented
              label="Claim"
              value={v.claim}
              onChange={(x) => form.set("claim", x)}
              options={[
                { value: "reimbursable", label: "Reimbursable" },
                { value: "personal", label: "Personal" },
              ]}
            />
          )}
        </div>
        {canSplit && (
          <Chips
            label={reimbursable || companyPaid ? "Spent for" : "Shared equally between"}
            options={[{ value: "me", label: "Me" }, ...sharers.map((p) => ({ value: p.id, label: p.name }))]}
            selected={v.split_with}
            error={form.errors.split_with}
            onToggle={(id) =>
              form.set(
                "split_with",
                v.split_with.includes(id) ? v.split_with.filter((x) => x !== id) : [...v.split_with, id],
              )
            }
          />
        )}
        <BillsField expenseId={savedId.current} pending={pictures} onPending={setPictures} />
        <details className="more" open={showMore || hiddenError} onToggle={(event) => setShowMore(event.currentTarget.open)}>
          <summary>More details</summary>
          <div className="grid-2">
            <Select
              label="Paid with"
              value={v.payment_method}
              onChange={(x) => form.set("payment_method", x)}
              options={METHODS.map((m) => ({ value: m, label: m || "Not noted" }))}
            />
            <Input label="Bill number" value={v.bill_no} onChange={(x) => form.set("bill_no", x)} />
          </div>
          <Toggle label="Paid in a foreign currency" checked={v.foreign} onChange={(x) => form.set("foreign", x)} />
          {v.foreign && (
            <div className="grid-2">
              <Input
                label="Currency code"
                value={v.currency}
                onChange={(x) => form.set("currency", x.toUpperCase())}
                error={form.errors.currency}
                placeholder="USD"
              />
              <Input
                label="₹ for 1 unit"
                value={v.rate}
                onChange={(x) => form.set("rate", x)}
                error={form.errors.rate_ppm}
                inputMode="decimal"
              />
            </div>
          )}
          {expense && reimbursable && !companyPaid && (
            <Toggle
              label="Company rejected this claim"
              hint="Counts as your own cost"
              checked={v.rejected}
              onChange={(x) => form.set("rejected", x)}
            />
          )}
          {!v.tour_id && (
            <Toggle
              label="Keep on this device only"
              hint="Never sent to linked devices"
              checked={v.local_only}
              onChange={(x) => form.set("local_only", x)}
            />
          )}
          {!expense && !deviceOnly && <Toggle label="Save as a quick entry" checked={keepQuick} onChange={setKeepQuick} />}
        </details>
      </Form>
    </Sheet>
  );
}
