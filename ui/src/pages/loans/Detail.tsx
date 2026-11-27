import { Check, HandCoins, Landmark, Pencil, RotateCcw, XCircle } from "lucide-react";
import { useState } from "react";
import { api } from "../../api";
import { Badge, Button, Card, EmptyState, List, ListRow, Money, PageHeader, Stat } from "../../components/ui";
import { CloseLoanForm, LoanForm, PaymentForm } from "../../forms/MoneyForms";
import { fullDay, money } from "../../format";
import { loanCategory } from "../../labels";
import { useApp } from "../../state";
import type { Payment } from "../../types";
import { Instalments, Legend } from "./Tracker";

export function LoanDetail({ id }: { id: string }) {
  const { data, reload, notify, fail, confirm } = useApp();
  const [editing, setEditing] = useState(false);
  const [closing, setClosing] = useState(false);
  const [payment, setPayment] = useState<Payment | "new" | null>(null);
  const loan = data.loans.find((l) => l.id === id);
  const back = { href: "#/loans/list", label: "My loans" };

  if (!loan) {
    return (
      <>
        <PageHeader title="Loan not found" back={back} />
        <Card>
          <EmptyState icon={Landmark} title="It may have been deleted on another device." />
        </Card>
      </>
    );
  }

  const s = loan.status;
  const emi = loan.style === "emi";
  const payments = data.payments.filter((p) => p.loan_id === id);

  const reopen = async () => {
    if (!(await confirm(`Reopen ${loan.name}?`, "Any closing payment recorded when it was closed is removed.", "Reopen"))) return;
    try {
      await api({ op: "loan_reopen", loan_id: id });
      await reload();
      notify(`${loan.name} reopened`);
    } catch (error) {
      fail(error);
    }
  };

  return (
    <>
      <PageHeader
        title={loan.name}
        back={back}
        context={[
          loan.direction === "borrowed" ? `${loanCategory(loan.category)} loan` : "Money I lent",
          loan.counterparty,
          loan.reference && `Ref ${loan.reference}`,
          `${loan.direction === "borrowed" ? "taken" : "given"} ${fullDay(loan.start_date)}`,
          emi && `${loan.rate_bps / 100}% a year, ${loan.tenure_months} months`,
        ]
          .filter(Boolean)
          .join(" · ")}
      >
        {loan.closed && <Badge tone="good">Closed{loan.closed_on && ` ${fullDay(loan.closed_on)}`}</Badge>}
        {loan.local_only && <Badge tone="warn">This device only</Badge>}
        <Button icon={Pencil} onClick={() => setEditing(true)}>
          Edit
        </Button>
        {loan.closed ? (
          <Button icon={RotateCcw} onClick={reopen}>
            Reopen
          </Button>
        ) : (
          <>
            <Button icon={XCircle} onClick={() => setClosing(true)}>
              Close loan
            </Button>
            <Button variant="primary" icon={Check} onClick={() => setPayment("new")}>
              Record payment
            </Button>
          </>
        )}
      </PageHeader>

      <div className="stats">
        <Stat label={loan.direction === "borrowed" ? "Borrowed" : "Lent"} value={money(loan.principal)} />
        <Stat label="Paid so far" value={money(s.paid_all)} hint={emi ? (loan.closed ? `${s.paid_count} instalments, then closed` : `${s.paid_count} of ${s.total_count} instalments`) : undefined} />
        <Stat label="Left to repay" value={money(s.outstanding)} hint={emi && s.ends_on && !loan.closed ? `Ends ${fullDay(s.ends_on)}` : undefined} />
        {emi && <Stat label="Monthly instalment" value={money(s.emi)} />}
        {emi && (
          <Stat
            label="Interest paid"
            value={money(s.interest_paid)}
            hint={`${money(s.interest_remaining)} still to pay. Estimates.`}
          />
        )}
      </div>

      {emi && (
        <Card title="Month by month">
          <Legend />
          <Instalments loan={loan} />
          <details>
            <summary>Show the full schedule</summary>
            <div className="table-scroll">
              <table>
                <thead>
                  <tr>
                    <th>#</th>
                    <th>Due</th>
                    <th className="num">Instalment</th>
                    <th className="num">Interest</th>
                    <th className="num">Balance</th>
                    <th>Paid on</th>
                  </tr>
                </thead>
                <tbody>
                  {s.schedule.map((r) => (
                    <tr key={r.number} className={r.paid ? "paid" : ""}>
                      <td>{r.number}</td>
                      <td>{fullDay(r.due)}</td>
                      <td className="num">{money(r.amount)}</td>
                      <td className="num">{money(r.interest)}</td>
                      <td className="num">{money(r.balance)}</td>
                      <td>{r.paid ? (r.paid_on ? fullDay(r.paid_on) : "Earlier") : ""}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </details>
        </Card>
      )}

      <Card title="Payments recorded">
        {payments.length ? (
          <List>
            {payments.map((p) => (
              <ListRow
                key={p.id}
                icon={Check}
                title={p.kind === "instalment" ? `Instalment ${p.number}` : p.kind === "prepayment" ? "Part prepayment" : "Repayment"}
                detail={[fullDay(p.date), p.note].filter(Boolean).join(" · ")}
                value={<Money paise={p.amount} />}
                onClick={() => setPayment(p)}
              />
            ))}
          </List>
        ) : (
          <EmptyState icon={HandCoins} title="No payments recorded here yet." />
        )}
      </Card>
      {loan.notes && (
        <Card title="Notes">
          <p>{loan.notes}</p>
        </Card>
      )}

      {editing && <LoanForm loan={loan} onClose={() => setEditing(false)} />}
      {closing && <CloseLoanForm loan={loan} onClose={() => setClosing(false)} />}
      {payment && <PaymentForm loan={loan} payment={payment === "new" ? undefined : payment} onClose={() => setPayment(null)} />}
    </>
  );
}
