import { Building2, Database, Download, FileUp, Luggage, Pencil, Plus, RefreshCw, Route, Settings, User, Users, Zap } from "lucide-react";
import { useRef, useState, type ChangeEvent } from "react";
import { api, saveFile, today } from "../api";
import { Badge, Button, Card, EmptyState, Input, List, ListRow, Money, PageHeader, Segmented, Select, Toggle } from "../components/ui";
import { CompanyForm, PersonForm, QuickForm } from "../forms/DirectoryForms";
import { TransferForm } from "../forms/MoneyForms";
import { amount, Form, useForm } from "../forms/useForm";
import { dateRange, fullDay, money, moneyInput } from "../format";
import { categoryName, PURPOSES } from "../labels";
import { ClaimBadge } from "../components/shared";
import { usePreferences } from "../preferences";
import { useApp } from "../state";
import type { ExportedFile, Person, QuickEntry, Transfer } from "../types";

const back = { href: "#/more", label: "More" };

export function More() {
  const { data, sync } = useApp();
  const current = data.companies.find((c) => c.id === data.profile.company_id);
  const steps = [
    { done: !!data.profile.name, title: "Add your details", href: "#/more/profile" },
    { done: data.companies.length > 0, title: "Add your company and your role", href: "#/more/companies" },
    { done: data.people.length > 0, title: "Add the colleagues you travel with", href: current ? `#/more/companies/${current.id}` : "#/more/people" },
  ];
  return (
    <>
      <PageHeader title="More" context="Your details, your companies, your devices and your data" />
      {steps.some((s) => !s.done) && (
        <Card title="Set up once">
          <List>
            {steps.map((s, i) => (
              <ListRow key={s.title} title={`${i + 1}. ${s.title}`} detail={s.done ? "Done" : "Not yet"} href={s.href} />
            ))}
          </List>
        </Card>
      )}
      <Card title="Me and my work">
        <List>
          <ListRow icon={User} title="My details" detail={data.profile.name || "Name, phone, income"} href="#/more/profile" />
          <ListRow
            icon={Building2}
            title="My companies"
            detail={current ? `Now at ${current.name}` : `${data.companies.length} saved`}
            href="#/more/companies"
          />
          <ListRow icon={Users} title="All people" detail={`${data.people.length} saved`} href="#/more/people" />
          <ListRow icon={Route} title="Quick entries" detail="Journeys and costs you repeat" href="#/more/quick" />
        </List>
      </Card>
      <Card title="This app">
        <List>
          <ListRow icon={RefreshCw} title="Sync" detail={sync?.enabled ? `On · ${sync.peers.length} linked` : "Off"} href="#/more/sync" />
          <ListRow icon={Database} title="Backup and import" href="#/more/data" />
          <ListRow icon={Settings} title="Appearance" href="#/more/appearance" />
        </List>
      </Card>
    </>
  );
}

