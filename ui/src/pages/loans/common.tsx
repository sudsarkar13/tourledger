// What every page in the Loans area shares.
import { Plus } from "lucide-react";
import { useState } from "react";
import { api, today } from "../../api";
import { Button, PageHeader, Tabs } from "../../components/ui";
import { LoanForm } from "../../forms/MoneyForms";
import { fullDay, money } from "../../format";
import { useApp } from "../../state";
import type { Loan } from "../../types";

export function LoansHeader({ tab, context }: { tab: "dashboard" | "list" | "tracker" | "insights"; context: string }) {
  const [adding, setAdding] = useState(false);
  return (
    <>
      <PageHeader title="Loans" context={context}>
        <Button variant="primary" icon={Plus} onClick={() => setAdding(true)}>
          Add loan
        </Button>
      </PageHeader>
      <Tabs
        label="Loan pages"
        current={tab}
        items={[
          { id: "dashboard", label: "Dashboard", href: "#/loans" },
          { id: "list", label: "My loans", href: "#/loans/list" },
          { id: "tracker", label: "EMI tracker", href: "#/loans/tracker" },
          { id: "insights", label: "Insights", href: "#/loans/insights" },
        ]}
      />
      {adding && <LoanForm onClose={() => setAdding(false)} />}
    </>
  );
}

export const isRunning = (l: Loan) => !l.closed && l.status.outstanding > 0;

export function nextLine(l: Loan): string {
  if (l.closed) return l.closed_on ? `Closed on ${fullDay(l.closed_on)}` : "Closed";
  if (l.status.outstanding === 0) return "Fully repaid";
  if (l.status.next) {
    const late = l.status.next.due < today();
    return `${late ? "Overdue" : "Next"}: ${money(l.status.next.amount)} on ${fullDay(l.status.next.due)}`;
  }
  return "Repaid in any amounts";
}

/** Ticking and unticking instalments, used by the dashboard, tracker and loan page. */
export function useInstalments() {
  const { reload, notify, fail, confirm } = useApp();
  const [busy, setBusy] = useState(false);

  const run = async (request: Record<string, unknown>, done: string) => {
    setBusy(true);
    try {
      await api({ ...request, today: today() });
      await reload();
      notify(done);
    } catch (error) {
      fail(error);
    } finally {
      setBusy(false);
    }
  };

  const mark = async (loanId: string, number: number, paid: boolean) => {
    if (!paid) {
      const yes = await confirm(
        `Mark instalment ${number} as not paid?`,
        "Its payment record is removed.",
        "Mark not paid",
      );
      if (!yes) return;
    }
    await run(
      { op: "loan_mark", loan_id: loanId, number, paid },
      `Instalment ${number} marked ${paid ? "paid" : "not paid"}`,
    );
  };

  const markDue = async (loan: Loan) => {
    const due = loan.status.schedule.filter((r) => !r.paid && r.due <= today());
    const yes = await confirm(
      `Mark ${due.length} as paid?`,
      `Every instalment of ${loan.name} due up to today is recorded as paid on its due date.`,
      "Mark paid",
    );
    if (yes) await run({ op: "loan_mark_due", loan_id: loan.id }, `${due.length} marked paid`);
  };

  return { busy, mark, markDue };
}
