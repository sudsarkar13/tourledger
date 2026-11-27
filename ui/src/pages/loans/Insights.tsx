import { Landmark, TrendingDown } from "lucide-react";
import { CategoryBars, MonthBars } from "../../components/charts";
import { Card, EmptyState, List, ListRow, Money, Stat } from "../../components/ui";
import { fullDay, money } from "../../format";
import { loanCategory } from "../../labels";
import { useApp } from "../../state";
import { isRunning, LoansHeader } from "./common";

export function LoanInsights() {
  const { data } = useApp();
  const p = data.portfolio;
  const borrowed = data.loans.filter((l) => l.direction === "borrowed");
  const now = p.months[12]?.month;
  const past = p.months.filter((m) => m.month <= now).map((m) => ({ month: m.month, value: m.paid }));
  const ahead = p.months.filter((m) => m.month >= now).map((m) => ({ month: m.month, value: m.due }));

  return (
    <>
      <LoansHeader tab="insights" context="What borrowing costs you and when it ends" />
      {borrowed.length === 0 ? (
        <Card>
          <EmptyState icon={TrendingDown} title="Add a loan to see what it costs and when it ends." />
        </Card>
      ) : (
        <>
          <div className="stats">
            <Stat label="Borrowed in all" value={money(p.borrowed_total)} hint={`${p.running} running, ${p.closed} closed`} />
            <Stat label="Paid back so far" value={money(p.paid_total)} />
            <Stat label="Interest paid" value={money(p.interest_paid)} hint={`${money(p.interest_remaining)} still to pay`} />
            <Stat label="Free of EMIs" value={p.debt_free_on ? fullDay(p.debt_free_on) : "Now"} hint="If paid as scheduled" />
            <Stat
              label="EMIs from income"
              value={p.emi_share_percent !== null ? `${p.emi_share_percent}%` : "Unknown"}
              hint={p.left_after_emi !== null ? `${money(p.left_after_emi)} left a month before other spending` : "Add monthly income in your profile"}
            />
          </div>
          {p.highest_rate && (
            <Card title="Highest interest rate">
              <List>
                <ListRow
                  icon={Landmark}
                  title={p.highest_rate[1]}
                  detail={`${p.highest_rate[2] / 100}% a year, the most among your running loans`}
                  href={`#/loans/l/${p.highest_rate[0]}`}
                />
              </List>
            </Card>
          )}
          <div className="columns">
            <Card title="Paid in the last year">
              <MonthBars months={past} />
            </Card>
            <Card title="Still due in the year ahead">
              <MonthBars months={ahead} start="first" />
            </Card>
          </div>
          {p.by_category.length > 0 && (
            <Card title="Still owed, by kind">
              <CategoryBars values={Object.fromEntries(p.by_category.map((c) => [c.category, c.outstanding]))} name={loanCategory} />
            </Card>
          )}
          <Card title="Interest, loan by loan">
            <List>
              {borrowed
                .filter((l) => l.style === "emi")
                .map((l) => (
                  <ListRow
                    key={l.id}
                    title={l.name}
                    detail={`${l.rate_bps / 100}% a year · ${isRunning(l) ? `${money(l.status.interest_remaining)} still to pay` : "closed"}`}
                    value={<Money paise={l.status.interest_paid} />}
                    note="interest paid"
                    href={`#/loans/l/${l.id}`}
                  />
                ))}
            </List>
            <p className="muted">Calculated on a monthly reducing balance. Your lender's statement is the authority.</p>
          </Card>
        </>
      )}
    </>
  );
}