/** One company: my role there, the colleagues I travel with, and its tours. */
export function CompanyPage({ id }: { id: string }) {
  const { data } = useApp();
  const [editing, setEditing] = useState(false);
  const [person, setPerson] = useState<Person | "new" | null>(null);
  const back = { href: "#/more/companies", label: "My companies" };
  const company = data.companies.find((c) => c.id === id);
  if (!company) {
    return (
      <>
        <PageHeader title="Company not found" back={back} />
        <Card>
          <EmptyState icon={Building2} title="It may have been deleted on another device." />
        </Card>
      </>
    );
  }
  const people = data.people.filter((p) => p.company_id === id);
  const tours = data.tours.filter((t) => t.kind === "company" && t.company_id === id);
  const sum = (pick: (t: (typeof tours)[number]) => number) => tours.reduce((total, t) => total + pick(t), 0);
  const role: [string, string][] = [
    ["Designation", company.designation],
    ["Employee ID", company.employee_id],
    ["Department", company.department],
    ["Joined", company.joined_on && fullDay(company.joined_on)],
    ["Left", company.left_on && fullDay(company.left_on)],
    ["Approves claims", company.contact],
    ["GSTIN", company.gstin],
    ["Address", company.address],
  ];
  return (
    <>
      <PageHeader title={company.name} back={back} context={data.profile.company_id === id ? "Where you work now" : "Previous or other company"}>
        <Button icon={Pencil} onClick={() => setEditing(true)}>
          Edit company
        </Button>
      </PageHeader>
      <Card title="My role">
        {role.some(([, v]) => v) ? (
          <dl className="facts">
            {role
              .filter(([, v]) => v)
              .map(([label, value]) => (
                <div key={label}>
                  <dt className="muted">{label}</dt>
                  <dd>{value}</dd>
                </div>
              ))}
          </dl>
        ) : (
          <EmptyState icon={User} title="Add your designation and employee ID with Edit company. They go on claims." />
        )}
      </Card>
      <Card
        title="Colleagues and partners"
        action={
          <Button icon={Plus} onClick={() => setPerson("new")}>
            Add colleague
          </Button>
        }
      >
        {people.length ? (
          <List>
            {people.map((p) => (
              <ListRow key={p.id} icon={User} title={p.name} detail={p.relation} onClick={() => setPerson(p)} />
            ))}
          </List>
        ) : (
          <EmptyState icon={Users} title="Add the people from this company you travel with." />
        )}
      </Card>
      <Card title="Tours and trips">
        {tours.length ? (
          <>
            <p className="muted">
              Claimed {money(sum((t) => t.summary.claimable))} · reimbursed {money(sum((t) => t.summary.reimbursed))} · still to
              receive {money(sum((t) => (t.status === "settled" ? 0 : Math.max(t.summary.net, 0))))}
            </p>
            <List>
              {tours.map((t) => (
                <ListRow
                  key={t.id}
                  icon={Luggage}
                  title={t.name}
                  detail={[dateRange(t.start_date, t.end_date), PURPOSES[t.purpose || "business"], t.city].filter(Boolean).join(" · ")}
                  value={<Money paise={t.summary.total_cost} />}
                  note={<ClaimBadge state={t.claim_state} />}
                  href={`#/tours/${t.id}/report`}
                />
              ))}
            </List>
          </>
        ) : (
          <EmptyState icon={Luggage} title="No tours with this company yet." />
        )}
      </Card>
      {editing && <CompanyForm company={company} onClose={() => setEditing(false)} />}
      {person && <PersonForm person={person === "new" ? undefined : person} companyId={id} onClose={() => setPerson(null)} />}
    </>
  );
}

export function ProfilePage() {
  const { data, save, notify } = useApp();
  const p = data.profile;
  const form = useForm({ ...p, monthly_income: moneyInput(p.monthly_income) as string | number });
  const v = form.values;
  const text = (key: Exclude<keyof typeof v, "monthly_income">, label: string, type?: "email" | "tel") => (
    <Input label={label} type={type} value={v[key]} onChange={(x) => form.set(key, x)} />
  );
  return (
    <>
      <PageHeader title="My details" back={back} context="Your own details, filled into every claim" />
      <Card>
        <Form
          onSubmit={form.submit(async () => {
            await save("profile", { ...v, monthly_income: amount("monthly_income", String(v.monthly_income)) });
            notify("Profile saved");
          })}
          busy={form.busy}
          message={form.message}
          submitLabel="Save profile"
        >
          <div className="grid-2">
            {text("name", "Name")}
            <Select
              label="Company I work for now"
              value={v.company_id}
              onChange={(x) => form.set("company_id", x)}
              hint={data.companies.length ? undefined : "Add it under More → Companies"}
              options={[{ value: "", label: "None" }, ...data.companies.map((c) => ({ value: c.id, label: c.name }))]}
            />
            {text("home_city", "Home city")}
            {text("phone", "Phone", "tel")}
            {text("email", "Email", "email")}
            {text("upi_id", "UPI ID for reimbursement")}
            <Input
              label="Monthly income (₹)"
              value={String(v.monthly_income)}
              onChange={(x) => form.set("monthly_income", x)}
              error={form.errors.monthly_income}
              inputMode="decimal"
              hint="Used only to estimate savings"
            />
          </div>
          <p className="muted">
            Designation, employee ID and department are kept with each company, so claims stay right after a job change.
          </p>
        </Form>
      </Card>
    </>
  );
}

