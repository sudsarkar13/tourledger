// The report as it will be handed in, with the PDF and spreadsheet buttons.
import { Download, FileSpreadsheet, ReceiptText } from "lucide-react";
import { useEffect, useState } from "react";
import { api, saveFile, today } from "../api";
import { ReportPreview } from "../components/ReportPreview";
import { Button, Card, EmptyState, List, ListRow, Money, PageHeader, Segmented, Toggle } from "../components/ui";
import { day, fullDay, money } from "../format";
import { categoryName } from "../labels";
import { usePreferences } from "../preferences";
import { useApp } from "../state";
import type { ExportedFile, Report, ReportLine } from "../types";

function Lines({ lines }: { lines: ReportLine[] }) {
  return (
    <List>
      {lines.map((l) => (
        <ListRow
          key={l.id}
          title={l.particulars}
          detail={[
            day(l.date),
            `paid by ${l.paid_by}`,
            l.people && `for ${l.people}`,
            l.bill_no && `bill ${l.bill_no}`,
            l.bills.length > 0 && `picture ${l.bills.map((n) => `#${n}`).join(" ")}`,
          ]
            .filter(Boolean)
            .join(" · ")}
          value={<Money paise={l.amount} />}
        />
      ))}
    </List>
  );
}

export function ClaimReport({ id }: { id: string }) {
  const { data, save, notify, fail } = useApp();
  const [report, setReport] = useState<Report | null>(null);
  const [busy, setBusy] = useState(false);
  const [view, setView] = useState<"summary" | "preview">("summary");
  const { preferences, update } = usePreferences();
  // Totals by category is what is handed over unless detail is asked for.
  const detail = preferences.reportDetail;
  // Off until asked for: it makes a declaration in the person's name.
  const proof = preferences.reportProof;

  useEffect(() => {
    let stale = false;
    api<Report>({ op: "claim_report", tour_id: id, today: today(), detail, proof })
      .then((r) => !stale && setReport(r))
      .catch((e) => !stale && fail(e));
    return () => {
      stale = true;
    };
    // Rebuilt when anything is saved or arrives from another device.
  }, [id, detail, proof, data.revision, fail]);

  const refile = async (expenseId: string, category: string) => {
    const expense = data.expenses.find((e) => e.id === expenseId);
    if (!expense) return;
    const { id: _id, home: _home, ...body } = expense;
    try {
      await save("expense", { ...body, category }, expenseId);
      notify(`Filed as ${categoryName(category)}`);
    } catch (error) {
      fail(error);
    }
  };

  const download = async (format: "pdf" | "csv") => {
    setBusy(true);
    try {
      const file = await api<ExportedFile>({ op: "export_claim", tour_id: id, format, today: today(), detail, proof });
      if (await saveFile(file)) notify(`Saved ${file.name}`);
    } catch (error) {
      fail(error);
    } finally {
      setBusy(false);
    }
  };

  const back = { href: `#/tours/${id}`, label: "Tour" };
  if (!report) return <PageHeader title="Report" back={back} />;
  const claimed = report.groups.filter((g) => g.lines.length);
  const away = report.bills.filter((b) => !b.present).length;

  return (
    <>
      <PageHeader title={report.title} back={back} context={[report.company, report.tour].filter(Boolean).join(" · ")}>
        <Button icon={FileSpreadsheet} disabled={busy} onClick={() => download("csv")}>
          Spreadsheet
        </Button>
        <Button variant="primary" icon={Download} disabled={busy} onClick={() => download("pdf")}>
          Save PDF
        </Button>
      </PageHeader>

      <Segmented
        label="What the report lists (PDF and spreadsheet)"
        value={detail ? "entries" : "category"}
        onChange={(x) => update({ reportDetail: x === "entries" })}
        options={[
          { value: "category", label: "Category totals" },
          { value: "entries", label: "Every entry" },
        ]}
      />
      <Toggle
        label="Add proof of payment"
        hint="How each payment was made, and a declaration in your name for those with no picture"
        checked={proof}
        onChange={(reportProof) => update({ reportProof })}
      />
      <Segmented
        label="Show"
        value={view}
        onChange={setView}
        options={[
          { value: "summary", label: "On screen" },
          { value: "preview", label: "Preview the PDF" },
        ]}
      />
      {report.without_bill > 0 && claimed.length > 0 && (
        <p className="muted">
          {report.without_bill} {report.without_bill === 1 ? "entry" : "entries"} in this claim with no bill picture. Open an
          expense to add one.
        </p>
      )}

      {away > 0 && (
        <p className="error" role="alert">
          {away} {away === 1 ? "picture has" : "pictures have"} not arrived on this device yet, so the PDF will not show{" "}
          {away === 1 ? "it" : "them"}. Sync with your other device before saving.
        </p>
      )}

      {report.check.length > 0 && (
        <Card title="Check these categories">
          <p className="muted">The note points to another category than the one chosen. Nothing changes unless you say so.</p>
          <List>
            {report.check.map((c) => (
              <ListRow
                key={c.id}
                title={c.particulars}
                detail={`Filed as ${categoryName(c.filed)}`}
                value={<Money paise={c.amount} />}
              >
                <Button disabled={busy} onClick={() => refile(c.id, c.suggested)}>
                  File as {categoryName(c.suggested)}
                </Button>
              </ListRow>
            ))}
          </List>
        </Card>
      )}

      {view === "preview" ? (
        <ReportPreview tourId={id} detail={detail} proof={proof} />
      ) : (
        <>
      <Card title="Who and where">
        <dl className="facts">
          {[...report.claimant, ...report.details].map(([label, value]) => (
            <div key={label}>
              <dt className="muted">{label}</dt>
              <dd>{value}</dd>
            </div>
          ))}
        </dl>
        {!data.profile.name && <p className="error">Your name is missing. Add it under More → My profile.</p>}
      </Card>

      <Card title="Summary by category">
        {claimed.length ? (
          <List>
            {claimed.map((g) => (
              <ListRow key={g.name} title={g.name} value={<Money paise={g.total} />} />
            ))}
            <ListRow title={report.is_claim ? "Total claimed" : "Total spent"} value={<Money paise={report.total} />} />
          </List>
        ) : (
          <EmptyState icon={ReceiptText} title="No expenses to list yet." />
        )}
      </Card>

      {detail && claimed.map((g) => (
        <Card key={g.name} title={`${g.name}: ${money(g.total)}`}>
          <Lines lines={g.lines} />
        </Card>
      ))}

      {report.company_total > 0 && (
        <Card title={`Paid directly by the company: ${money(report.company_total)}`}>
          <p className="muted">Listed for the record. Not part of what you claim.</p>
          {detail ? (
            <Lines lines={report.groups.flatMap((g) => g.company_lines)} />
          ) : (
            <List>
              {report.groups
                .filter((g) => g.company_total > 0)
                .map((g) => (
                  <ListRow key={g.name} title={g.name} value={<Money paise={g.company_total} />} />
                ))}
            </List>
          )}
        </Card>
      )}

      {report.stages.length > 0 && (
        <Card title="Spent at each stage of the trip">
          <List>
            {report.stages.map((s) => (
              <ListRow key={s.name} title={s.name} value={<Money paise={s.total} />} />
            ))}
          </List>
        </Card>
      )}

      {report.by_person.length > 0 && (
        <Card title="Spent for each person">
          <List>
            {report.by_person.map(([name, value]) => (
              <ListRow key={name} title={name} value={<Money paise={value} />} />
            ))}
          </List>
        </Card>
      )}

      {report.proof && report.proof_summary.all.count > 0 && (
        <Card title="How I paid">
          <List>
            {report.proof_summary.modes.map((m) => (
              <ListRow
                key={m.name}
                title={m.name}
                detail={`${m.all.count} paid, ${m.pictured.count} with a picture`}
                value={<Money paise={m.all.total} />}
              />
            ))}
          </List>
        </Card>
      )}

      {report.is_claim && (
        <Card title="Settlement">
          <List>
            <ListRow title="Total claimed" value={<Money paise={report.total} />} />
            {report.money.map(([date, label, amount], i) => (
              <ListRow
                key={i}
                title={`${label.includes("returned") ? "Add" : "Less"}: ${label}`}
                detail={fullDay(date)}
                value={<Money paise={amount} />}
              />
            ))}
            <ListRow
              title={report.net_label}
              detail={report.net_words}
              value={<Money paise={Math.abs(report.net)} tone={report.net > 0 ? "good" : report.net < 0 ? "bad" : undefined} />}
            />
          </List>
          {report.allowance > 0 && (
            <p className="muted">
              Bonus or allowance of {money(report.allowance)} received from the company, not adjusted here.
            </p>
          )}
        </Card>
      )}

      {report.proof && report.unproved.length > 0 && (
        <Card title={`No bill or screenshot attached: ${money(report.unproved_total)}`}>
          <p>{report.statement}</p>
          <List>
            {report.unproved.map((l) => (
              <ListRow
                key={l.id}
                title={detail ? l.particulars : l.category}
                detail={`${day(l.date)} · ${l.mode}`}
                value={<Money paise={l.amount} />}
              />
            ))}
          </List>
          <p className="muted">
            {[report.declared_by, report.declared_on].filter(Boolean).join(" · ")}
          </p>
        </Card>
      )}

      {report.left_out_count > 0 && (
        <p className="muted">
          Not in this claim: {report.left_out_count} personal, rejected or colleague-paid{" "}
          {report.left_out_count === 1 ? "entry" : "entries"} ({money(report.left_out_total)}). They stay private to you.
        </p>
      )}
        </>
      )}
    </>
  );
}
