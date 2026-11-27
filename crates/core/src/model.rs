//! The kinds of record, their fields, and what makes each one valid.

use chrono::NaiveDate;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::money::{self, MAX_AMOUNT, RATE_SCALE};

pub const PROFILE: &str = "profile";
pub const COMPANY: &str = "company";
pub const PERSON: &str = "person";
pub const QUICK: &str = "quick";
pub const TOUR: &str = "tour";
pub const EXPENSE: &str = "expense";
pub const TRANSFER: &str = "transfer";
pub const LOAN: &str = "loan";
pub const PAYMENT: &str = "payment";
pub const BUDGET: &str = "budget";
pub const ATTACHMENT: &str = "attachment";

pub const KINDS: [&str; 11] = [
    PROFILE, COMPANY, PERSON, QUICK, TOUR, EXPENSE, TRANSFER, LOAN, PAYMENT, BUDGET, ATTACHMENT,
];

/// A bill or screenshot for one expense. The picture itself is a file named by
/// `hash` (see `blobs`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Attachment {
    pub expense_id: String,
    pub hash: String,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    /// Give it a whole page in the report instead of a quarter.
    pub full_page: bool,
}

pub const MAX_BILLS_PER_EXPENSE: usize = 10;

/// The id that stands for the owner of the app in splits and payers.
pub const ME: &str = "me";

