import {
  BedDouble,
  Bus,
  CarTaxiFront,
  Fuel,
  Pill,
  Plane,
  ReceiptText,
  ShoppingBag,
  Smartphone,
  Ticket,
  TrainFront,
  Utensils,
  type LucideIcon,
} from "lucide-react";
import type { Alert, Snapshot, TourStatus, TransferKind } from "./types";
import { money, plural } from "./format";

export const CATEGORIES: Record<string, { name: string; icon: LucideIcon }> = {
  local: { name: "Local transport", icon: CarTaxiFront },
  train: { name: "Train", icon: TrainFront },
  flight: { name: "Flight", icon: Plane },
  bus: { name: "Bus", icon: Bus },
  food: { name: "Food and refreshments", icon: Utensils },
  lodging: { name: "Lodging", icon: BedDouble },
  event: { name: "Event and tickets", icon: Ticket },
  fuel: { name: "Fuel, parking, toll", icon: Fuel },
  phone: { name: "Phone and internet", icon: Smartphone },
  shopping: { name: "Shopping", icon: ShoppingBag },
  medical: { name: "Medical", icon: Pill },
  other: { name: "Other", icon: ReceiptText },
};

export const categoryName = (id: string) =>
  id === "total" ? "All spending" : (CATEGORIES[id]?.name ?? id);

export const categoryOptions = Object.entries(CATEGORIES).map(([value, c]) => ({
  value,
  label: c.name,
}));

export const LOAN_CATEGORIES: Record<string, string> = {
  home: "Home",
  vehicle: "Vehicle",
  personal: "Personal",
  education: "Education",
  gold: "Gold",
  business: "Business",
  consumer: "Gadget or appliance",
  informal: "From or to a person",
  other: "Other",
};

export const loanCategory = (id: string) => LOAN_CATEGORIES[id || "other"] ?? id;

export const loanCategoryOptions = Object.entries(LOAN_CATEGORIES).map(([value, label]) => ({ value, label }));

export const TOUR_STATUS: Record<TourStatus, string> = {
  planned: "Planned",
  active: "On tour",
  completed: "Completed",
  submitted: "Claim sent",
  settled: "Settled",
};

export const EVENT_TYPES = [
  "Trade fair",
  "Exhibition",
  "Conference",
  "Client meeting",
  "Training",
  "Site visit",
  "Holiday",
  "Family",
  "Other",
];

export const TRANSFER_KINDS: Record<TransferKind, string> = {
  advance: "Advance from company",
  advance_returned: "Advance returned to company",
  reimbursement: "Reimbursement received",
  received_from: "Received from a person",
  paid_to: "Paid to a person",
  allowance: "Bonus or allowance (mine to keep)",
};

export const PURPOSES: Record<string, string> = {
  business: "Business (work or event)",
  wellness: "Wellness trip",
  reward: "Reward or bonus trip",
};

/** On a work trip, what I pay is claimed unless said otherwise. */
export const isWorkTrip = (t?: { kind: string; purpose: string }) =>
  t?.kind === "company" && (t.purpose === "" || t.purpose === "business");

export function personName(data: Snapshot, id: string): string {
  if (id === "me") return "Me";
  if (id === "company") return "Company";
  return data.people.find((p) => p.id === id)?.name ?? "Removed person";
}

export function alertText(a: Alert): { title: string; detail: string } {
  switch (a.kind) {
    case "claim_not_sent":
      return { title: `Send the claim for ${a.subject}`, detail: `${money(a.amount)} to get back, tour ended ${plural(a.days, "day")} ago` };
    case "claim_waiting":
      return { title: `${a.subject}: claim not paid yet`, detail: `${money(a.amount)} waiting ${plural(a.days, "day")}` };
    case "advance_unreturned":
      return { title: `${a.subject}: advance to return`, detail: `${money(a.amount)} of the advance was not spent` };
    case "orphans":
      return {
        title: `${plural(a.amount, "entry", "entries")} lost their tour`,
        detail: "The tour was deleted on another device. Open each expense to move or delete it.",
      };
    case "tour_soon":
      return { title: `${a.subject} ${a.days === 0 ? "starts today" : `starts in ${plural(a.days, "day")}`}`, detail: "Check the advance and tickets" };
    case "budget_over":
      return { title: `${categoryName(a.subject)} is over budget`, detail: `${money(a.amount)} over this month` };
    case "budget_near":
      return { title: `${categoryName(a.subject)} budget nearly used`, detail: `${a.percent}% used, ${money(a.amount)} left` };
    case "category_up":
      return { title: `${categoryName(a.subject)} is higher than usual`, detail: `${money(a.amount)} (${a.percent}%) above your recent average` };
    case "last_year":
      return { title: `This month last year: ${money(a.amount)}`, detail: `Mostly ${categoryName(a.subject).toLowerCase()}` };
    default:
      return { title: a.subject, detail: money(a.amount) };
  }
}
