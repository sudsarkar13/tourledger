// Claims and spending for tours and daily costs. No loan figures here.
import { FileSpreadsheet, PiggyBank, Plus, ReceiptText, TrendingUp } from "lucide-react";
import { useState } from "react";
import { api, saveFile, today } from "../api";
import { CategoryBars, Meter, MonthBars } from "../components/charts";
import { ClaimBadge, CompanySwitcher, useCompanyFilter } from "../components/shared";
import { Button, Card, EmptyState, List, ListRow, Money, PageHeader, Segmented, Select, Stat } from "../components/ui";
import { ExpenseForm } from "../forms/ExpenseForm";
import { BudgetForm } from "../forms/MoneyForms";
import { dateRange, money, monthName, plural } from "../format";
import { alertText, categoryName } from "../labels";
import { useApp } from "../state";
import type { ExportedFile, Expense } from "../types";
import { ExpenseRow } from "./ExpenseRow";

function Claims() {
  const { data } = useApp();
  const { tours, value } = useCompanyFilter();
  const personal = value === "personal";
  const company = tours.filter((t) => t.kind === "company");
  const sum = (pick: (t: (typeof company)[number]) => number) => company.reduce((total, t) => total + pick(t), 0);
  const open = company.filter((t) => t.status !== "settled");
  const pending = open.reduce((total, t) => total + Math.max(t.summary.net, 0), 0);

  const byCategory: Record<string, number> = {};
  for (const t of tours) {
    for (const [category, value] of Object.entries(t.summary.by_category)) {
      byCategory[category] = (byCategory[category] ?? 0) + value;
    }
  }

  return (
    <>
      <CompanySwitcher />
      {personal ? (
        <div className="stats">
          <Stat label="Personal trips cost" value={money(tours.reduce((total, t) => total + t.summary.total_cost, 0))} />
          <Stat label="My share" value={money(tours.reduce((total, t) => total + t.summary.my_cost, 0))} />
        </div>
      ) : (
      <div className="stats">
        <Stat label="Claimed in all" value={money(sum((t) => t.summary.claimable))} />
        <Stat label="Cash given in advance" value={money(sum((t) => t.summary.advance - t.summary.advance_returned))} />
        <Stat label="Reimbursed so far" value={money(sum((t) => t.summary.reimbursed))} />
        <Stat label="Still to receive" value={money(pending)} tone={pending ? "good" : undefined} />
      </div>
      )}
      <Card title={personal ? "Report by trip" : "Claim by tour"}>
        {tours.length ? (
          <List>
            {tours.map((t) => (
              <ListRow
                key={t.id}
                title={t.name}
                detail={[
                  data.companies.find((c) => c.id === t.company_id)?.name,
                  dateRange(t.start_date, t.end_date),
                  personal ? `cost ${money(t.summary.total_cost)}` : `claimed ${money(t.summary.claimable)}`,
                ]
                  .filter(Boolean)
                  .join(" · ")}
                value={personal || t.status === "settled" ? undefined : <Money paise={Math.abs(t.summary.net)} />}
                note={<ClaimBadge state={t.claim_state} />}
                href={`#/tours/${t.id}/report`}
              />
            ))}
          </List>
        ) : (
          <EmptyState icon={ReceiptText} title="No tours here yet." />
        )}
      </Card>
      <Card title="Tour costs by category">
        {Object.keys(byCategory).length ? (
          <CategoryBars values={byCategory} />
        ) : (
          <EmptyState icon={TrendingUp} title="Add tour expenses and the split appears here." />
        )}
      </Card>
    </>
  );
}

