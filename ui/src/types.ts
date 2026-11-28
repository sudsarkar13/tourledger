// Mirrors crates/core/src/model.rs and views.rs. Money is in paise.

export type Claim = "reimbursable" | "personal";
export type TourKind = "company" | "individual";
export type TourStatus = "planned" | "active" | "completed" | "submitted" | "settled";
export type TransferKind =
  | "advance"
  | "advance_returned"
  | "reimbursement"
  | "received_from"
  | "paid_to"
  | "allowance";
export type Direction = "borrowed" | "lent";
export type LoanStyle = "emi" | "flexible";
export type PaymentKind = "instalment" | "prepayment" | "repayment";

export interface Profile {
  name: string;
  company_id: string;
  home_city: string;
  phone: string;
  email: string;
  upi_id: string;
  monthly_income: number;
}

export interface Company {
  id: string;
  name: string;
  designation: string;
  employee_id: string;
  department: string;
  joined_on: string;
  left_on: string;
  address: string;
  gstin: string;
  contact: string;
  notes: string;
}

export interface Person {
  id: string;
  name: string;
  relation: string;
  company_id: string;
  phone: string;
  notes: string;
}

export interface QuickEntry {
  id: string;
  label: string;
  category: string;
  from: string;
  to: string;
  mode: string;
  amount: number;
  claim: Claim;
  /** The amount covers the way back too. */
  round_trip?: boolean;
}

export interface Summary {
  total_cost: number;
  paid_by_me: number;
  company_paid: number;
  colleague_paid: number;
  my_cost: number;
  claimable: number;
  rejected: number;
  advance: number;
  advance_returned: number;
  reimbursed: number;
  allowance: number;
  net: number;
  balances: Record<string, number>;
  by_category: Record<string, number>;
}

export interface Tally {
  count: number;
  total: number;
}

/** How my payments on a tour were made, and which have a picture. */
export interface Proof {
  modes: { name: string; all: Tally; pictured: Tally }[];
  all: Tally;
  pictured: Tally;
  /** Expenses still without a picture. */
  missing: string[];
}

export interface Tour {
  id: string;
  name: string;
  kind: TourKind;
  purpose: string;
  event_type: string;
  event_name: string;
  organiser: string;
  venue: string;
  city: string;
  company_id: string;
  start_date: string;
  end_date: string;
  participants: string[];
  budget: number;
  status: TourStatus;
  submitted_on: string;
  notes: string;
  local_only: boolean;
  /** Expense ids in the order chosen for the report; empty means by date. */
  order: string[];
  summary: Summary;
  claim_state: ClaimState;
  proof: Proof;
  /** Its entries under the report's headings, in the order listed there. */
  sections: { name: string; ids: string[] }[];
}

export interface Expense {
  id: string;
  tour_id: string;
  date: string;
  category: string;
  description: string;
  from: string;
  to: string;
  mode: string;
  amount: number;
  currency: string;
  rate_ppm: number;
  paid_by: string;
  claim: Claim;
  rejected: boolean;
  payment_method: string;
  bill_no: string;
  split_with: string[];
  local_only: boolean;
  /** The one amount covers the way back too. Absent when it does not. */
  round_trip?: boolean;
  /** `amount` in rupee paise. */
  home: number;
}

export interface Transfer {
  id: string;
  tour_id: string;
  date: string;
  kind: TransferKind;
  person_id: string;
  amount: number;
  note: string;
}

export type ClaimState =
  | ""
  | "none"
  | "not_sent"
  | "sent"
  | "part_paid"
  | "paid"
  | "return_due"
  | "settled";

export interface Instalment {
  number: number;
  due: string;
  amount: number;
  principal: number;
  interest: number;
  balance: number;
  paid: boolean;
  paid_on: string;
}

export interface Loan {
  id: string;
  name: string;
  category: string;
  reference: string;
  counterparty: string;
  direction: Direction;
  style: LoanStyle;
  principal: number;
  rate_bps: number;
  tenure_months: number;
  start_date: string;
  first_due_date: string;
  emi: number;
  paid_before: number;
  notes: string;
  closed: boolean;
  closed_on: string;
  local_only: boolean;
  status: {
    emi: number;
    outstanding: number;
    paid_total: number;
    paid_count: number;
    total_count: number;
    total_interest: number;
    overdue_count: number;
    paid_all: number;
    interest_paid: number;
    interest_remaining: number;
    remaining_amount: number;
    ends_on: string | null;
    next: Instalment | null;
    schedule: Instalment[];
  };
}

export interface Payment {
  id: string;
  loan_id: string;
  date: string;
  kind: PaymentKind;
  number: number;
  amount: number;
  note: string;
}

export interface Alert {
  kind: string;
  level: "danger" | "warn" | "info";
  subject: string;
  amount: number;
  days: number;
  percent: number;
  ref_id: string;
}

export interface Month {
  month: string;
  spend: number;
  by_category: Record<string, number>;
}

