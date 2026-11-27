//! EMI schedules (monthly reducing balance) and what is still owed.
//!
//! Lenders differ on rounding and broken-period interest, so figures are
//! estimates; `Loan::emi` lets the lender's own instalment be used.

use std::collections::BTreeMap;

use chrono::{Months, NaiveDate};
use serde::Serialize;

use crate::error::{Error, Result};
use crate::model::{Loan, LoanStyle, Payment, PaymentKind, parse_date};

pub fn emi_for(principal: i64, rate_bps: u32, months: u32) -> i64 {
    if months == 0 {
        return 0;
    }
    if rate_bps == 0 {
        // Round up so the last instalment is the smaller one.
        return (principal + i64::from(months) - 1) / i64::from(months);
    }
    let r = f64::from(rate_bps) / 10_000.0 / 12.0;
    let growth = (1.0 + r).powi(months as i32);
    (principal as f64 * r * growth / (growth - 1.0)).round() as i64
}

/// One month's interest, rounded half up, in integers.
fn interest(balance: i64, rate_bps: u32) -> i64 {
    let product = i128::from(balance) * i128::from(rate_bps);
    ((product + 60_000) / 120_000) as i64
}

fn instalment_amount(loan: &Loan) -> i64 {
    if loan.emi > 0 {
        loan.emi
    } else {
        emi_for(loan.principal, loan.rate_bps, loan.tenure_months)
    }
}

