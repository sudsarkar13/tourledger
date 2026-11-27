import { ArrowLeftRight, FileText, HandCoins, Pencil, Plus, ReceiptText } from "lucide-react";
import { useState } from "react";
import { today } from "../api";
import { CategoryBars, Meter } from "../components/charts";
import { Badge, Button, Card, EmptyState, List, ListRow, Money, PageHeader, Stat } from "../components/ui";
import { ExpenseForm } from "../forms/ExpenseForm";
import { TransferForm } from "../forms/MoneyForms";
import { TourForm } from "../forms/TourForm";
import { dateRange, day, money } from "../format";
import { personName, TOUR_STATUS, TRANSFER_KINDS } from "../labels";
import { useApp } from "../state";
import type { Expense, Tour, Transfer, TourStatus } from "../types";
import { navigate } from "../router";
import { QuickEntry } from "../components/QuickEntry";
import { ExpenseRow } from "./ExpenseRow";

const NEXT: Partial<Record<TourStatus, { to: TourStatus; label: string }>> = {
  planned: { to: "active", label: "Start tour" },
  active: { to: "completed", label: "Mark completed" },
  completed: { to: "submitted", label: "Mark claim sent" },
  submitted: { to: "settled", label: "Mark settled" },
};

export function TourDetail({ id }: { id: string }) {
  const { data, save, notify, fail } = useApp();
  const [editing, setEditing] = useState(false);
  const [expense, setExpense] = useState<Expense | "new" | null>(null);
  const [transfer, setTransfer] = useState<Transfer | "new" | null>(null);
  const tour = data.tours.find((t) => t.id === id);

  if (!tour) {
    return (
      <>
        <PageHeader title="Tour not found" back={{ href: "#/tours", label: "Tours" }} />
        <Card>
          <EmptyState icon={ReceiptText} title="It may have been deleted on another device." />
        </Card>
      </>
    );
  }

  const s = tour.summary;
  const company = tour.kind === "company";
  const expenses = data.expenses.filter((e) => e.tour_id === id);
  const transfers = data.transfers.filter((t) => t.tour_id === id);
  const companyName = data.companies.find((c) => c.id === tour.company_id)?.name;
  const next = NEXT[tour.status];
  // A personal trip has no claim to send.
  const step = next && !company && next.to === "submitted" ? { to: "settled" as TourStatus, label: "Mark settled" } : next;

  const advance = async (to: TourStatus) => {
    try {
      const { id: _id, summary: _summary, claim_state: _state, ...body }: Tour = tour;
      await save("tour", { ...body, status: to, submitted_on: to === "submitted" ? today() : body.submitted_on }, id);
      notify(`Marked ${TOUR_STATUS[to].toLowerCase()}`);
    } catch (error) {
      fail(error);
    }
  };

  return (
    <>
      <PageHeader
        title={tour.name}
        back={{ href: "#/tours", label: "Tours" }}
        context={[dateRange(tour.start_date, tour.end_date), tour.venue, tour.city, companyName].filter(Boolean).join(" · ")}
      >
        <Badge tone={tour.status === "settled" ? "good" : "neutral"}>{TOUR_STATUS[tour.status]}</Badge>
        {tour.local_only && <Badge tone="warn">This device only</Badge>}
        <Button icon={Pencil} onClick={() => setEditing(true)}>
          Edit
        </Button>
        <Button variant="primary" icon={Plus} onClick={() => setExpense("new")}>
          Add expense
        </Button>
      </PageHeader>

      {(tour.event_name || tour.organiser) && (
        <p className="muted">
          {[tour.event_name, tour.organiser && `Organised by ${tour.organiser}`].filter(Boolean).join(" · ")}
        </p>
      )}

      <div className="stats">
        <Stat label="Total tour cost" value={money(s.total_cost)} hint={s.company_paid ? `Company paid ${money(s.company_paid)} directly` : undefined} />
        {company ? (
          <>
            <Stat label="I paid, to claim" value={money(s.claimable)} />
            <Stat label="Advance received" value={money(s.advance)} />
            <Stat
              label={s.net < 0 ? "I return to company" : "Company pays me"}
              value={money(Math.abs(s.net))}
              tone={s.net > 0 ? "good" : s.net < 0 ? "bad" : undefined}
              hint={s.net === 0 && s.claimable ? "Fully settled" : undefined}
            />
          </>
        ) : (
          <>
            <Stat label="I paid" value={money(s.paid_by_me)} />
            <Stat label="My share" value={money(s.my_cost)} />
          </>
        )}
      </div>

      {tour.budget > 0 && (
        <Card title="Budget">
          <Meter label="Spent on this tour" used={s.total_cost} limit={tour.budget} />
        </Card>
      )}

      <div className="actions">
        {(company || data.people.length > 0) && (
          <Button icon={HandCoins} onClick={() => setTransfer("new")}>
            {company ? "Record advance or payment" : "Record a payment"}
          </Button>
        )}
        <Button icon={FileText} onClick={() => navigate(`/tours/${id}/report`)}>
          {company ? "Claim report" : "Expense report"}
        </Button>
        {step && (
          <Button variant="ghost" onClick={() => advance(step.to)}>
            {step.label}
          </Button>
        )}
      </div>

      <QuickEntry key={id} tourId={id} />

      <div className="columns">
        <Card title="Expenses">
          {expenses.length ? (
            <List>
              {expenses.map((e) => (
                <ExpenseRow key={e.id} expense={e} onClick={() => setExpense(e)} />
              ))}
            </List>
          ) : (
            <EmptyState icon={ReceiptText} title="No expenses yet. Start with the ride from home to the station or airport." />
          )}
        </Card>

        <div className="stack">
          {s.total_cost > 0 && (
            <Card title="Where it went">
              <CategoryBars values={s.by_category} />
            </Card>
          )}
          {Object.keys(s.balances).length > 0 && (
            <Card title="Between people">
              <List>
                {Object.entries(s.balances).map(([person, value]) => (
                  <ListRow
                    key={person}
                    title={personName(data, person)}
                    detail={value > 0 ? "owes me" : "I owe them"}
                    value={<Money paise={Math.abs(value)} tone={value > 0 ? "good" : "bad"} />}
                  />
                ))}
              </List>
            </Card>
          )}
          {transfers.length > 0 && (
            <Card title="Advances and payments">
              <List>
                {transfers.map((t) => (
                  <ListRow
                    key={t.id}
                    icon={ArrowLeftRight}
                    title={TRANSFER_KINDS[t.kind]}
                    detail={[day(t.date), t.person_id && personName(data, t.person_id), t.note].filter(Boolean).join(" · ")}
                    value={<Money paise={t.amount} />}
                    onClick={() => setTransfer(t)}
                  />
                ))}
              </List>
            </Card>
          )}
          {tour.notes && (
            <Card title="Notes">
              <p>{tour.notes}</p>
            </Card>
          )}
        </div>
      </div>

      {editing && <TourForm tour={tour} onClose={() => setEditing(false)} />}
      {expense && <ExpenseForm expense={expense === "new" ? undefined : expense} tourId={id} onClose={() => setExpense(null)} />}
      {transfer && (
        <TransferForm
          transfer={transfer === "new" ? undefined : transfer}
          tourId={id}
          preset={transfer === "new" && !company ? { kind: "received_from" } : undefined}
          onClose={() => setTransfer(null)}
        />
      )}
    </>
  );
}