export function People() {
  const { data } = useApp();
  const [editing, setEditing] = useState<Person | "new" | null>(null);
  const [settle, setSettle] = useState<Partial<Transfer> | null>(null);
  return (
    <>
      <PageHeader title="People" back={back} context="Colleagues and partners you travel with">
        <Button variant="primary" icon={Plus} onClick={() => setEditing("new")}>
          Add person
        </Button>
      </PageHeader>
      <Card>
        {data.people.length ? (
          <List>
            {data.people.map((p) => {
              const balance = data.balances[p.id] ?? 0;
              return (
                <ListRow
                  key={p.id}
                  icon={User}
                  title={p.name}
                  detail={[p.relation, data.companies.find((c) => c.id === p.company_id)?.name].filter(Boolean).join(" · ")}
                  value={balance ? <Money paise={Math.abs(balance)} tone={balance > 0 ? "good" : "bad"} /> : undefined}
                  note={balance > 0 ? "owes me" : balance < 0 ? "I owe" : undefined}
                  onClick={() => setEditing(p)}
                >
                  {balance !== 0 && (
                    <Button
                      variant="ghost"
                      onClick={() =>
                        setSettle({ person_id: p.id, amount: Math.abs(balance), kind: balance > 0 ? "received_from" : "paid_to" })
                      }
                    >
                      Settle {money(Math.abs(balance))}
                    </Button>
                  )}
                </ListRow>
              );
            })}
          </List>
        ) : (
          <EmptyState icon={Users} title="Add the people you travel with to tag tours and split costs." />
        )}
      </Card>
      {editing && <PersonForm person={editing === "new" ? undefined : editing} onClose={() => setEditing(null)} />}
      {settle && <TransferForm preset={settle} onClose={() => setSettle(null)} />}
    </>
  );
}

export function Companies() {
  const { data } = useApp();
  const [adding, setAdding] = useState(false);
  return (
    <>
      <PageHeader title="My companies" back={back} context="Open one to see your role, colleagues and tours there">
        <Button variant="primary" icon={Plus} onClick={() => setAdding(true)}>
          Add company
        </Button>
      </PageHeader>
      <Card>
        {data.companies.length ? (
          <List>
            {data.companies.map((c) => (
              <ListRow
                key={c.id}
                icon={Building2}
                title={c.name}
                detail={[c.designation, c.employee_id && `ID ${c.employee_id}`, c.left_on && `left ${fullDay(c.left_on)}`]
                  .filter(Boolean)
                  .join(" · ")}
                note={data.profile.company_id === c.id ? <Badge tone="good">Current</Badge> : c.left_on ? <Badge>Previous</Badge> : undefined}
                href={`#/more/companies/${c.id}`}
              />
            ))}
          </List>
        ) : (
          <EmptyState icon={Building2} title="Save your company once, with your role there, and pick it on each tour." />
        )}
      </Card>
      {adding && <CompanyForm onClose={() => setAdding(false)} />}
    </>
  );
}

export function QuickEntries() {
  const { data } = useApp();
  const [editing, setEditing] = useState<QuickEntry | "new" | null>(null);
  return (
    <>
      <PageHeader title="Quick entries" back={back} context="One tap fills the expense form">
        <Button variant="primary" icon={Plus} onClick={() => setEditing("new")}>
          Add quick entry
        </Button>
      </PageHeader>
      <Card>
        {data.quick.length ? (
          <List>
            {data.quick.map((q) => (
              <ListRow
                key={q.id}
                icon={Zap}
                title={q.label}
                detail={[categoryName(q.category), q.mode].filter(Boolean).join(" · ")}
                value={<Money paise={q.amount} />}
                onClick={() => setEditing(q)}
              />
            ))}
          </List>
        ) : (
          <EmptyState icon={Zap} title="Save a ride you take often, such as home to the railway station." />
        )}
      </Card>
      {editing && <QuickForm quick={editing === "new" ? undefined : editing} onClose={() => setEditing(null)} />}
    </>
  );
}