pub const CATEGORIES: [&str; 13] = [
    "local", "metro", "train", "flight", "bus", "food", "lodging", "event", "fuel", "phone",
    "shopping", "medical", "other",
];

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub name: String,
    /// The company worked for now.
    pub company_id: String,
    pub home_city: String,
    pub phone: String,
    pub email: String,
    pub upi_id: String,
    pub monthly_income: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Company {
    pub name: String,
    /// My role there.
    pub designation: String,
    pub employee_id: String,
    pub department: String,
    pub joined_on: String,
    pub left_on: String,
    pub address: String,
    pub gstin: String,
    pub contact: String,
    pub notes: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Person {
    pub name: String,
    pub relation: String,
    pub company_id: String,
    pub phone: String,
    pub notes: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Claim {
    /// The company pays me back.
    Reimbursable,
    #[default]
    Personal,
}

/// A journey or purchase repeated often enough to save.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct QuickEntry {
    pub label: String,
    pub category: String,
    pub from: String,
    pub to: String,
    pub mode: String,
    pub amount: i64,
    pub claim: Claim,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TourKind {
    #[default]
    Company,
    Individual,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TourStatus {
    #[default]
    Planned,
    Active,
    Completed,
    Submitted,
    Settled,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Tour {
    pub name: String,
    pub kind: TourKind,
    /// Why the company sent me: one of `PURPOSES`; empty means business.
    pub purpose: String,
    pub event_type: String,
    pub event_name: String,
    pub organiser: String,
    pub venue: String,
    pub city: String,
    pub company_id: String,
    pub start_date: String,
    pub end_date: String,
    pub participants: Vec<String>,
    pub budget: i64,
    pub status: TourStatus,
    pub submitted_on: String,
    pub notes: String,
    pub local_only: bool,
}

pub const PURPOSES: [&str; 3] = ["business", "wellness", "reward"];

impl Tour {
    /// A company tour for work: spending is claimed unless said otherwise.
    pub fn is_work(&self) -> bool {
        self.kind == TourKind::Company && matches!(self.purpose.as_str(), "" | "business")
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Expense {
    pub tour_id: String,
    pub date: String,
    pub category: String,
    pub description: String,
    pub from: String,
    pub to: String,
    pub mode: String,
    /// In `currency` minor units.
    pub amount: i64,
    /// Empty means the home currency (INR).
    pub currency: String,
    pub rate_ppm: i64,
    /// `me`, `company`, or a person id.
    pub paid_by: String,
    pub claim: Claim,
    pub rejected: bool,
    pub payment_method: String,
    pub bill_no: String,
    /// Who shares the cost: `me` and/or person ids. Empty means not shared.
    pub split_with: Vec<String>,
    pub local_only: bool,
}

impl Expense {
    pub fn home_amount(&self) -> i64 {
        if self.currency.is_empty() || self.currency == "INR" {
            self.amount
        } else {
            money::convert(self.amount, self.rate_ppm)
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferKind {
    #[default]
    Advance,
    AdvanceReturned,
    Reimbursement,
    /// A colleague paid me back.
    ReceivedFrom,
    /// I paid a colleague back.
    PaidTo,
    /// A bonus or allowance from the company that is mine to keep.
    Allowance,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Transfer {
    pub tour_id: String,
    pub date: String,
    pub kind: TransferKind,
    pub person_id: String,
    pub amount: i64,
    pub note: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    #[default]
    Borrowed,
    Lent,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoanStyle {
    /// Fixed monthly instalments.
    #[default]
    Emi,
    /// Repaid in any amounts, no interest tracked.
    Flexible,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Loan {
    pub name: String,
    /// One of `LOAN_CATEGORIES`; empty counts as `other`.
    pub category: String,
    /// Lender's account or reference number.
    pub reference: String,
    pub counterparty: String,
    pub direction: Direction,
    pub style: LoanStyle,
    pub principal: i64,
    /// Yearly interest in basis points: 1050 is 10.50%.
    pub rate_bps: u32,
    pub tenure_months: u32,
    pub start_date: String,
    pub first_due_date: String,
    /// The lender's figure, when it differs from the calculated one. 0 = calculate.
    pub emi: i64,
    /// Instalments already paid before the loan was entered here.
    pub paid_before: u32,
    pub notes: String,
    pub closed: bool,
    pub closed_on: String,
    pub local_only: bool,
}

pub const LOAN_CATEGORIES: [&str; 9] = [
    "home",
    "vehicle",
    "personal",
    "education",
    "gold",
    "business",
    "consumer",
    "informal",
    "other",
];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentKind {
    #[default]
    Instalment,
    Prepayment,
    Repayment,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Payment {
    pub loan_id: String,
    pub date: String,
    pub kind: PaymentKind,
    /// Instalment number, starting at 1.
    pub number: u32,
    pub amount: i64,
    pub note: String,
}

/// Record id is the category, or `total`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Budget {
    pub monthly: i64,
}

pub fn parse_date(field: &'static str, text: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .ok()
        .filter(|d| (1990..=2100).contains(&chrono::Datelike::year(d)))
        .ok_or_else(|| Error::invalid(field, "Pick a date."))
}

fn optional_date(field: &'static str, text: &str) -> Result<Option<NaiveDate>> {
    if text.is_empty() {
        Ok(None)
    } else {
        parse_date(field, text).map(Some)
    }
}

fn required(field: &'static str, value: &str, message: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(Error::invalid(field, message));
    }
    Ok(())
}

fn not_negative(field: &'static str, value: i64) -> Result<()> {
    if !(0..=MAX_AMOUNT).contains(&value) {
        return Err(Error::invalid(field, "Enter an amount of zero or more."));
    }
    Ok(())
}

/// One of `CATEGORIES`, or a short id a newer version added. Refusing an unknown
/// one would stop all syncing with a device that updated first. It is counted
/// as miscellaneous.
fn category(field: &'static str, value: &str) -> Result<()> {
    let from_a_newer_version = (1..=32).contains(&value.len())
        && value.bytes().all(|b| b.is_ascii_lowercase() || b == b'_');
    if !CATEGORIES.contains(&value) && !from_a_newer_version {
        return Err(Error::invalid(field, "Pick a category."));
    }
    Ok(())
}

fn typed<T: DeserializeOwned + Serialize>(
    body: &Value,
    check: impl FnOnce(&T) -> Result<()>,
) -> Result<Value> {
    let value: T = serde_json::from_value(body.clone())
        .map_err(|e| Error::msg(format!("Unreadable entry: {e}")))?;
    check(&value)?;
    Ok(serde_json::to_value(value)?)
}

/// Checks a record body and returns it in its canonical shape. Used for what the
/// user types and for what arrives from another device or file.
pub fn validate(kind: &str, id: &str, body: &Value) -> Result<Value> {
    if id.is_empty() || id.len() > 64 {
        return Err(Error::msg("Bad item id."));
    }
    if serde_json::to_vec(body)?.len() > 64 * 1024 {
        return Err(Error::msg("That entry is too long."));
    }
    match kind {
        PROFILE => typed::<Profile>(body, |p| not_negative("monthly_income", p.monthly_income)),
        COMPANY => typed::<Company>(body, |c| {
            required("name", &c.name, "Enter the company name.")?;
            let joined = optional_date("joined_on", &c.joined_on)?;
            let left = optional_date("left_on", &c.left_on)?;
            if let (Some(joined), Some(left)) = (joined, left)
                && left < joined
            {
                return Err(Error::invalid(
                    "left_on",
                    "The leaving date is before the joining date.",
                ));
            }
            Ok(())
        }),
        PERSON => typed::<Person>(body, |p| required("name", &p.name, "Enter a name.")),
        QUICK => typed::<QuickEntry>(body, |q| {
            required("label", &q.label, "Give this entry a name.")?;
            category("category", &q.category)?;
            money::check_amount("amount", q.amount)
        }),
        TOUR => typed::<Tour>(body, |t| {
            required("name", &t.name, "Enter a name for the tour.")?;
            let start = parse_date("start_date", &t.start_date)?;
            let end = parse_date("end_date", &t.end_date)?;
            if end < start {
                return Err(Error::invalid(
                    "end_date",
                    "The end date is before the start date.",
                ));
            }
            if !t.purpose.is_empty() && !PURPOSES.contains(&t.purpose.as_str()) {
                return Err(Error::invalid("purpose", "Pick the purpose."));
            }
            not_negative("budget", t.budget)
        }),
        EXPENSE => typed::<Expense>(body, |e| {
            parse_date("date", &e.date)?;
            category("category", &e.category)?;
            money::check_amount("amount", e.amount)?;
            required("paid_by", &e.paid_by, "Say who paid.")?;
            if !e.currency.is_empty() && e.currency != "INR" {
                if e.currency.len() != 3 || !e.currency.chars().all(|c| c.is_ascii_uppercase()) {
                    return Err(Error::invalid("currency", "Use a code like USD."));
                }
                if e.rate_ppm <= 0 || e.rate_ppm > 100_000 * RATE_SCALE {
                    return Err(Error::invalid("rate_ppm", "Enter the exchange rate."));
                }
                money::check_amount("amount", e.home_amount())?;
            }
            if e.split_with.len() > 50 {
                return Err(Error::invalid("split_with", "Too many people."));
            }
            Ok(())
        }),
        TRANSFER => typed::<Transfer>(body, |t| {
            parse_date("date", &t.date)?;
            money::check_amount("amount", t.amount)?;
            // Settling with a person can be outside any tour; company money cannot.
            if matches!(t.kind, TransferKind::ReceivedFrom | TransferKind::PaidTo) {
                required("person_id", &t.person_id, "Pick the person.")?;
            } else {
                required("tour_id", &t.tour_id, "Pick a tour.")?;
            }
            Ok(())
        }),
        LOAN => typed::<Loan>(body, |l| {
            required("name", &l.name, "Give the loan a name.")?;
            if !l.category.is_empty() && !LOAN_CATEGORIES.contains(&l.category.as_str()) {
                return Err(Error::invalid("category", "Pick a kind of loan."));
            }
            optional_date("closed_on", &l.closed_on)?;
            money::check_amount("principal", l.principal)?;
            parse_date("start_date", &l.start_date)?;
            if l.style == LoanStyle::Emi {
                if !(1..=600).contains(&l.tenure_months) {
                    return Err(Error::invalid("tenure_months", "Enter 1 to 600 months."));
                }
                if l.rate_bps > 10_000 {
                    return Err(Error::invalid("rate_bps", "Enter a rate up to 100%."));
                }
                parse_date("first_due_date", &l.first_due_date)?;
                not_negative("emi", l.emi)?;
                if l.paid_before > l.tenure_months {
                    return Err(Error::invalid(
                        "paid_before",
                        "That is more than the number of months.",
                    ));
                }
                crate::loans::check_emi(l)?;
            }
            Ok(())
        }),
        PAYMENT => typed::<Payment>(body, |p| {
            required("loan_id", &p.loan_id, "Pick a loan.")?;
            parse_date("date", &p.date)?;
            money::check_amount("amount", p.amount)
        }),
        ATTACHMENT => typed::<Attachment>(body, |a| {
            required("expense_id", &a.expense_id, "Pick the expense.")?;
            if !crate::blobs::is_hash(&a.hash) || a.width == 0 || a.height == 0 {
                return Err(Error::invalid("bill", "That picture could not be read."));
            }
            Ok(())
        }),
        BUDGET => {
            if id != "total" {
                category("category", id)?;
            }
            typed::<Budget>(body, |b| money::check_amount("monthly", b.monthly))
        }
        _ => Err(Error::msg("Unknown kind of item.")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_bad_input_with_the_field_to_fix() {
        let error = validate(
            TOUR,
            "t",
            &json!({"name": "IITF", "start_date": "2026-11-20", "end_date": "2026-11-14"}),
        )
        .unwrap_err();
        assert_eq!(error.field(), Some("end_date"));
    }

    #[test]
    fn foreign_expense_needs_a_rate() {
        let body = json!({"date": "2026-11-14", "category": "food", "amount": 100,
            "paid_by": "me", "currency": "USD"});
        assert_eq!(
            validate(EXPENSE, "e", &body).unwrap_err().field(),
            Some("rate_ppm")
        );
    }
}
