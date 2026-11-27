//! Which loan instalments to be reminded of, and on which day. When and how a
//! reminder is shown is up to the app on each device.

use chrono::{Days, NaiveDate};
use serde::Serialize;

use crate::model::{Direction, LoanStyle};
use crate::views::LoanView;

/// How far ahead to list, so a phone that is not opened for a while is still told.
const AHEAD_DAYS: u64 = 62;
/// Older misses are left to the loan screens.
const MISSED_DAYS: u64 = 30;
const LIMIT: usize = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Some days before it is due.
    Before,
    /// On the day.
    Due,
    /// The day after, if still not marked as paid.
    Missed,
}

#[derive(Debug, Clone, Serialize)]
pub struct Reminder {
    /// Stays the same for the same instalment and kind.
    pub key: String,
    pub loan_id: String,
    pub name: String,
    pub number: u32,
    pub due: NaiveDate,
    pub amount: i64,
    /// Money lent: the other person pays me.
    pub lent: bool,
    /// The day to show it.
    pub on: NaiveDate,
    pub kind: Kind,
}

/// Reminders still worth showing on `today`, earliest first. `lead_days` of 0
/// leaves out the early one; `missed` adds one for an instalment left unpaid.
pub fn upcoming(
    loans: &[LoanView],
    today: NaiveDate,
    lead_days: u32,
    missed: bool,
) -> Vec<Reminder> {
    let horizon = today + Days::new(AHEAD_DAYS);
    let oldest = today - Days::new(MISSED_DAYS);
    let mut found = Vec::new();
    for view in loans {
        let loan = &view.data;
        if loan.closed || loan.style != LoanStyle::Emi || view.status.outstanding <= 0 {
            continue;
        }
        let unpaid = view
            .status
            .schedule
            .iter()
            .filter(|row| !row.paid && row.due >= oldest && row.due <= horizon);
        for row in unpaid {
            let mut add = |kind: Kind, on: NaiveDate| {
                found.push(Reminder {
                    key: format!("{}:{}:{kind:?}", view.id, row.number),
                    loan_id: view.id.clone(),
                    name: loan.name.clone(),
                    number: row.number,
                    due: row.due,
                    amount: row.amount,
                    lent: loan.direction == Direction::Lent,
                    on,
                    kind,
                });
            };
            if lead_days > 0 && row.due > today {
                add(
                    Kind::Before,
                    row.due - Days::new(u64::from(lead_days.min(30))),
                );
            }
            if row.due >= today {
                add(Kind::Due, row.due);
            }
            if missed {
                add(Kind::Missed, row.due + Days::new(1));
            }
        }
    }
    found.sort_by(|a, b| (a.on, &a.name, a.number).cmp(&(b.on, &b.name, b.number)));
    found.truncate(LIMIT);
    found
}
