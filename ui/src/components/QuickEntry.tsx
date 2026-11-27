// Type what happened, one line each; check what was understood; save. The reading
// is done by rules in Rust, and nothing is saved before it is shown here.
import { Pencil, Trash2 } from "lucide-react";
import { useState } from "react";
import { api, ApiError, today } from "../api";
import { ExpenseForm } from "../forms/ExpenseForm";
import { TransferForm } from "../forms/MoneyForms";
import { day, money } from "../format";
import { CATEGORIES, categoryName, personName, TRANSFER_KINDS } from "../labels";
import { useApp } from "../state";
import type { Draft, Snapshot } from "../types";
import { Button, Card, IconButton, List, ListRow, Select, TextArea } from "./ui";

function expenseBody(d: Draft, tourId: string) {
  return {
    tour_id: tourId,
    date: d.date,
    category: d.category,
    description: d.description,
    from: d.from,
    to: d.to,
    mode: d.mode,
    amount: d.amount,
    paid_by: d.paid_by,
    claim: tourId ? d.claim : ("personal" as const),
    split_with: d.split_with,
    bill_no: d.bill_no,
    payment_method: d.payment_method,
  };
}

const transferBody = (d: Draft, tourId: string) => ({
  tour_id: tourId,
  date: d.date,
  kind: d.kind as "advance" | "advance_returned" | "reimbursement" | "allowance",
  amount: d.amount,
});

function describe(data: Snapshot, d: Draft): { title: string; detail: string } {
  if (d.kind !== "expense") {
    return { title: TRANSFER_KINDS[d.kind], detail: day(d.date) };
  }
  const route = d.from && d.to ? `${d.from} → ${d.to}` : "";
  const sharers = d.split_with.map((id) => personName(data, id)).join(", ");
  return {
    title: [categoryName(d.category), route || d.description].filter(Boolean).join(": "),
    detail: [
      day(d.date),
      d.mode,
      d.payment_method,
      `paid by ${d.paid_by === "me" ? "me" : personName(data, d.paid_by)}`,
      d.paid_by !== "me" ? "" : d.claim === "reimbursable" ? "to claim" : "my own cost",
      sharers && `for ${sharers}`,
      d.bill_no && `bill ${d.bill_no}`,
    ]
      .filter(Boolean)
      .join(" · "),
  };
}

export function QuickEntry({ tourId, tourChoices }: { tourId: string; tourChoices?: boolean }) {
  const { data, reload, notify, fail } = useApp();
  const [chosen, setChosen] = useState(tourId);
  const target = tourChoices ? chosen : tourId;
  const [text, setText] = useState("");
  const [drafts, setDrafts] = useState<Draft[] | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [editing, setEditing] = useState<number | null>(null);
  const tour = data.tours.find((t) => t.id === target);

  const check = async () => {
    setBusy(true);
    setError("");
    try {
      const result = await api<{ drafts: Draft[] }>({ op: "parse_entry", text, tour_id: target, today: today() });
      setDrafts(result.drafts);
    } catch (e) {
      if (e instanceof ApiError) setError(e.message);
      else fail(e);
    } finally {
      setBusy(false);
    }
  };

  const problem = (d: Draft) =>
    d.problem || (d.kind !== "expense" && tour?.kind !== "company" ? "Company money needs a company tour." : "");
  const ready = drafts?.filter((d) => !problem(d)) ?? [];

  const saveAll = async () => {
    setBusy(true);
    try {
      await api({
        op: "save_many",
        items: ready.map((d) =>
          d.kind === "expense"
            ? { kind: "expense", body: expenseBody(d, target) }
            : { kind: "transfer", body: transferBody(d, target) },
        ),
      });
      await reload();
      notify(ready.length === 1 ? "1 entry saved" : `${ready.length} entries saved`);
      // Lines that could not be saved stay in the box to be fixed.
      const rest = drafts!.filter((d) => problem(d)).map((d) => d.line);
      setText(rest.join("\n"));
      setDrafts(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const drop = (index: number) => {
    const next = drafts!.filter((_, i) => i !== index);
    setDrafts(next.length ? next : null);
    setText(next.map((d) => d.line).join("\n"));
  };

  const current = editing === null ? null : drafts?.[editing];

  return (
    <Card title="Quick entry">
      <p className="muted">
        One line for each thing, for example <em>auto home to station 180 on 14 Nov</em>, <em>lunch with Ravi 640 upi</em>,{" "}
        <em>hotel 9600 company paid</em> or <em>company gave 2000</em>.
      </p>
      {tourChoices && (
        <Select
          label="Add to"
          value={chosen}
          onChange={(x) => {
            setChosen(x);
            setDrafts(null);
          }}
          options={[
            ...data.tours.map((t) => ({ value: t.id, label: t.name })),
            { value: "", label: "Not part of a tour (daily cost)" },
          ]}
        />
      )}
      <TextArea
        label="What did you spend or receive?"
        value={text}
        onChange={(x) => {
          setText(x);
          setDrafts(null);
          setError("");
        }}
        error={error || undefined}
        placeholder={"cab home to station 180\nlunch 240\ncompany gave 2000"}
      />
      {drafts && (
        <>
          <p>This is what was understood. Check it, change anything wrong, then save.</p>
          <List>
            {drafts.map((d, i) => {
              const { title, detail } = describe(data, d);
              const notes = [d.date_assumed ? "No date typed, so one was chosen." : "", ...d.warnings, problem(d)]
                .filter(Boolean)
                .join(" ");
              return (
                <ListRow
                  key={i}
                  icon={d.kind === "expense" ? CATEGORIES[d.category]?.icon : undefined}
                  title={title}
                  detail={
                    <>
                      {detail}
                      {notes && <span className={problem(d) ? "error note" : "note"}>{notes}</span>}
                    </>
                  }
                  value={d.amount ? money(d.amount) : "?"}
                >
                  {!problem(d) && <IconButton label="Change this entry" icon={Pencil} onClick={() => setEditing(i)} />}
                  <IconButton label="Leave this line out" icon={Trash2} onClick={() => drop(i)} />
                </ListRow>
              );
            })}
          </List>
        </>
      )}
      <div className="actions">
        {drafts ? (
          <>
            <Button variant="primary" disabled={busy || !ready.length} onClick={saveAll}>
              {ready.length === 1 ? "Save 1 entry" : `Save ${ready.length} entries`}
            </Button>
            <Button onClick={() => setDrafts(null)}>Back to typing</Button>
          </>
        ) : (
          <Button variant="primary" disabled={busy || !text.trim()} onClick={check}>
            Check what I typed
          </Button>
        )}
      </div>
      {/* Changing one entry opens the full form, which saves it by itself. */}
      {current && current.kind === "expense" && (
        <ExpenseForm
          preset={expenseBody(current, target)}
          onSaved={() => drop(editing!)}
          onClose={() => setEditing(null)}
        />
      )}
      {current && current.kind !== "expense" && (
        <TransferForm
          tourId={target}
          preset={transferBody(current, target)}
          onSaved={() => drop(editing!)}
          onClose={() => setEditing(null)}
        />
      )}
    </Card>
  );
}
