// Advances and settlements, loans and their payments, budgets.
import { useEffect, useState } from "react";
import { api, ApiError, today } from "../api";
import { Input, Segmented, Select, Sheet, TextArea, Toggle } from "../components/ui";
import { decimalInput, money, moneyInput, parseDecimal, parseMoney } from "../format";
import { categoryOptions, loanCategoryOptions, TRANSFER_KINDS } from "../labels";
import { navigate } from "../router";
import { useApp } from "../state";
import type { Loan, Payment, PaymentKind, Transfer, TransferKind } from "../types";
import { DateField } from "../components/DateField";
import { amount, Form, useForm } from "./useForm";

export function TransferForm({
  transfer,
  tourId = "",
  preset,
  onSaved,
  onClose,
}: {
  transfer?: Transfer;
  tourId?: string;
  preset?: Partial<Transfer>;
  onSaved?: () => void;
  onClose: () => void;
}) {
  const { data, save, remove, notify } = useApp();
  const start = { ...preset, ...transfer };
  const tour = start.tour_id ?? tourId;
  const company = data.tours.find((t) => t.id === tour)?.kind === "company";
  const kinds = (Object.keys(TRANSFER_KINDS) as TransferKind[]).filter((k) =>
    k === "received_from" || k === "paid_to"
      ? data.people.length > 0
      : // Company money only makes sense against a company tour (or an entry that already has it).
        company || k === transfer?.kind,
  );
  const form = useForm({
    kind: start.kind && kinds.includes(start.kind) ? start.kind : (kinds[0] ?? ("advance" as TransferKind)),
    person_id: start.person_id ?? "",
    amount: moneyInput(start.amount ?? 0),
    date: start.date ?? today(),
    note: start.note ?? "",
  });
  const v = form.values;
  const withPerson = v.kind === "received_from" || v.kind === "paid_to";

  const submit = form.submit(async () => {
    await save(
      "transfer",
      {
        tour_id: tour,
        kind: v.kind,
        person_id: withPerson ? v.person_id : "",
        amount: amount("amount", v.amount),
        date: v.date,
        note: v.note.trim(),
      },
      transfer?.id,
    );
    notify("Saved");
    onSaved?.();
    onClose();
  });

  return (
    <Sheet title={transfer ? "Edit payment" : company ? "Record advance or payment" : "Record a payment"} onClose={onClose}>
      <Form
        onSubmit={submit}
        busy={form.busy}
        message={form.message}
        submitLabel="Save"
        onDelete={
          transfer &&
          (async () => {
            if (await remove("transfer", transfer.id, "Delete this entry?", "The tour balance changes.")) onClose();
          })
        }
      >
        <Select
          label="What happened"
          value={v.kind}
          onChange={(x) => form.set("kind", x as TransferKind)}
          options={kinds.map((value) => ({ value, label: TRANSFER_KINDS[value] }))}
        />
        {withPerson && (
          <Select
            label="Person"
            value={v.person_id}
            onChange={(x) => form.set("person_id", x)}
            error={form.errors.person_id}
            options={[{ value: "", label: "Choose…" }, ...data.people.map((p) => ({ value: p.id, label: p.name }))]}
          />
        )}
        <div className="grid-2">
          <Input label="Amount (₹)" value={v.amount} onChange={(x) => form.set("amount", x)} error={form.errors.amount} inputMode="decimal" />
          <DateField label="Date" value={v.date} onChange={(x) => form.set("date", x)} error={form.errors.date} />
        </div>
        <Input label="Note" value={v.note} onChange={(x) => form.set("note", x)} />
      </Form>
    </Sheet>
  );
}