function Spending() {
  const { data, notify, fail } = useApp();
  const insights = data.insights;
  const [month, setMonth] = useState(insights.months.at(-1)!.month);
  const [budget, setBudget] = useState<{ category?: string; monthly?: number } | null>(null);
  const [expense, setExpense] = useState<Expense | "new" | null>(null);
  const chosen = insights.months.find((m) => m.month === month) ?? insights.months.at(-1)!;
  const next = insights.next_month;
  const daily = data.expenses.filter((e) => !e.tour_id);
  const notes = insights.alerts.filter((a) => ["budget_over", "budget_near", "category_up", "last_year"].includes(a.kind));

  const exportAll = async () => {
    try {
      const file = await api<ExportedFile>({ op: "export_expenses", today: today() });
      if (await saveFile(file)) notify(`Saved ${file.name}`);
    } catch (error) {
      fail(error);
    }
  };

  return (
    <>
      <p className="muted">What finally came out of your own pocket. Claimed and company-paid costs are left out.</p>
      <div className="stats">
        <Stat label="This month" value={money(insights.months.at(-1)!.spend)} />
        <Stat
          label="Next month, estimated"
          value={next ? money(next.spend) : "Not enough data"}
          hint={
            next
              ? `Weighted average of ${plural(Math.min(insights.history_months, 3), "recent month")}`
              : "Needs two earlier months of entries"
          }
        />
        <Stat
          label="Income left after spending"
          value={insights.savings_estimate !== null ? money(insights.savings_estimate) : "Unknown"}
          hint={insights.income ? "Next month, before loan instalments" : "Add monthly income in your profile"}
        />
      </div>
      {notes.length > 0 && (
        <Card title="Worth knowing">
          <List>
            {notes.map((a, i) => (
              <ListRow key={i} title={alertText(a).title} detail={alertText(a).detail} />
            ))}
          </List>
        </Card>
      )}
      <Card title="Month by month">
        {insights.months.some((m) => m.spend > 0) ? (
          <MonthBars months={insights.months.map((m) => ({ month: m.month, value: m.spend }))} />
        ) : (
          <EmptyState icon={TrendingUp} title="Your own spending appears here." />
        )}
      </Card>
      <div className="columns">
        <Card
          title="By category"
          action={
            <Select
              label="Month"
              value={month}
              onChange={setMonth}
              options={[...insights.months].reverse().map((m) => ({ value: m.month, label: monthName(m.month, true) }))}
            />
          }
        >
          {chosen.spend > 0 ? (
            <CategoryBars values={chosen.by_category} />
          ) : (
            <EmptyState icon={TrendingUp} title="No spending of your own in this month." />
          )}
        </Card>
        <Card
          title="Budgets this month"
          action={
            <Button icon={Plus} onClick={() => setBudget({})}>
              Set budget
            </Button>
          }
        >
          {insights.budgets.length ? (
            <div className="stack">
              {insights.budgets.map((b) => (
                <button key={b.category} type="button" className="plain" onClick={() => setBudget({ category: b.category, monthly: b.budget })}>
                  <Meter label={categoryName(b.category)} used={b.spent} limit={b.budget} />
                </button>
              ))}
            </div>
          ) : (
            <EmptyState icon={PiggyBank} title="Set a monthly limit to be warned before you cross it." />
          )}
        </Card>
      </div>
      <Card
        title="Daily costs outside tours"
        action={
          <Button icon={Plus} onClick={() => setExpense("new")}>
            Add daily cost
          </Button>
        }
      >
        {daily.length ? (
          <List>
            {daily.slice(0, 15).map((e) => (
              <ExpenseRow key={e.id} expense={e} onClick={() => setExpense(e)} />
            ))}
          </List>
        ) : (
          <EmptyState icon={ReceiptText} title="Costs that belong to no tour are listed here." />
        )}
      </Card>
      <div className="actions">
        <Button icon={FileSpreadsheet} onClick={exportAll}>
          Export every expense
        </Button>
      </div>
      {budget && <BudgetForm {...budget} onClose={() => setBudget(null)} />}
      {expense && <ExpenseForm expense={expense === "new" ? undefined : expense} onClose={() => setExpense(null)} />}
    </>
  );
}

export function Reports() {
  const [view, setView] = useState<"claims" | "spending">("claims");
  return (
    <>
      <PageHeader title="Reports" context="Claims to be paid, and where your own money goes" />
      <Segmented
        label="Show"
        value={view}
        onChange={setView}
        options={[
          { value: "claims", label: "Company claims" },
          { value: "spending", label: "My spending" },
        ]}
      />
      {view === "claims" ? <Claims /> : <Spending />}
    </>
  );
}
