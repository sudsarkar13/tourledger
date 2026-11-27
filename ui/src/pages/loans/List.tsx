// Every loan, running or closed, grouped by what it is for.
import { HandCoins, Landmark } from "lucide-react";
import { useState } from "react";
import { Badge, Card, EmptyState, List, ListRow, Money, Segmented } from "../../components/ui";
import { money } from "../../format";
import { loanCategory } from "../../labels";
import { useApp } from "../../state";
import type { Loan } from "../../types";
import { isRunning, LoansHeader, nextLine } from "./common";

export function LoanList() {
  const { data } = useApp();
  const [show, setShow] = useState<"running" | "closed" | "all">("running");
  const loans = data.loans.filter((l) => (show === "all" ? true : show === "running" ? isRunning(l) : !isRunning(l)));

  const groups = new Map<string, Loan[]>();
  for (const l of loans) {
    const heading = l.direction === "lent" ? "Money I lent" : `${loanCategory(l.category)} loans`;
    groups.set(heading, [...(groups.get(heading) ?? []), l]);
  }
  const counts = {
    running: data.loans.filter(isRunning).length,
    closed: data.loans.filter((l) => !isRunning(l)).length,
  };

  return (
    <>
      <LoansHeader tab="list" context="Running and closed loans, by kind" />
      <Segmented
        label="Show"
        value={show}
        onChange={setShow}
        options={[
          { value: "running", label: `Running (${counts.running})` },
          { value: "closed", label: `Closed (${counts.closed})` },
          { value: "all", label: "All" },
        ]}
      />
      {loans.length === 0 && (
        <Card>
          <EmptyState
            icon={Landmark}
            title={data.loans.length ? "No loans in this view." : "No loans yet. Use Add loan to enter one."}
          />
        </Card>
      )}
      {[...groups].map(([heading, group]) => {
        const left = group.filter(isRunning).reduce((sum, l) => sum + l.status.outstanding, 0);
        return (
          <Card key={heading} title={left ? `${heading}: ${money(left)} left` : heading}>
            <List>
              {group.map((l) => (
                <ListRow
                  key={l.id}
                  icon={l.style === "emi" ? Landmark : HandCoins}
                  title={l.name}
                  detail={[
                    l.counterparty,
                    l.style === "emi" && `${l.status.paid_count} of ${l.status.total_count} paid`,
                    nextLine(l),
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                  value={<Money paise={isRunning(l) ? l.status.outstanding : l.principal} />}
                  note={
                    !isRunning(l) ? (
                      <Badge tone="good">Closed</Badge>
                    ) : l.status.overdue_count ? (
                      <Badge tone="bad">{l.status.overdue_count} overdue</Badge>
                    ) : (
                      "left"
                    )
                  }
                  href={`#/loans/l/${l.id}`}
                />
              ))}
            </List>
          </Card>
        );
      })}
    </>
  );
}
