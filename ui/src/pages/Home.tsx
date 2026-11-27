// Tours only. No loan figure ever appears here (docs/DESIGN.md).
import { AlertTriangle, Bell, CircleAlert, FileText, HandCoins, Luggage, Plus, ReceiptText } from "lucide-react";
import { useState } from "react";
import { ClaimBadge, CompanySwitcher, useCompanyFilter } from "../components/shared";
import { QuickEntry } from "../components/QuickEntry";
import { Button, Card, EmptyState, List, ListRow, Money, PageHeader, Select, Stat } from "../components/ui";
import { ExpenseForm } from "../forms/ExpenseForm";
import { TransferForm } from "../forms/MoneyForms";
import { TourForm } from "../forms/TourForm";
import { dateRange, money } from "../format";
import { alertText } from "../labels";
import { navigate } from "../router";
import { useApp } from "../state";
import type { Expense, Tour } from "../types";
import { today } from "../api";
import { ExpenseRow } from "./ExpenseRow";

/** The tour being worked on: today's, else the latest one with money open. */
function pickTour(tours: Tour[]): Tour | undefined {
  const now = today();
  return (
    tours.find((t) => t.start_date <= now && now <= t.end_date && t.status !== "settled") ??
    tours.find((t) => t.status !== "settled" && t.claim_state !== "paid") ??
    tours[0]
  );
}

export function Home() {
  const { data } = useApp();
  const filter = useCompanyFilter();
  const [picked, setPicked] = useState("");
  const [expense, setExpense] = useState<Expense | "new" | null>(null);
  const [cash, setCash] = useState(false);
  const [newTour, setNewTour] = useState(false);

  const tour = filter.tours.find((t) => t.id === picked) ?? pickTour(filter.tours);
  const company = tour?.kind === "company";
  const s = tour?.summary;
  const ids = new Set(filter.tours.map((t) => t.id));
  const recent = data.expenses.filter((e) => ids.has(e.tour_id)).slice(0, 8);
  const waiting = filter.tours.filter((t) => t.kind === "company" && t.status !== "settled" && t.summary.net !== 0);
  const toGet = waiting.reduce((sum, t) => sum + Math.max(t.summary.net, 0), 0);
  const toReturn = waiting.reduce((sum, t) => sum + Math.max(-t.summary.net, 0), 0);
  const reminders = data.insights.alerts.filter(
    (a) => ["claim_not_sent", "claim_waiting", "advance_unreturned", "tour_soon", "orphans"].includes(a.kind) && (!a.ref_id || ids.has(a.ref_id)),
  );
  const first = data.profile.name.split(" ")[0];

  return (
    <>
      <PageHeader title={first ? `Hello, ${first}` : "Home"} context="Your tour expenses and what is still to come back">
        <Button variant="primary" icon={Plus} onClick={() => setNewTour(true)}>
          New tour
        </Button>
      </PageHeader>
      <CompanySwitcher />

      {tour && s ? (
        <Card
          title="Current tour"
          action={
            filter.tours.length > 1 ? (
              <Select
                label="Tour"
                value={tour.id}
                onChange={setPicked}
                options={filter.tours.map((t) => ({ value: t.id, label: t.name }))}
              />
            ) : undefined
          }
        >
          <List>
            <ListRow
              icon={Luggage}
              title={tour.name}
              detail={[dateRange(tour.start_date, tour.end_date), tour.city].filter(Boolean).join(" · ")}
              note={<ClaimBadge state={tour.claim_state} />}
              href={`#/tours/${tour.id}`}
            />
          </List>
          <div className="stats">
            {company ? (
              <>
                <Stat label="Cash the company gave" value={money(s.advance - s.advance_returned)} />
                <Stat label="I spent, to claim" value={money(s.claimable)} />
                <Stat
                  label={s.net < 0 ? "I must return" : "Company still pays me"}
                  value={money(Math.abs(s.net))}
                  tone={s.net > 0 ? "good" : s.net < 0 ? "bad" : undefined}
                />
              </>
            ) : (
              <>
                <Stat label="Total cost" value={money(s.total_cost)} />
                <Stat label="I paid" value={money(s.paid_by_me)} />
                <Stat label="My share" value={money(s.my_cost)} />
              </>
            )}
          </div>
          {s.allowance > 0 && (
            <p>
              Bonus or allowance from the company: <Money paise={s.allowance} tone="good" />
            </p>
          )}
          <div className="actions">
            <Button variant="primary" icon={ReceiptText} onClick={() => setExpense("new")}>
              Add expense
            </Button>
            {company && (
              <Button icon={HandCoins} onClick={() => setCash(true)}>
                Cash from company
              </Button>
            )}
            <Button icon={FileText} onClick={() => navigate(`/tours/${tour.id}/report`)}>
              {company ? "Claim report" : "Expense report"}
            </Button>
          </div>
        </Card>
      ) : (
        <Card title="Current tour">
          <EmptyState
            icon={Luggage}
            title={
              data.companies.length
                ? "No tour here yet. Use New tour above before you travel."
                : "Start under More: add your details, your company and colleagues. Then create a tour."
            }
          >
            {!data.companies.length && (
              <a className="button secondary" href="#/more">
                Open More
              </a>
            )}
          </EmptyState>
        </Card>
      )}

      {tour && <QuickEntry key={tour.id} tourId={tour.id} />}

      {tour && (
      <div className="columns">
        <Card title="Recent tour expenses">
          {recent.length ? (
            <List>
              {recent.map((e) => (
                <ExpenseRow key={e.id} expense={e} showTour onClick={() => setExpense(e)} />
              ))}
            </List>
          ) : (
            <EmptyState icon={ReceiptText} title="Nothing yet. Type a line above, or use Add expense." />
          )}
        </Card>
        <div className="stack">
          <Card title="Money to settle">
            {waiting.length ? (
              <>
                <p>
                  {toGet > 0 && (
                    <>
                      To receive <Money paise={toGet} tone="good" />
                    </>
                  )}
                  {toGet > 0 && toReturn > 0 && " · "}
                  {toReturn > 0 && (
                    <>
                      To return <Money paise={toReturn} tone="bad" />
                    </>
                  )}
                </p>
                <List>
                  {waiting.map((t) => (
                    <ListRow
                      key={t.id}
                      title={t.name}
                      detail={data.companies.find((c) => c.id === t.company_id)?.name}
                      value={<Money paise={Math.abs(t.summary.net)} />}
                      note={<ClaimBadge state={t.claim_state} />}
                      href={`#/tours/${t.id}`}
                    />
                  ))}
                </List>
              </>
            ) : (
              <EmptyState icon={HandCoins} title="Nothing is waiting to be paid." />
            )}
          </Card>
          {reminders.length > 0 && (
            <Card title="Reminders">
              <List>
                {reminders.map((a, i) => (
                  <ListRow
                    key={i}
                    icon={a.level === "danger" ? CircleAlert : a.level === "warn" ? AlertTriangle : Bell}
                    title={alertText(a).title}
                    detail={alertText(a).detail}
                    href={a.ref_id ? `#/tours/${a.ref_id}` : "#/reports"}
                  />
                ))}
              </List>
            </Card>
          )}
        </div>
      </div>
      )}

      {expense && (
        <ExpenseForm expense={expense === "new" ? undefined : expense} tourId={tour?.id} onClose={() => setExpense(null)} />
      )}
      {cash && tour && <TransferForm tourId={tour.id} onClose={() => setCash(false)} />}
      {newTour && <TourForm onClose={() => setNewTour(false)} />}
    </>
  );
}