export interface Insights {
  months: Month[];
  history_months: number;
  this_month_estimate: number | null;
  next_month: { spend: number; by_category: Record<string, number> } | null;
  income: number;
  savings_estimate: number | null;
  budgets: { category: string; budget: number; spent: number }[];
  awaiting_reimbursement: number;
  alerts: Alert[];
}

export interface Attachment {
  id: string;
  expense_id: string;
  hash: string;
  width: number;
  height: number;
  bytes: number;
  full_page: boolean;
  /** False until the picture has arrived from another device. */
  present: boolean;
}

export type PreviewMark =
  | { kind: "text"; x: number; y: number; text: string; size: number; bold: boolean; grey: number; width: number }
  | { kind: "box"; x: number; y: number; w: number; h: number; grey: number }
  | { kind: "image"; x: number; y: number; w: number; h: number; hash: string };

export interface Preview {
  width: number;
  height: number;
  pages: PreviewMark[][];
  images: Record<string, string>;
}

export interface Snapshot {
  revision: number;
  profile: Profile;
  companies: Company[];
  people: Person[];
  quick: QuickEntry[];
  tours: Tour[];
  expenses: Expense[];
  transfers: Transfer[];
  loans: Loan[];
  payments: Payment[];
  budgets: { id: string; monthly: number }[];
  attachments: Attachment[];
  balances: Record<string, number>;
  orphans: number;
  insights: Insights;
  portfolio: Portfolio;
}

export interface Due {
  loan_id: string;
  name: string;
  number: number;
  due: string;
  amount: number;
}

export interface Portfolio {
  running: number;
  closed: number;
  outstanding: number;
  lent_outstanding: number;
  monthly_emi: number;
  this_month_due: { count: number; amount: number };
  this_month_paid: { count: number; amount: number };
  overdue: Due[];
  overdue_total: number;
  upcoming: Due[];
  by_category: { category: string; count: number; outstanding: number }[];
  borrowed_total: number;
  paid_total: number;
  interest_paid: number;
  interest_remaining: number;
  debt_free_on: string | null;
  emi_share_percent: number | null;
  left_after_emi: number | null;
  months: { month: string; paid: number; due: number }[];
  highest_rate: [string, string, number] | null;
}

export interface Draft {
  line: string;
  kind: "expense" | "advance" | "advance_returned" | "reimbursement" | "allowance";
  amount: number;
  date: string;
  date_assumed: boolean;
  category: string;
  description: string;
  from: string;
  to: string;
  mode: string;
  paid_by: string;
  claim: Claim;
  payment_method: string;
  split_with: string[];
  bill_no: string;
  warnings: string[];
  problem: string;
}

export interface ReportLine {
  id: string;
  date: string;
  category: string;
  particulars: string;
  people: string;
  paid_by: string;
  mode: string;
  bill_no: string;
  amount: number;
  bills: number[];
}

export interface Report {
  detail: boolean;
  is_claim: boolean;
  title: string;
  company: string;
  tour: string;
  claimant: [string, string][];
  details: [string, string][];
  groups: { name: string; lines: ReportLine[]; total: number; company_lines: ReportLine[]; company_total: number }[];
  total: number;
  company_total: number;
  left_out_count: number;
  left_out_total: number;
  money: [string, string, number][];
  bills: { number: number; hash: string; label: string; present: boolean }[];
  stages: { name: string; total: number }[];
  check: { id: string; particulars: string; amount: number; filed: string; suggested: string }[];
  without_bill: number;
  proof: boolean;
  proof_summary: Proof;
  unproved: ReportLine[];
  unproved_total: number;
  statement: string;
  declared_by: string;
  declared_on: string;
  allowance: number;
  by_person: [string, number][];
  advance: number;
  advance_returned: number;
  reimbursed: number;
  net: number;
  net_label: string;
  net_words: string;
}

export interface SyncPeer {
  id: string;
  name: string;
  last_sync_ms: number;
  error: string | null;
  /** The error is only that the other device is not there right now. */
  away: boolean;
  /** It has more bill pictures to get from here than one exchange carries. */
  more_pictures: boolean;
  busy: boolean;
  phase: "connecting" | "comparing" | "pictures" | null;
  /** What the last exchange that changed something moved. */
  moved: { at_ms: number; received: number; sent: number; pictures_received: number; pictures_sent: number } | null;
}

export interface SyncStatus {
  enabled: boolean;
  online: boolean;
  name: string;
  peers: SyncPeer[];
  pictures_waiting: number;
  /** The last device that ended its link with this one. */
  unlinked_by: { name: string; at_ms: number } | null;
  /** On the reply to unlinking: the other device has heard. */
  told?: boolean;
  pairing: { code: string; expires_ms: number } | null;
  error: string | null;
  revision: number;
}

export interface ExportedFile {
  name: string;
  mime: string;
  content: string;
  base64: boolean;
}

export type Kind =
  | "profile"
  | "company"
  | "person"
  | "quick"
  | "tour"
  | "expense"
  | "transfer"
  | "loan"
  | "payment"
  | "budget"
  | "attachment";
