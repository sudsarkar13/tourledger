// How my payments on a tour were made, and which still have no picture of a bill
// or payment screen. The counting is done in crates/core/src/proof.rs.
import { useState } from "react";
import { money } from "../format";
import { ExpenseRow } from "../pages/ExpenseRow";
import type { Expense, Proof } from "../types";
import { Button, Card, List, ListRow, Money, Stat } from "./ui";

const payments = (n: number) => `${n} ${n === 1 ? "payment" : "payments"}`;

export function ProofCard({
  proof,
  claim,
  expenses,
  onOpen,
}: {
  proof: Proof;
  /** A company tour: only what is claimed is counted. */
  claim: boolean;
  expenses: Expense[];
  onOpen: (expense: Expense) => void;
}) {
  const [listing, setListing] = useState(false);
  const without = proof.all.count - proof.pictured.count;
  const missing = new Set(proof.missing);

  return (
    <Card title="Proof of payment">
      <p className="muted">
        {proof.pictured.count} of {payments(proof.all.count)} {claim ? "you are claiming" : "you made"}{" "}
        {proof.pictured.count === 1 ? "has" : "have"} a bill or screenshot attached.
      </p>
      <div className="stats">
        <Stat label="With a picture" value={money(proof.pictured.total)} hint={payments(proof.pictured.count)} />
        <Stat label="Without a picture" value={money(proof.all.total - proof.pictured.total)} hint={payments(without)} />
      </div>
      <List>
        {proof.modes.map((m) => (
          <ListRow
            key={m.name}
            title={m.name}
            detail={`${payments(m.all.count)} · ${m.pictured.count} with a picture`}
            value={<Money paise={m.all.total} />}
          />
        ))}
      </List>
      {without > 0 && (
        <>
          <div className="actions">
            <Button aria-expanded={listing} onClick={() => setListing(!listing)}>
              {listing ? "Hide them" : `Show the ${without} without a picture`}
            </Button>
          </div>
          {listing && (
            <>
              <p className="muted">Open one to attach its bill or payment screenshot.</p>
              <List>
                {expenses
                  .filter((e) => missing.has(e.id))
                  .map((e) => (
                    <ExpenseRow key={e.id} expense={e} onClick={() => onOpen(e)} />
                  ))}
              </List>
            </>
          )}
        </>
      )}
    </Card>
  );
}
