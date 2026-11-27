import { useState } from "react";
import { today } from "../api";
import { useCompanyFilter } from "../components/shared";
import { Button, Chips, Input, Segmented, Select, Sheet, TextArea, Toggle } from "../components/ui";
import { moneyInput } from "../format";
import { EVENT_TYPES, PURPOSES, TOUR_STATUS } from "../labels";
import { navigate } from "../router";
import { useApp } from "../state";
import type { Tour, TourStatus } from "../types";
import { DateField } from "../components/DateField";
import { amount, Form, useForm } from "./useForm";

export function TourForm({ tour, onClose }: { tour?: Tour; onClose: () => void }) {
  const { data, save, remove, notify, fail } = useApp();
  // A new tour starts in whatever the switcher is showing.
  const { value: showing, set: show } = useCompanyFilter();
  const startCompany = showing === "all" || showing === "personal" ? data.profile.company_id : showing;
  const form = useForm({
    name: tour?.name ?? "",
    kind: tour?.kind ?? (showing === "personal" ? "individual" : "company"),
    purpose: tour?.purpose || "business",
    event_type: tour?.event_type ?? "Trade fair",
    event_name: tour?.event_name ?? "",
    organiser: tour?.organiser ?? "",
    venue: tour?.venue ?? "",
    city: tour?.city ?? "",
    company_id: tour?.company_id ?? startCompany,
    start_date: tour?.start_date ?? today(),
    end_date: tour?.end_date ?? today(),
    participants: tour?.participants ?? [],
    budget: moneyInput(tour?.budget ?? 0),
    status: tour?.status ?? ("planned" as TourStatus),
    submitted_on: tour?.submitted_on ?? "",
    notes: tour?.notes ?? "",
    local_only: tour?.local_only ?? false,
  });
  const v = form.values;
  const [colleague, setColleague] = useState("");
  const [adding, setAdding] = useState(false);
  const toggle = (id: string) =>
    form.set("participants", v.participants.includes(id) ? v.participants.filter((x) => x !== id) : [...v.participants, id]);
  // Colleagues of the chosen company, plus anyone already on the tour.
  const choices = data.people.filter(
    (p) => v.kind !== "company" || !p.company_id || p.company_id === v.company_id || v.participants.includes(p.id),
  );

  const addColleague = async () => {
    setAdding(true);
    try {
      const id = await save("person", {
        name: colleague.trim(),
        relation: v.kind === "company" ? "Colleague" : "Friend",
        company_id: v.kind === "company" ? v.company_id : "",
      });
      form.setValues((c) => ({ ...c, participants: [...c.participants, id] }));
      setColleague("");
    } catch (error) {
      fail(error);
    } finally {
      setAdding(false);
    }
  };

  const submit = form.submit(async () => {
    const id = await save(
      "tour",
      {
        ...v,
        name: v.name.trim(),
        company_id: v.kind === "company" ? v.company_id : "",
        purpose: v.kind === "company" ? v.purpose : "",
        budget: amount("budget", v.budget),
        submitted_on: v.status === "submitted" || v.status === "settled" ? v.submitted_on || today() : "",
      },
      tour?.id,
    );
    notify(tour ? "Tour updated" : "Tour created");
    // Show the tour's own company, so it does not seem to have vanished.
    if (!tour) show(v.kind === "company" ? v.company_id || "unassigned" : "personal");
    onClose();
    if (!tour) navigate(`/tours/${id}`);
  });

  const onDelete =
    tour &&
    (async () => {
      const count = data.expenses.filter((e) => e.tour_id === tour.id).length;
      const removed = await remove(
        "tour",
        tour.id,
        `Delete ${tour.name}?`,
        `Its ${count} expense${count === 1 ? "" : "s"} and advances are deleted too, on linked devices as well.`,
      );
      if (removed) {
        onClose();
        navigate("/tours");
      }
    });

  return (
    <Sheet title={tour ? "Edit tour" : "New tour"} onClose={onClose}>
      <Form
        onSubmit={submit}
        busy={form.busy}
        message={form.message}
        submitLabel={tour ? "Save changes" : "Create tour"}
        onDelete={onDelete}
      >
        <Input
          label="Tour name"
          value={v.name}
          onChange={(x) => form.set("name", x)}
          error={form.errors.name}
          placeholder="Trade fair, New Delhi"
          autoFocus={!tour}
        />
        <Segmented
          label="Who is it for"
          value={v.kind}
          onChange={(x) => form.set("kind", x)}
          options={[
            { value: "company", label: "Company" },
            { value: "individual", label: "Personal" },
          ]}
        />
        {v.kind === "company" && (
          <Select
            label="Company"
            value={v.company_id}
            onChange={(x) => form.set("company_id", x)}
            error={form.errors.company_id}
            hint={data.companies.length ? undefined : "Add your company under More to reuse it."}
            options={[{ value: "", label: "Not chosen" }, ...data.companies.map((c) => ({ value: c.id, label: c.name }))]}
          />
        )}
        {v.kind === "company" && (
          <Select
            label="Purpose"
            value={v.purpose}
            onChange={(x) => form.set("purpose", x)}
            error={form.errors.purpose}
            hint={v.purpose === "business" ? "What you pay is marked to claim" : "What you pay is your own cost unless you mark it"}
            options={Object.entries(PURPOSES).map(([value, label]) => ({ value, label }))}
          />
        )}
        <div className="grid-2">
          <DateField label="From" value={v.start_date} onChange={(x) => form.set("start_date", x)} error={form.errors.start_date} />
          <DateField label="To" value={v.end_date} onChange={(x) => form.set("end_date", x)} error={form.errors.end_date} />
          <Select
            label="Type of event"
            value={v.event_type}
            onChange={(x) => form.set("event_type", x)}
            options={EVENT_TYPES.map((t) => ({ value: t, label: t }))}
          />
          <Input label="City" value={v.city} onChange={(x) => form.set("city", x)} placeholder="New Delhi" />
        </div>
        <Input
          label="Event"
          value={v.event_name}
          onChange={(x) => form.set("event_name", x)}
          placeholder="India International Trade Fair"
        />
        <div className="grid-2">
          <Input label="Organiser" value={v.organiser} onChange={(x) => form.set("organiser", x)} placeholder="ITPO" />
          <Input label="Venue" value={v.venue} onChange={(x) => form.set("venue", x)} placeholder="Bharat Mandapam" />
        </div>
        {choices.length > 0 && (
          <Chips
            label="Travelling with"
            options={choices.map((p) => ({ value: p.id, label: p.name }))}
            selected={v.participants}
            onToggle={toggle}
          />
        )}
        <div className="inline-row">
          <Input label="Add someone new" value={colleague} onChange={setColleague} placeholder="Name" />
          <Button disabled={adding || !colleague.trim()} onClick={addColleague}>
            Add
          </Button>
        </div>
        <div className="grid-2">
          <Input label="Budget (₹, optional)" value={v.budget} onChange={(x) => form.set("budget", x)} error={form.errors.budget} inputMode="decimal" />
          <Select
            label="Status"
            value={v.status}
            onChange={(x) => form.set("status", x as TourStatus)}
            options={Object.entries(TOUR_STATUS).map(([value, label]) => ({ value, label }))}
          />
        </div>
        {(v.status === "submitted" || v.status === "settled") && (
          <DateField label="Claim sent on" value={v.submitted_on || today()} onChange={(x) => form.set("submitted_on", x)} />
        )}
        <TextArea label="Notes" value={v.notes} onChange={(x) => form.set("notes", x)} />
        <Toggle
          label="Keep this tour on this device only"
          hint="It is not sent to linked devices. Copies they already have stay there."
          checked={v.local_only}
          onChange={(x) => form.set("local_only", x)}
        />
      </Form>
    </Sheet>
  );
}
