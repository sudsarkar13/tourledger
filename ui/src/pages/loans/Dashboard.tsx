import { CalendarClock, Check, CircleAlert, HandCoins, Landmark, MessageSquareText } from "lucide-react";
import { useState } from "react";
import { api, today } from "../../api";
import { Meter } from "../../components/charts";
import { Button, Card, EmptyState, List, ListRow, Money, Select, Sheet, Stat, TextArea } from "../../components/ui";
import { PaymentForm } from "../../forms/MoneyForms";
import { Form, useForm } from "../../forms/useForm";
import { fullDay, money, plural } from "../../format";
import { useApp } from "../../state";
import type { Due } from "../../types";
import { isRunning, LoansHeader, nextLine, useInstalments } from "./common";

/** Paste a lender's message; choose the loan; check the payment before saving. */
function BankMessage({ onClose }: { onClose: () => void }) {
  const { data } = useApp();
  const loans = data.loans.filter((l) => l.direction === "borrowed" && isRunning(l));
  const form = useForm({ text: "" });
  const [target, setTarget] = useState("");
  const [draft, setDraft] = useState<{ amount: number; date: string } | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const loan = loans.find((l) => l.id === target);
  const instalment = !!draft && loan?.status.next?.amount === draft.amount;
  const recorded =
    !!draft && data.payments.some((p) => p.loan_id === target && p.date === draft.date && p.amount === draft.amount);

  if (draft && confirmed && loan) {
    // Only an exact instalment amount ticks an instalment.
    const kind = loan.style !== "emi" ? "repayment" : instalment ? "instalment" : "prepayment";
    return <PaymentForm loan={loan} preset={{ kind, amount: draft.amount, date: draft.date }} onClose={onClose} />;
  }
  const read = form.submit(async () => {
    const result = await api<{ amount: number; date: string }>({ op: "parse_message", text: form.values.text, today: today() });
    // Preselect only a loan whose instalment is exactly this amount.
    setTarget(loans.find((l) => l.status.next?.amount === result.amount)?.id ?? "");
    setDraft(result);
  });
  return (
    <Sheet title="Payment from a bank message" onClose={onClose}>
      <Form
        onSubmit={
          draft
            ? (e) => {
                e.preventDefault();
                if (loan) setConfirmed(true);
              }
            : read
        }
        busy={form.busy}
        message={form.message}
        submitLabel={draft ? "Continue" : "Read message"}
      >
        <TextArea
          label="Message"
          value={form.values.text}
          onChange={(x) => {
            form.set("text", x);
            setDraft(null);
          }}
          error={form.errors.text}
          placeholder="Rs.4,215.63 debited from A/c XX1234 on 05-11-26 towards EMI…"
        />
        {draft && (
          <>
            <p>
              Found <strong>{money(draft.amount)}</strong> on {fullDay(draft.date)}. Which loan is it for?
            </p>
            <Select
              label="Loan"
              value={target}
              onChange={setTarget}
              options={[{ value: "", label: "Choose a loan…" }, ...loans.map((l) => ({ value: l.id, label: l.name }))]}
            />
            {loan && loan.style === "emi" && !instalment && (
              <p className="error">
                This is not the instalment amount of {loan.name}, so no month will be ticked. It is recorded as a part
                prepayment; cancel if it was a fee or charge.
              </p>
            )}
            {recorded && <p className="error">A payment of this amount on this date is already recorded for this loan.</p>}
          </>
        )}
        <p className="muted">Only what you paste is read. The app has no access to your messages.</p>
      </Form>
    </Sheet>
  );
}

export function LoanDashboard() {
  const { data } = useApp();
  const p = data.portfolio;
  const { busy, mark } = useInstalments();
  const [pasting, setPasting] = useState(false);
  const running = data.loans.filter((l) => l.direction === "borrowed" && isRunning(l));
  const lent = data.loans.filter((l) => l.direction === "lent" && isRunning(l));
  const due = p.this_month_due.count + p.this_month_paid.count;

  const dueRows = (items: Due[], late: boolean) => (
    <List>
      {items.map((d) => (
        <ListRow
          key={`${d.loan_id}-${d.number}`}
          icon={late ? CircleAlert : CalendarClock}
          title={d.name}
          detail={`Instalment ${d.number} · ${late ? "was due" : "due"} ${fullDay(d.due)}`}
          value={<Money paise={d.amount} tone={late ? "bad" : undefined} />}
          href={`#/loans/l/${d.loan_id}`}
        >
          <Button icon={Check} disabled={busy} onClick={() => mark(d.loan_id, d.number, true)}>
            Paid
          </Button>
        </ListRow>
      ))}
    </List>
  );

  return (
    <>
      <LoansHeader tab="dashboard" context="What you owe, what is due and what was missed" />
      <div className="stats">
        <Stat label="Still to repay" value={money(p.outstanding)} hint={plural(p.running, "running loan")} />
        <Stat label="EMIs per month" value={money(p.monthly_emi)} />
        <Stat
          label="This month"
          value={due ? `${p.this_month_paid.count} of ${due} paid` : "Nothing due"}
          hint={p.this_month_due.amount ? `${money(p.this_month_due.amount)} still to pay` : undefined}
        />
        <Stat
          label="Overdue"
          value={money(p.overdue_total)}
          tone={p.overdue.length ? "bad" : undefined}
          hint={p.overdue.length ? plural(p.overdue.length, "instalment") : "Nothing missed"}
        />
      </div>

      {data.loans.length === 0 && (
        <Card>
          <EmptyState icon={Landmark} title="No loans yet. Add one, including how many instalments you have already paid." />
        </Card>
      )}

      {p.overdue.length > 0 && <Card title="Not paid yet, past the due date">{dueRows(p.overdue, true)}</Card>}

      {data.loans.length > 0 && (
        <div className="columns">
          <Card title="Due in the next 45 days">
            {p.upcoming.length ? dueRows(p.upcoming, false) : <EmptyState icon={CalendarClock} title="Nothing falls due soon." />}
          </Card>
          <Card
            title="Running loans"
            action={
              running.length || lent.length ? (
                <Button icon={MessageSquareText} onClick={() => setPasting(true)}>
                  From a message
                </Button>
              ) : undefined
            }
          >
            {running.length ? (
              <div className="stack">
                {running.map((l) => (
                  <a key={l.id} className="plain" href={`#/loans/l/${l.id}`}>
                    <Meter label={`${l.name} repaid`} used={l.principal - l.status.outstanding} limit={l.principal} plain />
                    <small className="muted">{nextLine(l)}</small>
                  </a>
                ))}
              </div>
            ) : (
              <EmptyState icon={Landmark} title="No running loans." />
            )}
          </Card>
        </div>
      )}

      {lent.length > 0 && (
        <Card title={`Lent to others: ${money(p.lent_outstanding)} to come back`}>
          <List>
            {lent.map((l) => (
              <ListRow
                key={l.id}
                icon={HandCoins}
                title={l.name}
                detail={l.counterparty}
                value={<Money paise={l.status.outstanding} />}
                href={`#/loans/l/${l.id}`}
              />
            ))}
          </List>
        </Card>
      )}
      {pasting && <BankMessage onClose={() => setPasting(false)} />}
    </>
  );
}
