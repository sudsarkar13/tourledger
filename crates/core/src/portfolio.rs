//! All loans together: what is due, what was missed, and where it is heading.
//! Used only by the Loans area; tour screens never see these figures.

use std::collections::BTreeMap;

use chrono::{Datelike, Months, NaiveDate};
use serde::Serialize;

use crate::model::{Direction, LoanStyle, parse_date};
use crate::views::{Item, LoanView};

#[derive(Debug, Clone, Serialize)]
pub struct Due {
    pub loan_id: String,
    pub name: String,
    pub number: u32,
    pub due: NaiveDate,
    pub amount: i64,
}

#[derive(Debug, Default, Serialize)]
pub struct Totals {
    pub count: u32,
    pub amount: i64,
}

#[derive(Debug, Serialize)]
pub struct CategoryTotal {
    pub category: String,
    pub count: u32,
    pub outstanding: i64,
}

#[derive(Debug, Serialize)]
pub struct LoanMonth {
    /// "2026-11"
    pub month: String,
    pub paid: i64,
    /// Instalments falling due and not yet paid.
    pub due: i64,
}

#[derive(Debug, Serialize)]
pub struct Portfolio {
    pub running: u32,
    pub closed: u32,
    /// Still to repay on money I borrowed.
    pub outstanding: i64,
    pub lent_outstanding: i64,
    /// One instalment of every running EMI loan added together.
    pub monthly_emi: i64,
    pub this_month_due: Totals,
    pub this_month_paid: Totals,
    pub overdue: Vec<Due>,
    pub overdue_total: i64,
    /// Unpaid instalments in the next 45 days.
    pub upcoming: Vec<Due>,
    pub by_category: Vec<CategoryTotal>,
    /// Over every borrowed loan, running or closed.
    pub borrowed_total: i64,
    pub paid_total: i64,
    pub interest_paid: i64,
    pub interest_remaining: i64,
    pub debt_free_on: Option<NaiveDate>,
    /// Instalments as a share of monthly income; none without an income.
    pub emi_share_percent: Option<i64>,
    pub left_after_emi: Option<i64>,
    /// Twelve months back to eleven ahead.
    pub months: Vec<LoanMonth>,
    /// The running loan charging the most, when there is more than one.
    pub highest_rate: Option<(String, String, u32)>,
}

fn key(d: NaiveDate) -> String {
    format!("{:04}-{:02}", d.year(), d.month())
}

pub fn build(
    loans: &[LoanView],
    payments: &[Item<crate::model::Payment>],
    income: i64,
    today: NaiveDate,
) -> Portfolio {
    let this_month = today.with_day(1).unwrap_or(today);
    let mut months: BTreeMap<String, LoanMonth> = (-12i32..12)
        .filter_map(|offset| {
            let step = Months::new(offset.unsigned_abs());
            if offset < 0 {
                this_month.checked_sub_months(step)
            } else {
                this_month.checked_add_months(step)
            }
        })
        .map(|d| {
            let month = key(d);
            (
                month.clone(),
                LoanMonth {
                    month,
                    paid: 0,
                    due: 0,
                },
            )
        })
        .collect();

    let mut p = Portfolio {
        running: 0,
        closed: 0,
        outstanding: 0,
        lent_outstanding: 0,
        monthly_emi: 0,
        this_month_due: Totals::default(),
        this_month_paid: Totals::default(),
        overdue: Vec::new(),
        overdue_total: 0,
        upcoming: Vec::new(),
        by_category: Vec::new(),
        borrowed_total: 0,
        paid_total: 0,
        interest_paid: 0,
        interest_remaining: 0,
        debt_free_on: None,
        emi_share_percent: None,
        left_after_emi: None,
        months: Vec::new(),
        highest_rate: None,
    };
    let mut categories: BTreeMap<String, (u32, i64)> = BTreeMap::new();
    let mut rates: Vec<(String, String, u32)> = Vec::new();

    for view in loans {
        let loan = &view.data;
        let status = &view.status;
        if loan.direction == Direction::Lent {
            p.lent_outstanding += status.outstanding;
            continue;
        }
        // Repaid in full counts as closed even if never marked so.
        let running = !loan.closed && status.outstanding > 0;
        if running {
            p.running += 1;
        } else {
            p.closed += 1;
        }
        p.borrowed_total += loan.principal;
        p.paid_total += status.paid_all;
        p.interest_paid += status.interest_paid;

        // Instalments marked as paid before tracking have no payment record.
        for row in status
            .schedule
            .iter()
            // One with a payment record is counted from the record.
            .filter(|r| r.paid && r.number <= loan.paid_before && r.paid_on.is_empty())
        {
            if let Some(m) = months.get_mut(&key(row.due)) {
                m.paid += row.amount;
            }
        }

        // This month counts even for a loan whose last instalment it was.
        if loan.style == LoanStyle::Emi {
            for row in &status.schedule {
                if key(row.due) != key(today) {
                    continue;
                }
                if row.paid {
                    p.this_month_paid.count += 1;
                    p.this_month_paid.amount += row.amount;
                } else if running {
                    p.this_month_due.count += 1;
                    p.this_month_due.amount += row.amount;
                }
            }
        }

        if !running {
            continue;
        }
        p.outstanding += status.outstanding;
        p.interest_remaining += status.interest_remaining;
        let category = if loan.category.is_empty() {
            "other"
        } else {
            &loan.category
        };
        let entry = categories.entry(category.to_owned()).or_default();
        entry.0 += 1;
        entry.1 += status.outstanding;
        if loan.rate_bps > 0 {
            rates.push((view.id.clone(), loan.name.clone(), loan.rate_bps));
        }
        if loan.style != LoanStyle::Emi {
            continue;
        }
        p.monthly_emi += status.next.as_ref().map_or(0, |n| n.amount);
        p.debt_free_on = p.debt_free_on.max(status.ends_on);
        for row in &status.schedule {
            if row.paid {
                continue;
            }
            if let Some(m) = months.get_mut(&key(row.due)) {
                m.due += row.amount;
            }
            let due = Due {
                loan_id: view.id.clone(),
                name: loan.name.clone(),
                number: row.number,
                due: row.due,
                amount: row.amount,
            };
            if row.due < today {
                p.overdue_total += row.amount;
                p.overdue.push(due);
            } else if (row.due - today).num_days() <= 45 {
                p.upcoming.push(due);
            }
        }
    }

    for payment in payments {
        let borrowed = loans
            .iter()
            .any(|l| l.id == payment.data.loan_id && l.data.direction == Direction::Borrowed);
        if borrowed
            && let Ok(date) = parse_date("date", &payment.data.date)
            && let Some(m) = months.get_mut(&key(date))
        {
            m.paid += payment.data.amount;
        }
    }

    p.overdue.sort_by_key(|d| d.due);
    p.upcoming.sort_by_key(|d| d.due);
    p.by_category = categories
        .into_iter()
        .map(|(category, (count, outstanding))| CategoryTotal {
            category,
            count,
            outstanding,
        })
        .collect();
    p.by_category.sort_by_key(|c| -c.outstanding);
    if income > 0 {
        p.emi_share_percent = Some(p.monthly_emi.saturating_mul(100) / income);
        p.left_after_emi = Some(income - p.monthly_emi);
    }
    if p.running > 1 {
        p.highest_rate = rates.into_iter().max_by_key(|r| r.2);
    }
    p.months = months.into_values().collect();
    p
}