pub fn check_emi(loan: &Loan) -> Result<()> {
    if loan.tenure_months > 1 && instalment_amount(loan) <= interest(loan.principal, loan.rate_bps)
    {
        return Err(Error::invalid(
            "emi",
            "This instalment does not cover the interest.",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct Instalment {
    pub number: u32,
    pub due: NaiveDate,
    pub amount: i64,
    pub principal: i64,
    pub interest: i64,
    pub balance: i64,
    pub paid: bool,
    /// The date recorded with the payment; empty if paid before tracking.
    pub paid_on: String,
}

#[derive(Debug, Default, Serialize)]
pub struct Status {
    pub emi: i64,
    pub outstanding: i64,
    pub paid_total: i64,
    pub paid_count: u32,
    pub total_count: u32,
    pub total_interest: i64,
    pub overdue_count: u32,
    /// Everything paid, instalments from before tracking included.
    pub paid_all: i64,
    pub interest_paid: i64,
    pub interest_remaining: i64,
    pub remaining_amount: i64,
    pub ends_on: Option<NaiveDate>,
    pub next: Option<Instalment>,
    pub schedule: Vec<Instalment>,
}

fn schedule(loan: &Loan, payments: &[&Payment]) -> Vec<Instalment> {
    let Ok(first_due) = parse_date("first_due_date", &loan.first_due_date) else {
        return Vec::new();
    };
    let mut prepayments: Vec<(NaiveDate, i64)> = payments
        .iter()
        .filter(|p| p.kind == PaymentKind::Prepayment)
        .filter_map(|p| parse_date("date", &p.date).ok().map(|d| (d, p.amount)))
        .collect();
    prepayments.sort_unstable();
    let mut prepayments = prepayments.into_iter().peekable();

    let emi = instalment_amount(loan);
    let mut balance = loan.principal;
    let mut out = Vec::new();
    for number in 1..=loan.tenure_months {
        // Counted from the first due date so a 31st does not drift to the 28th.
        let Some(due) = first_due.checked_add_months(Months::new(number - 1)) else {
            break;
        };
        while let Some((_, amount)) = prepayments.next_if(|(date, _)| *date <= due) {
            balance = (balance - amount).max(0);
        }
        if balance == 0 {
            break;
        }
        let interest = interest(balance, loan.rate_bps);
        let mut principal = (emi - interest).clamp(0, balance);
        if number == loan.tenure_months {
            principal = balance;
        }
        balance -= principal;
        out.push(Instalment {
            number,
            due,
            amount: principal + interest,
            principal,
            interest,
            balance,
            paid: false,
            paid_on: String::new(),
        });
    }
    out
}

pub fn status<'a>(
    loan: &Loan,
    payments: impl IntoIterator<Item = &'a Payment>,
    today: NaiveDate,
) -> Status {
    let payments: Vec<&Payment> = payments.into_iter().collect();
    let paid_total: i64 = payments.iter().map(|p| p.amount).sum();

    if loan.style == LoanStyle::Flexible {
        return Status {
            outstanding: if loan.closed {
                0
            } else {
                (loan.principal - paid_total).max(0)
            },
            paid_total,
            paid_all: paid_total,
            remaining_amount: if loan.closed {
                0
            } else {
                (loan.principal - paid_total).max(0)
            },
            paid_count: payments.len() as u32,
            ..Default::default()
        };
    }

    let paid: BTreeMap<u32, &str> = payments
        .iter()
        .filter(|p| p.kind == PaymentKind::Instalment)
        .map(|p| (p.number, p.date.as_str()))
        .collect();
    let mut rows = schedule(loan, &payments);
    for row in &mut rows {
        row.paid = row.number <= loan.paid_before || paid.contains_key(&row.number);
        if let Some(date) = paid.get(&row.number) {
            row.paid_on = (*date).to_owned();
        }
    }
    let repaid: i64 = rows.iter().filter(|r| r.paid).map(|r| r.principal).sum();
    let prepaid: i64 = payments
        .iter()
        .filter(|p| p.kind == PaymentKind::Prepayment)
        .map(|p| p.amount)
        .sum();
    let outstanding = if loan.closed {
        0
    } else {
        (loan.principal - repaid - prepaid).max(0)
    };
    let open = !loan.closed && outstanding > 0;
    let before_tracking: i64 = rows
        .iter()
        .filter(|r| r.number <= loan.paid_before && !paid.contains_key(&r.number))
        .map(|r| r.amount)
        .sum();
    let unpaid = || rows.iter().filter(move |r| open && !r.paid);

    Status {
        emi: instalment_amount(loan),
        outstanding,
        paid_total,
        paid_count: rows.iter().filter(|r| r.paid).count() as u32,
        total_count: rows.len() as u32,
        total_interest: rows.iter().map(|r| r.interest).sum(),
        paid_all: paid_total + before_tracking,
        interest_paid: rows.iter().filter(|r| r.paid).map(|r| r.interest).sum(),
        interest_remaining: unpaid().map(|r| r.interest).sum(),
        remaining_amount: unpaid().map(|r| r.amount).sum(),
        ends_on: rows.last().map(|r| r.due),
        overdue_count: rows
            .iter()
            .filter(|r| open && !r.paid && r.due < today)
            .count() as u32,
        next: rows.iter().find(|r| open && !r.paid).cloned(),
        schedule: rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    fn loan() -> Loan {
        Loan {
            principal: 10_000_000, // 1,00,000.00
            rate_bps: 1200,
            tenure_months: 12,
            first_due_date: "2026-01-31".into(),
            ..Default::default()
        }
    }

    #[test]
    fn matches_the_standard_emi_figure() {
        // 1 lakh, 12% a year, 12 months: 8,884.88
        assert_eq!(emi_for(10_000_000, 1200, 12), 888_488);
        assert_eq!(emi_for(1_000, 0, 3), 334);
    }

    #[test]
    fn schedule_repays_exactly_the_principal() {
        let s = status(&loan(), [], date("2026-01-01"));
        assert_eq!(s.total_count, 12);
        assert_eq!(
            s.schedule.iter().map(|r| r.principal).sum::<i64>(),
            10_000_000
        );
        assert_eq!(s.schedule.last().unwrap().balance, 0);
        assert_eq!(s.schedule[1].due, date("2026-02-28"));
        assert_eq!(s.schedule[2].due, date("2026-03-31"));
    }

    #[test]
    fn counts_paid_and_overdue() {
        let mut l = loan();
        l.paid_before = 2;
        let paid = Payment {
            kind: PaymentKind::Instalment,
            number: 3,
            amount: 888_488,
            ..Default::default()
        };
        let s = status(&l, [&paid], date("2026-05-15"));
        assert_eq!((s.paid_count, s.overdue_count), (3, 1));
        assert_eq!(s.next.unwrap().number, 4);
    }

    #[test]
    fn prepayment_shortens_the_loan() {
        let prepay = Payment {
            kind: PaymentKind::Prepayment,
            date: "2026-02-01".into(),
            amount: 5_000_000,
            ..Default::default()
        };
        let s = status(&loan(), [&prepay], date("2026-02-02"));
        assert!(s.total_count < 12);
        assert_eq!(s.outstanding, 5_000_000);
    }
}