function nextMonth(iso: string) {
  const [y, m, d] = iso.split("-").map(Number);
  const last = new Date(y, m + 1, 0).getDate();
  const date = new Date(y, m, Math.min(d, last), 12);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

export function LoanForm({ loan, onClose }: { loan?: Loan; onClose: () => void }) {
  const { save, remove, notify } = useApp();
  const form = useForm({
    name: loan?.name ?? "",
    category: loan?.category || "personal",
    reference: loan?.reference ?? "",
    counterparty: loan?.counterparty ?? "",
    direction: loan?.direction ?? "borrowed",
    style: loan?.style ?? "emi",
    principal: moneyInput(loan?.principal ?? 0),
    rate: decimalInput(loan?.rate_bps ?? 0, 2),
    tenure: loan?.tenure_months ? String(loan.tenure_months) : "",
    start_date: loan?.start_date ?? today(),
    first_due_date: loan?.first_due_date || nextMonth(today()),
    emi: moneyInput(loan?.emi ?? 0),
    paid_before: loan?.paid_before ? String(loan.paid_before) : "",
    notes: loan?.notes ?? "",
    local_only: loan?.local_only ?? false,
  });
  const v = form.values;
  const emiStyle = v.style === "emi";
  const [estimate, setEstimate] = useState(0);

  useEffect(() => {
    const principal = parseMoney(v.principal);
    const rate_bps = parseDecimal(v.rate || "0", 2);
    const tenure_months = Number(v.tenure);
    if (!emiStyle || !principal || rate_bps === null || !Number.isInteger(tenure_months) || tenure_months < 1) {
      setEstimate(0);
      return;
    }
    let stale = false;
    api<{ emi: number }>({ op: "loan_preview", principal, rate_bps, tenure_months })
      .then((r) => !stale && setEstimate(r.emi))
      .catch(() => !stale && setEstimate(0));
    return () => {
      stale = true;
    };
  }, [emiStyle, v.principal, v.rate, v.tenure]);

  const submit = form.submit(async () => {
    const rate = parseDecimal(v.rate || "0", 2);
    if (rate === null) throw new ApiError("Enter the yearly rate, like 10.5.", "rate_bps");
    const id = await save(
      "loan",
      {
        name: v.name.trim(),
        category: v.category,
        reference: v.reference.trim(),
        counterparty: v.counterparty.trim(),
        direction: v.direction,
        style: v.style,
        principal: amount("principal", v.principal),
        rate_bps: emiStyle ? rate : 0,
        tenure_months: emiStyle ? Math.trunc(Number(v.tenure)) || 0 : 0,
        start_date: v.start_date,
        first_due_date: emiStyle ? v.first_due_date : "",
        emi: emiStyle ? amount("emi", v.emi) : 0,
        paid_before: emiStyle ? Math.max(0, Math.trunc(Number(v.paid_before)) || 0) : 0,
        notes: v.notes.trim(),
        closed: loan?.closed ?? false,
        closed_on: loan?.closed_on ?? "",
        local_only: v.local_only,
      },
      loan?.id,
    );
    notify(loan ? "Loan updated" : "Loan added");
    onClose();
    if (!loan) navigate(`/loans/l/${id}`);
  });

  return (
    <Sheet title={loan ? "Edit loan" : "Add loan"} onClose={onClose}>
      <Form
        onSubmit={submit}
        busy={form.busy}
        message={form.message}
        submitLabel={loan ? "Save changes" : "Add loan"}
        onDelete={
          loan &&
          (async () => {
            if (await remove("loan", loan.id, `Delete ${loan.name}?`, "Its payment history is deleted too.")) {
              onClose();
              navigate("/loans/list");
            }
          })
        }
      >
        <Segmented
          label="Direction"
          value={v.direction}
          onChange={(x) => form.set("direction", x)}
          options={[
            { value: "borrowed", label: "I borrowed" },
            { value: "lent", label: "I lent" },
          ]}
        />
        <Segmented
          label="Repayment"
          value={v.style}
          onChange={(x) => form.set("style", x)}
          options={[
            { value: "emi", label: "Monthly EMI" },
            { value: "flexible", label: "Any amounts" },
          ]}
        />
        <div className="grid-2">
          <Input label="Name" value={v.name} onChange={(x) => form.set("name", x)} error={form.errors.name} placeholder="Bike loan" />
          <Select label="Kind of loan" value={v.category} onChange={(x) => form.set("category", x)} error={form.errors.category} options={loanCategoryOptions} />
          <Input label={v.direction === "borrowed" ? "Lender" : "Borrower"} value={v.counterparty} onChange={(x) => form.set("counterparty", x)} />
          <Input label="Account or reference number" value={v.reference} onChange={(x) => form.set("reference", x)} />
          <Input label="Amount (₹)" value={v.principal} onChange={(x) => form.set("principal", x)} error={form.errors.principal} inputMode="decimal" />
          <DateField label={v.direction === "borrowed" ? "Taken on" : "Given on"} value={v.start_date} onChange={(x) => form.set("start_date", x)} error={form.errors.start_date} />
          {emiStyle && (
            <>
              <Input label="Interest a year (%)" value={v.rate} onChange={(x) => form.set("rate", x)} error={form.errors.rate_bps} inputMode="decimal" placeholder="0" />
              <Input label="Months" value={v.tenure} onChange={(x) => form.set("tenure", x)} error={form.errors.tenure_months} inputMode="numeric" />
              <DateField label="First instalment due" value={v.first_due_date} onChange={(x) => form.set("first_due_date", x)} error={form.errors.first_due_date} />
              <Input
                label="Lender's EMI (₹, optional)"
                value={v.emi}
                onChange={(x) => form.set("emi", x)}
                error={form.errors.emi}
                hint={estimate ? `Calculated: ${money(estimate)} a month` : undefined}
                inputMode="decimal"
              />
              <Input label="Instalments already paid" value={v.paid_before} onChange={(x) => form.set("paid_before", x)} inputMode="numeric" placeholder="0" />
            </>
          )}
        </div>
        <TextArea label="Notes" value={v.notes} onChange={(x) => form.set("notes", x)} />
        <Toggle label="Keep on this device only" hint="Never sent to linked devices" checked={v.local_only} onChange={(x) => form.set("local_only", x)} />
      </Form>
    </Sheet>
  );
}

export function PaymentForm({
  loan,
  payment,
  preset,
  onClose,
}: {
  loan: Loan;
  payment?: Payment;
  preset?: Partial<Payment>;
  onClose: () => void;
}) {
  const { save, remove, notify } = useApp();
  const emi = loan.style === "emi";
  const next = loan.status.next;
  const start = { ...preset, ...payment };
  const defaultKind: PaymentKind = emi ? (next ? "instalment" : "prepayment") : "repayment";
  const form = useForm({
    kind: start.kind ?? defaultKind,
    amount: moneyInput(start.amount ?? (defaultKind === "instalment" ? (next?.amount ?? 0) : 0)),
    date: start.date ?? today(),
    note: start.note ?? "",
  });
  const v = form.values;
  const number = payment?.number || start.number || next?.number || 0;

  const submit = form.submit(async () => {
    await save(
      "payment",
      {
        loan_id: loan.id,
        kind: v.kind,
        number: v.kind === "instalment" ? number : 0,
        amount: amount("amount", v.amount),
        date: v.date,
        note: v.note.trim(),
      },
      payment?.id,
    );
    notify(payment ? "Payment saved" : "Payment recorded");
    onClose();
  });

  return (
    <Sheet title={payment ? "Edit payment" : `Payment: ${loan.name}`} onClose={onClose}>
      <Form
        onSubmit={submit}
        busy={form.busy}
        message={form.message}
        submitLabel="Save payment"
        onDelete={
          payment &&
          (async () => {
            if (await remove("payment", payment.id, "Delete this payment?", "The amount owed changes.")) onClose();
          })
        }
      >
        {emi && !payment && (
          <Segmented
            label="Type"
            value={v.kind}
            onChange={(x) => form.set("kind", x)}
            options={[
              ...(next ? [{ value: "instalment" as const, label: `Instalment ${next.number}` }] : []),
              { value: "prepayment", label: "Part prepayment" },
            ]}
          />
        )}
        <div className="grid-2">
          <Input label="Amount (₹)" value={v.amount} onChange={(x) => form.set("amount", x)} error={form.errors.amount ?? form.errors.number} inputMode="decimal" />
          <DateField label="Date" value={v.date} onChange={(x) => form.set("date", x)} error={form.errors.date} />
        </div>
        <Input label="Note" value={v.note} onChange={(x) => form.set("note", x)} />
      </Form>
    </Sheet>
  );
}

export function BudgetForm({ category, monthly, onClose }: { category?: string; monthly?: number; onClose: () => void }) {
  const { save, remove, notify } = useApp();
  const form = useForm({ category: category ?? "total", monthly: moneyInput(monthly ?? 0) });
  const v = form.values;
  const submit = form.submit(async () => {
    await save("budget", { monthly: amount("monthly", v.monthly) }, v.category);
    notify("Budget saved");
    onClose();
  });
  return (
    <Sheet title="Monthly budget" onClose={onClose}>
      <Form
        onSubmit={submit}
        busy={form.busy}
        message={form.message}
        submitLabel="Save budget"
        onDelete={
          category
            ? async () => {
                if (await remove("budget", category, "Delete this budget?", "Your expenses are not affected.")) onClose();
              }
            : undefined
        }
      >
        {!category && (
          <Select
            label="For"
            value={v.category}
            onChange={(x) => form.set("category", x)}
            error={form.errors.category}
            options={[{ value: "total", label: "All spending" }, ...categoryOptions]}
          />
        )}
        <Input label="Limit each month (₹)" value={v.monthly} onChange={(x) => form.set("monthly", x)} error={form.errors.monthly} inputMode="decimal" />
      </Form>
    </Sheet>
  );
}

/** Closing a loan: the date, and anything paid to settle it. */
export function CloseLoanForm({ loan, onClose }: { loan: Loan; onClose: () => void }) {
  const { reload, notify } = useApp();
  const form = useForm({ date: today(), amount: moneyInput(loan.status.outstanding) });
  const v = form.values;
  const submit = form.submit(async () => {
    await api({ op: "loan_close", loan_id: loan.id, date: v.date, amount: amount("amount", v.amount) });
    await reload();
    notify(`${loan.name} closed`);
    onClose();
  });
  return (
    <Sheet title={`Close ${loan.name}`} onClose={onClose}>
      <Form onSubmit={submit} busy={form.busy} message={form.message} submitLabel="Close loan">
        <div className="grid-2">
          <DateField label="Closed on" value={v.date} onChange={(x) => form.set("date", x)} error={form.errors.date} />
          <Input
            label="Paid to settle it (₹)"
            value={v.amount}
            onChange={(x) => form.set("amount", x)}
            error={form.errors.amount}
            hint="The outstanding principal. Change it to your lender's figure, or empty it if nothing was paid."
            inputMode="decimal"
          />
        </div>
        <p className="muted">It moves to closed loans. You can reopen it.</p>
      </Form>
    </Sheet>
  );
}
