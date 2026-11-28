import { useEffect, useRef, useState } from "react";
import { api, ApiError, today } from "../api";
import { BillsField } from "../components/Bills";
import type { Picture } from "../pictures";
import { Chips, Input, Segmented, Select, Sheet, Toggle } from "../components/ui";
import { day, decimalInput, moneyInput, parseDecimal, routeOf } from "../format";
import { CATEGORIES, categoryOptions, isWorkTrip } from "../labels";
import { useApp } from "../state";
import type { Claim, Expense, Tour } from "../types";
import { DateField } from "../components/DateField";
import { amount, Form, useForm } from "./useForm";

const TRANSPORT = ["local", "metro", "train", "flight", "bus", "fuel"];
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
  // The way back is either covered by this entry's amount, or saved as its own
  // entry whose fare and date follow the first journey until they are changed.
  // One fare is offered first: it never adds money to the claim by itself.
  const [returning, setReturning] = useState(!!start.round_trip);
  const [oneFare, setOneFare] = useState(true);
  // Splitting an amount that covered both ways: the return fare must be typed.
  const [wayBack, setWayBack] = useState<{ fare: string | null; date: string | null }>({
    fare: start.round_trip ? "" : null,
    date: null,
  });
  const returnedId = useRef<string | undefined>(undefined);
  const tour = tourOf(v.tour_id);
  // A category from a newer version may be a journey: keep its From, To and By,
  // or saving here would erase them there.
  const unknown = !(v.category in CATEGORIES);
  const transport = TRANSPORT.includes(v.category) || (unknown && !!(v.from || v.to || v.mode));
  const stageOf = (date: string) =>
    !tour
      ? undefined
      : date < tour.start_date
        ? "Before the event: counted as the journey"
        : date > tour.end_date
          ? "After the event: counted as the return"
          : undefined;
  const returnFare = wayBack.fare ?? v.amount;
  const returnDate = wayBack.date ?? v.date;
  // Editing an entry whose way back is already there: say so before adding another.
  const same = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase();
  const recorded =
    expense && v.from.trim() && v.to.trim()
      ? data.expenses.find(
          (x) => x.id !== expense.id && x.tour_id === v.tour_id && x.date >= v.date && same(x.from, v.to) && same(x.to, v.from),
        )
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
      round_trip: transport && returning && oneFare,
    };
    if (start.round_trip && returning && !oneFare && !body.amount) {
      throw new ApiError("Enter the fare for the way there.", "amount");
    }
    // Checked before anything is saved, so a mistake here leaves nothing half done.
    let back: typeof body | undefined;
    if (returning && transport && !oneFare) {
      const fare = amount("return_amount", returnFare);
      if (!fare) throw new ApiError("Enter the return fare.", "return_amount");
      if (returnDate < v.date) throw new ApiError("The return cannot be before the journey.", "return_date");
      back = {
        ...body,
        date: returnDate,
        from: body.to,
        to: body.from,
        amount: fare,
        description: body.description && `${body.description} (return)`,
        bill_no: "",
        rejected: false,
      };
    }
    savedId.current = await save("expense", body, savedId.current);
    // Kept apart from the first, so a retry updates it rather than adding a third.
    if (back) {
      try {
        returnedId.current = await save("expense", back, returnedId.current);
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        // Said under the return, so it is not taken for the whole entry failing.
        throw new ApiError(`The journey was saved, the return was not: ${reason}`, "return_amount");
      }
    }
    // A return saved by an earlier try that is no longer wanted.
    if (!back && returnedId.current) {
      await api({ op: "delete", kind: "expense", id: returnedId.current });
      returnedId.current = undefined;
      await reload();
    }
    // One at a time; each saved one leaves the list, so none is sent twice.
    for (let i = 0; i < pictures.length; i++) {
      await api({ op: "add_attachment", expense_id: savedId.current, data: pictures[i].data });
      setPictures(pictures.slice(i + 1));
    }
    if (pictures.length) await reload();
    if (keepQuick && !deviceOnly) {
      const route = routeOf(body.from, body.to, body.round_trip);
      await save("quick", {
        label: route || body.description || "Quick entry",
        category: body.category,
        from: body.from,
        to: body.to,
        mode: body.mode,
        amount: body.amount,
        claim: body.claim,
        round_trip: body.round_trip,
      });
    }
    if (returnedId.current) notify(expense ? "Expense updated, return added" : "Journey and return added");
    else notify(expense ? "Expense updated" : "Expense added");
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
        submitLabel={
          returning && transport && !oneFare
            ? expense
              ? "Save and add return"
              : "Add both journeys"
            : expense
              ? "Save changes"
              : "Add expense"
        }
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
              setReturning(!!q.round_trip);
              setOneFare(true);
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
          <DateField label="Date" event={tour && { from: tour.start_date, to: tour.end_date }} hint={stageOf(v.date)} value={v.date} onChange={(x) => form.set("date", x)} error={form.errors.date} />
          <Select
            label="Category"
            value={v.category}
            onChange={(x) => {
              setPicked(true);
              setFromNote("");
              form.set("category", x);
            }}
            options={unknown ? [...categoryOptions, { value: v.category, label: "Other (from a newer version)" }] : categoryOptions}
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
        {transport && (
          <>
            <Toggle
              label="Return journey too"
              hint="There and back"
              checked={returning}
              onChange={setReturning}
            />
            {returning && (
              <>
                <Segmented
                  label="Fare"
                  value={oneFare ? "one" : "two"}
                  onChange={(x) => {
                    setOneFare(x === "one");
                    // That amount covered both ways. Keeping it and adding a
                    // return would count the way back twice.
                    if (start.round_trip) form.set("amount", x === "two" ? "" : moneyInput(start.amount ?? 0));
                  }}
                  options={[
                    { value: "one", label: "One for both ways" },
                    { value: "two", label: "Separate fares" },
                  ]}
                />
                <p className="muted">
                  {[
                    oneFare
                      ? `One entry: the amount above counts once${
                          v.from.trim() && v.to.trim()
                            ? `, listed as ${routeOf(v.from.trim(), v.to.trim(), true)}`
                            : ""
                        }.`
                      : `A second entry, ${
                          v.from.trim() && v.to.trim() ? routeOf(v.to.trim(), v.from.trim()) : "From and To the other way round"
                        }, paid the same way.`,
                    recorded &&
                      (oneFare
                        ? `A way back is already recorded on ${day(recorded.date)}: delete it if this fare covers both.`
                        : `That way back is already recorded on ${day(recorded.date)}.`),
                    !oneFare && start.round_trip && "The old amount covered both ways: enter the fare there above.",
                    !oneFare && pictures.length > 0 && "Bill pictures stay with the first journey.",
                  ]
                    .filter(Boolean)
                    .join(" ")}
                </p>
              </>
            )}
            {returning && !oneFare && (
              <div className="grid-2">
                <Input
                  label={v.foreign ? `Return fare (${v.currency || "foreign"})` : "Return fare (₹)"}
                  value={returnFare}
                  onChange={(fare) => setWayBack((w) => ({ ...w, fare }))}
                  error={form.errors.return_amount}
                  inputMode="decimal"
                />
                <DateField
                  label="Return date"
                  event={tour && { from: tour.start_date, to: tour.end_date }}
                  hint={stageOf(returnDate)}
                  value={returnDate}
                  onChange={(date) => setWayBack((w) => ({ ...w, date }))}
                  error={form.errors.return_date}
                />
              </div>
            )}
          </>
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
        <Select
          label="Paid with"
          value={v.payment_method}
          onChange={(x) => form.set("payment_method", x)}
          options={METHODS.map((m) => ({ value: m, label: m || "Not noted" }))}
        />
        <BillsField expenseId={savedId.current} pending={pictures} onPending={setPictures} />
        <details className="more" open={showMore || hiddenError} onToggle={(event) => setShowMore(event.currentTarget.open)}>
          <summary>More details</summary>
          <Input label="Bill number" value={v.bill_no} onChange={(x) => form.set("bill_no", x)} />
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
