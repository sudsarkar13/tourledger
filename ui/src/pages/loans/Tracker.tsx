// Every instalment of every loan on one page, from its first due date, each one
// tappable to record it as paid or not.
import { Check, Landmark } from "lucide-react";
import { useState } from "react";
import { today } from "../../api";
import { Button, Card, EmptyState, Toggle } from "../../components/ui";
import { day, money, monthName } from "../../format";
import { useApp } from "../../state";
import type { Instalment, Loan } from "../../types";
import { isRunning, LoansHeader, useInstalments } from "./common";

type State = "paid" | "overdue" | "next" | "later" | "closed";
const STATES: Record<State, { mark: string; label: string }> = {
  paid: { mark: "✓", label: "Paid" },
  overdue: { mark: "!", label: "Overdue" },
  next: { mark: "○", label: "Due this month" },
  later: { mark: "·", label: "Later" },
  closed: { mark: "–", label: "Closed before it fell due" },
};

function stateOf(loan: Loan, row: Instalment, now: string): State {
  if (row.paid) return "paid";
  if (loan.closed) return "closed";
  if (row.due < now) return "overdue";
  return row.due.slice(0, 7) === now.slice(0, 7) ? "next" : "later";
}

export function Instalments({ loan }: { loan: Loan }) {
  const { busy, mark } = useInstalments();
  const now = today();
  const years = new Map<string, Instalment[]>();
  for (const row of loan.status.schedule) {
    const year = row.due.slice(0, 4);
    years.set(year, [...(years.get(year) ?? []), row]);
  }
  return (
    <div className="years">
      {[...years].map(([year, rows]) => (
        <div key={year} className="year">
          <h3>{year}</h3>
          <div className="cells">
            {rows.map((row) => {
              const state = stateOf(loan, row, now);
              return (
                <button
                  key={row.number}
                  type="button"
                  className={`cell ${state}`}
                  disabled={busy || (loan.closed && !row.paid)}
                  aria-pressed={row.paid}
                  aria-label={`Instalment ${row.number}, due ${day(row.due)} ${year}, ${money(row.amount)}, ${STATES[state].label}`}
                  title={`${money(row.amount)} due ${day(row.due)}. ${STATES[state].label}.`}
                  onClick={() => mark(loan.id, row.number, !row.paid)}
                >
                  <span>{monthName(row.due.slice(0, 7))}</span>
                  <strong aria-hidden>{STATES[state].mark}</strong>
                </button>
              );
            })}
          </div>
        </div>
      ))}
    </div>
  );
}

export function Legend() {
  return (
    <ul className="legend">
      {(Object.keys(STATES) as State[]).map((s) => (
        <li key={s}>
          <span className={`cell ${s}`} aria-hidden>
            {STATES[s].mark}
          </span>
          {STATES[s].label}
        </li>
      ))}
    </ul>
  );
}

export function Tracker() {
  const { data } = useApp();
  const { busy, markDue } = useInstalments();
  const [closed, setClosed] = useState(false);
  const now = today();
  const loans = data.loans.filter((l) => l.style === "emi" && l.direction === "borrowed" && (closed || isRunning(l)));
  const anyEmi = data.loans.some((l) => l.style === "emi" && l.direction === "borrowed");

  return (
    <>
      <LoansHeader tab="tracker" context="Tap a month to record it as paid or not paid" />
      {anyEmi && (
        <Card>
          <Legend />
          <Toggle label="Show closed loans too" checked={closed} onChange={setClosed} />
        </Card>
      )}
      {loans.length === 0 && (
        <Card>
          <EmptyState
            icon={Landmark}
            title={anyEmi ? "No running EMI loans. Turn on closed loans to see the rest." : "Add a loan with monthly instalments and each month appears here."}
          />
        </Card>
      )}
      {loans.map((loan) => {
        const s = loan.status;
        const dueNow = loan.closed ? 0 : s.schedule.filter((r) => !r.paid && r.due <= now).length;
        return (
          <Card
            key={loan.id}
            title={loan.name}
            action={
              dueNow > 0 ? (
                <Button icon={Check} disabled={busy} onClick={() => markDue(loan)}>
                  Mark all due as paid ({dueNow})
                </Button>
              ) : undefined
            }
          >
            <p className="muted">
              {[
                `${money(s.emi)} a month`,
                `${s.paid_count} of ${s.total_count} paid`,
                s.overdue_count ? `${s.overdue_count} overdue` : "",
                loan.closed ? "closed" : `${money(s.outstanding)} left`,
              ]
                .filter(Boolean)
                .join(" · ")}{" "}
              · <a href={`#/loans/l/${loan.id}`}>Open</a>
            </p>
            <Instalments loan={loan} />
          </Card>
        );
      })}
    </>
  );
}
