// Things entered once and reused: people, companies, quick entries.
import { Input, Segmented, Select, Sheet, TextArea, Toggle } from "../components/ui";
import { moneyInput } from "../format";
import { categoryOptions } from "../labels";
import { useApp } from "../state";
import type { Company, Person, QuickEntry } from "../types";
import { DateField } from "../components/DateField";
import { amount, Form, useForm } from "./useForm";

const RELATIONS = ["Colleague", "Partner", "Friend", "Family", "Client", "Other"];

export function PersonForm({ person, companyId, onClose }: { person?: Person; companyId?: string; onClose: () => void }) {
  const { data, save, remove, notify } = useApp();
  const form = useForm({
    name: person?.name ?? "",
    relation: person?.relation ?? "Colleague",
    company_id: person?.company_id ?? companyId ?? data.profile.company_id,
    phone: person?.phone ?? "",
    notes: person?.notes ?? "",
  });
  const v = form.values;
  const submit = form.submit(async () => {
    await save("person", { ...v, name: v.name.trim() }, person?.id);
    notify("Saved");
    onClose();
  });
  return (
    <Sheet title={person ? "Edit person" : "Add person"} onClose={onClose}>
      <Form
        onSubmit={submit}
        busy={form.busy}
        message={form.message}
        submitLabel="Save"
        onDelete={
          person &&
          (async () => {
            if (await remove("person", person.id, `Delete ${person.name}?`, "Only possible if no entry uses them.")) onClose();
          })
        }
      >
        <Input label="Name" value={v.name} onChange={(x) => form.set("name", x)} error={form.errors.name} autoFocus={!person} />
        <div className="grid-2">
          <Select label="Relation" value={v.relation} onChange={(x) => form.set("relation", x)} options={RELATIONS.map((r) => ({ value: r, label: r }))} />
          <Select
            label="Company"
            value={v.company_id}
            onChange={(x) => form.set("company_id", x)}
            error={form.errors.company_id}
            options={[{ value: "", label: "None" }, ...data.companies.map((c) => ({ value: c.id, label: c.name }))]}
          />
        </div>
        <Input label="Phone" type="tel" value={v.phone} onChange={(x) => form.set("phone", x)} />
        <TextArea label="Notes" value={v.notes} onChange={(x) => form.set("notes", x)} />
      </Form>
    </Sheet>
  );
}

export function CompanyForm({ company, onClose }: { company?: Company; onClose: () => void }) {
  const { data, save, remove, notify } = useApp();
  const wasCurrent = !!company && data.profile.company_id === company.id;
  const form = useForm({
    name: company?.name ?? "",
    designation: company?.designation ?? "",
    employee_id: company?.employee_id ?? "",
    department: company?.department ?? "",
    joined_on: company?.joined_on ?? "",
    left_on: company?.left_on ?? "",
    address: company?.address ?? "",
    gstin: company?.gstin ?? "",
    contact: company?.contact ?? "",
    notes: company?.notes ?? "",
    current: company ? wasCurrent : !data.profile.company_id,
  });
  const v = form.values;
  const submit = form.submit(async () => {
    const { current, ...body } = v;
    const id = await save("company", { ...body, name: v.name.trim(), left_on: current ? "" : v.left_on }, company?.id);
    if (current !== wasCurrent) {
      await save("profile", { ...data.profile, company_id: current ? id : "" });
    }
    notify("Saved");
    onClose();
  });
  return (
    <Sheet title={company ? "Edit company" : "Add company"} onClose={onClose}>
      <Form
        onSubmit={submit}
        busy={form.busy}
        message={form.message}
        submitLabel="Save"
        onDelete={
          company &&
          (async () => {
            if (await remove("company", company.id, `Delete ${company.name}?`, "Only possible if no tour uses it.")) onClose();
          })
        }
      >
        <Input label="Company name" value={v.name} onChange={(x) => form.set("name", x)} error={form.errors.name} autoFocus={!company} />
        <Toggle label="I work here now" checked={v.current} onChange={(x) => form.set("current", x)} />
        <div className="grid-2">
          <Input label="My designation" value={v.designation} onChange={(x) => form.set("designation", x)} />
          <Input label="My employee ID" value={v.employee_id} onChange={(x) => form.set("employee_id", x)} />
          <Input label="My department" value={v.department} onChange={(x) => form.set("department", x)} />
          <Input label="Who approves claims" value={v.contact} onChange={(x) => form.set("contact", x)} />
          <DateField optional label="Joined on" value={v.joined_on} onChange={(x) => form.set("joined_on", x)} error={form.errors.joined_on} />
          {!v.current && (
            <DateField optional label="Left on" value={v.left_on} onChange={(x) => form.set("left_on", x)} error={form.errors.left_on} />
          )}
        </div>
        <TextArea label="Company address" value={v.address} onChange={(x) => form.set("address", x)} />
        <Input label="GSTIN" value={v.gstin} onChange={(x) => form.set("gstin", x)} />
        <TextArea label="Notes" value={v.notes} onChange={(x) => form.set("notes", x)} />
      </Form>
    </Sheet>
  );
}

export function QuickForm({ quick, onClose }: { quick?: QuickEntry; onClose: () => void }) {
  const { save, remove, notify } = useApp();
  const form = useForm({
    label: quick?.label ?? "",
    category: quick?.category ?? "local",
    from: quick?.from ?? "",
    to: quick?.to ?? "",
    mode: quick?.mode ?? "",
    amount: moneyInput(quick?.amount ?? 0),
    claim: quick?.claim ?? "reimbursable",
  });
  const v = form.values;
  const submit = form.submit(async () => {
    // The form does not offer the round-trip mark, so it is carried over.
    const body = { ...v, label: v.label.trim(), amount: amount("amount", v.amount), round_trip: quick?.round_trip };
    await save("quick", body, quick?.id);
    notify("Saved");
    onClose();
  });
  return (
    <Sheet title={quick ? "Edit quick entry" : "Add quick entry"} onClose={onClose}>
      <Form
        onSubmit={submit}
        busy={form.busy}
        message={form.message}
        submitLabel="Save"
        onDelete={
          quick &&
          (async () => {
            if (await remove("quick", quick.id, "Delete this quick entry?", "Past expenses stay.")) onClose();
          })
        }
      >
        <Input label="Name" value={v.label} onChange={(x) => form.set("label", x)} error={form.errors.label} placeholder="Home → station" autoFocus={!quick} />
        <div className="grid-2">
          <Select label="Category" value={v.category} onChange={(x) => form.set("category", x)} options={categoryOptions} />
          <Input label="Usual amount (₹)" value={v.amount} onChange={(x) => form.set("amount", x)} error={form.errors.amount} inputMode="decimal" />
          <Input label="From" value={v.from} onChange={(x) => form.set("from", x)} />
          <Input label="To" value={v.to} onChange={(x) => form.set("to", x)} />
          <Input label="By" value={v.mode} onChange={(x) => form.set("mode", x)} />
        </div>
        <Segmented
          label="Claim"
          value={v.claim}
          onChange={(x) => form.set("claim", x)}
          options={[
            { value: "reimbursable", label: "Reimbursable" },
            { value: "personal", label: "Personal" },
          ]}
        />
      </Form>
    </Sheet>
  );
}
