// Pieces tied to this app's data that several pages show.
import { usePreferences } from "../preferences";
import { useApp } from "../state";
import type { ClaimState, Tour } from "../types";
import { Badge, Select } from "./ui";

/** Company tours that have no company chosen. */
const UNASSIGNED = "unassigned";

/**
 * Which one set of tours is on screen: a single company or personal trips. They
 * are never shown mixed (docs/DESIGN.md).
 */
export function useCompanyFilter() {
  const { data } = useApp();
  const { preferences, update } = usePreferences();
  const unassigned = data.tours.some((t) => t.kind === "company" && !data.companies.some((c) => c.id === t.company_id));
  const choices = [
    ...[...data.companies]
      .sort(
        (a, b) =>
          Number(b.id === data.profile.company_id) - Number(a.id === data.profile.company_id) ||
          Number(!!a.left_on) - Number(!!b.left_on),
      )
      .map((c) => ({
        value: c.id,
        label: c.id === data.profile.company_id ? `${c.name} (current)` : c.left_on ? `${c.name} (previous)` : c.name,
      })),
    ...(unassigned ? [{ value: UNASSIGNED, label: "Company not chosen" }] : []),
    { value: "personal", label: "Personal trips" },
  ];
  // The current company comes first, so it is also what a new device shows.
  const value = choices.some((c) => c.value === preferences.company) ? preferences.company : choices[0].value;
  const matches = (tour: Tour) => {
    if (value === "personal") return tour.kind === "individual";
    if (tour.kind !== "company") return false;
    return value === UNASSIGNED ? !data.companies.some((c) => c.id === tour.company_id) : tour.company_id === value;
  };
  return {
    value,
    choices,
    companyId: value === "personal" || value === UNASSIGNED ? "" : value,
    set: (company: string) => update({ company }),
    tours: data.tours.filter(matches),
  };
}

export function CompanySwitcher() {
  const { value, choices, set } = useCompanyFilter();
  if (choices.length < 2) return null;
  return (
    <div className="switcher">
      <Select label="Company" value={value} onChange={set} options={choices} />
    </div>
  );
}

export const CLAIM_STATES: Record<ClaimState, { label: string; tone: "neutral" | "good" | "warn" | "bad" }> = {
  "": { label: "Personal", tone: "neutral" },
  none: { label: "Nothing to claim", tone: "neutral" },
  not_sent: { label: "Claim not sent", tone: "warn" },
  sent: { label: "Waiting for payment", tone: "warn" },
  part_paid: { label: "Partly paid", tone: "warn" },
  paid: { label: "Paid in full", tone: "good" },
  return_due: { label: "Return unused cash", tone: "bad" },
  settled: { label: "Settled", tone: "good" },
};

export function ClaimBadge({ state }: { state: ClaimState }) {
  const { label, tone } = CLAIM_STATES[state];
  return <Badge tone={tone}>{label}</Badge>;
}