export function DataPage() {
  const { reload, notify, fail, confirm } = useApp();
  const syncFile = useRef<HTMLInputElement>(null);
  const csvFile = useRef<HTMLInputElement>(null);

  const exportBundle = async (include_local: boolean) => {
    try {
      const file = await api<ExportedFile>({ op: "export_bundle", include_local, today: today() });
      if (await saveFile(file)) notify("Saved");
    } catch (error) {
      fail(error);
    }
  };

  const importFile = (op: "import_bundle" | "import_csv") => async (event: ChangeEvent<HTMLInputElement>) => {
    const input = event.target;
    const file = input.files?.[0];
    if (!file) return;
    input.value = "";
    try {
      // A backup holds the bill pictures, so it is far bigger than a spreadsheet.
      const limit = op === "import_bundle" ? 400 : 20;
      if (file.size > limit * 1024 * 1024) throw new Error("That file is too large.");
      const content = await file.text();
      const yes = await confirm(
        `Import ${file.name}?`,
        op === "import_bundle"
          ? "Newer entries in the file replace older ones here, including deletions."
          : "Every row is added as a new personal expense. Importing twice adds them twice.",
        "Import",
      );
      if (!yes) return;
      const result = await api<{ imported: number }>({ op, content });
      await reload();
      notify(result.imported ? `Imported ${result.imported} items` : "Nothing new in that file");
    } catch (error) {
      fail(error);
    }
  };

  return (
    <>
      <PageHeader title="Backup and import" back={back} context="Keep a copy, or bring entries in from a file" />
      <Card title="Backup">
        <p>
          A backup holds everything, including items kept on this device only. A sync file leaves those out and can be
          imported on another device without a network. Neither is encrypted, so keep them somewhere private.
        </p>
        <div className="actions">
          <Button icon={Download} onClick={() => exportBundle(true)}>
            Save backup
          </Button>
          <Button icon={Download} onClick={() => exportBundle(false)}>
            Save sync file
          </Button>
          <Button icon={FileUp} onClick={() => syncFile.current?.click()}>
            Import backup or sync file
          </Button>
        </div>
        <input ref={syncFile} type="file" accept=".json,application/json" hidden onChange={importFile("import_bundle")} />
      </Card>
      <Card title="Spreadsheet import">
        <p>
          A CSV file with the columns <code>date</code> and <code>amount</code>, and optionally <code>category</code>,{" "}
          <code>description</code>, <code>from</code> and <code>to</code>. Dates as 2026-11-27 or 27/11/2026.
        </p>
        <div className="actions">
          <Button icon={FileUp} onClick={() => csvFile.current?.click()}>
            Import CSV
          </Button>
        </div>
        <input ref={csvFile} type="file" accept=".csv,text/csv" hidden onChange={importFile("import_csv")} />
      </Card>
    </>
  );
}

export function Appearance() {
  const { preferences, update } = usePreferences();
  return (
    <>
      <PageHeader title="Appearance" back={back} context="Applies to this device" />
      <Card>
        <Segmented
          label="Theme"
          value={preferences.theme}
          onChange={(theme) => update({ theme })}
          options={[
            { value: "system", label: "System" },
            { value: "light", label: "Light" },
            { value: "dark", label: "Dark" },
          ]}
        />
        <Toggle
          label="Reduce effects"
          hint="Solid panels and no motion. Helps older phones and battery."
          checked={preferences.reduced}
          onChange={(reduced) => update({ reduced })}
        />
      </Card>
    </>
  );
}
