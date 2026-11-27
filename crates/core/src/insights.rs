//! Spending history, estimates and reminders for tours and daily costs. Plain
//! statistics, deliberately. Loans are kept apart, in `portfolio`.

use std::collections::BTreeMap;

use chrono::{Datelike, Months, NaiveDate};
use serde::Serialize;

use crate::model::{TourKind, TourStatus, parse_date};
use crate::settlement::my_cost;
use crate::views::Data;

#[derive(Debug, Default, Serialize)]
pub struct Month {
    /// "2026-11"
    pub month: String,
    pub spend: i64,
    pub by_category: BTreeMap<String, i64>,
}

#[derive(Debug, Serialize)]
pub struct Forecast {
    pub spend: i64,
    pub by_category: BTreeMap<String, i64>,
}

#[derive(Debug, Serialize)]
pub struct BudgetLine {
    pub category: String,
    pub budget: i64,
    pub spent: i64,
}

#[derive(Debug, Serialize)]
pub struct Alert {
    pub kind: &'static str,
    /// "danger", "warn" or "info".
    pub level: &'static str,
    pub subject: String,
    pub amount: i64,
    pub days: i64,
    pub percent: i64,
    pub ref_id: String,
}

impl Alert {
    fn new(kind: &'static str, level: &'static str) -> Self {
        Alert {
            kind,
            level,
            subject: String::new(),
            amount: 0,
            days: 0,
            percent: 0,
            ref_id: String::new(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Insights {
    /// Oldest first, ending with the current month.
    pub months: Vec<Month>,
    /// Full months of history available for estimates.
    pub history_months: u32,
    pub this_month_estimate: Option<i64>,
    pub next_month: Option<Forecast>,
    pub income: i64,
    /// Income less estimated spending next month, before loan instalments.
    pub savings_estimate: Option<i64>,
    pub budgets: Vec<BudgetLine>,
    pub awaiting_reimbursement: i64,
    pub alerts: Vec<Alert>,
}

const HISTORY: u32 = 13;
/// Smallest rise worth mentioning: 500.00.
const NOTABLE: i64 = 50_000;

fn month_key(d: NaiveDate) -> String {
    format!("{:04}-{:02}", d.year(), d.month())
}

fn month_start(d: NaiveDate) -> NaiveDate {
    d.with_day(1).unwrap_or(d)
}

fn days_in_month(d: NaiveDate) -> i64 {
    let start = month_start(d);
    start
        .checked_add_months(Months::new(1))
        .map_or(30, |next| (next - start).num_days())
}

pub fn build(data: &Data, today: NaiveDate) -> Insights {
    let this_month = month_start(today);
    let mut months: BTreeMap<String, Month> = (0..HISTORY)
        .filter_map(|back| this_month.checked_sub_months(Months::new(back)))
        .map(|d| {
            let month = month_key(d);
            (
                month.clone(),
                Month {
                    month,
                    ..Default::default()
                },
            )
        })
        .collect();

    let mut first_spend: Option<NaiveDate> = None;
    for e in &data.expenses {
        let cost = my_cost(&e.data);
        let Ok(date) = parse_date("date", &e.data.date) else {
            continue;
        };
        if cost == 0 || date > today {
            continue;
        }
        first_spend = Some(first_spend.map_or(date, |f| f.min(date)));
        if let Some(m) = months.get_mut(&month_key(date)) {
            m.spend += cost;
            *m.by_category.entry(e.data.category.clone()).or_default() += cost;
        }
    }

    let months: Vec<Month> = months.into_values().collect();

    // Full months from the first recorded spend up to last month.
    let history_months = first_spend.map_or(0, |first| {
        let first = month_start(first);
        let span = (this_month.year() - first.year()) * 12
            + (this_month.month() as i32 - first.month() as i32);
        span.clamp(0, HISTORY as i32 - 1) as u32
    });

    // Recent months weigh more: 3, 2, 1.
    let recent: Vec<(&Month, i64)> = months
        .iter()
        .rev()
        .skip(1)
        .take(history_months.min(3) as usize)
        .zip([3, 2, 1])
        .collect();
    let weight: i64 = recent.iter().map(|(_, w)| w).sum();
    let average = (history_months >= 2).then(|| {
        let mut by_category: BTreeMap<String, i64> = BTreeMap::new();
        for (month, w) in &recent {
            for (category, amount) in &month.by_category {
                *by_category.entry(category.clone()).or_default() += amount * w;
            }
        }
        by_category.values_mut().for_each(|v| *v /= weight);
        by_category
    });

    let current = months.last();
    let spent_now = current.map_or(0, |m| m.spend);
    let average_total: Option<i64> = average.as_ref().map(|a| a.values().sum());
    // Spend so far plus the usual share for the days left. Extrapolating this
    // month's pace would be wild after a single large tour.
    let this_month_estimate = average_total.map(|avg| {
        let days = days_in_month(today);
        spent_now + avg * (days - i64::from(today.day())) / days
    });
    let next_month = average.clone().map(|by_category| Forecast {
        spend: by_category.values().sum(),
        by_category,
    });
    let income = data.profile.monthly_income;
    let savings_estimate = next_month
        .as_ref()
        .filter(|_| income > 0)
        .map(|f| income - f.spend);

    let budgets: Vec<BudgetLine> = data
        .budgets
        .iter()
        .map(|b| BudgetLine {
            category: b.id.clone(),
            budget: b.data.monthly,
            spent: match current {
                Some(m) if b.id == "total" => m.spend,
                Some(m) => m.by_category.get(&b.id).copied().unwrap_or(0),
                None => 0,
            },
        })
        .collect();

    let mut alerts = Vec::new();
    if data.orphans > 0 {
        let mut alert = Alert::new("orphans", "warn");
        alert.amount = data.orphans as i64;
        alerts.push(alert);
    }

    let mut awaiting_reimbursement = 0;
    for t in &data.tours {
        let tour = &t.data;
        if tour.kind != TourKind::Company || tour.status == TourStatus::Settled {
            continue;
        }
        let Ok(end) = parse_date("end_date", &tour.end_date) else {
            continue;
        };
        let net = t.summary.net;
        awaiting_reimbursement += net.max(0);
        let since = (today - end).num_days();
        if net == 0 || since < 0 {
            continue;
        }
        let kind = match (net > 0, tour.status) {
            (false, _) => "advance_unreturned",
            (true, TourStatus::Submitted) => "claim_waiting",
            (true, _) => "claim_not_sent",
        };
        let mut alert = Alert::new(kind, if since > 15 { "warn" } else { "info" });
        alert.subject = tour.name.clone();
        alert.amount = net.abs();
        alert.days = if tour.status == TourStatus::Submitted {
            parse_date("submitted_on", &tour.submitted_on)
                .map_or(since, |sent| (today - sent).num_days().max(0))
        } else {
            since
        };
        alert.ref_id = t.id.clone();
        alerts.push(alert);
    }

    for t in &data.tours {
        if let Ok(start) = parse_date("start_date", &t.data.start_date) {
            let days = (start - today).num_days();
            if (0..=7).contains(&days) && t.data.status == TourStatus::Planned {
                let mut alert = Alert::new("tour_soon", "info");
                alert.subject = t.data.name.clone();
                alert.days = days;
                alert.ref_id = t.id.clone();
                alerts.push(alert);
            }
        }
    }

    for line in &budgets {
        let percent = line.spent.saturating_mul(100) / line.budget.max(1);
        if percent >= 80 {
            let mut alert = Alert::new(
                if percent > 100 {
                    "budget_over"
                } else {
                    "budget_near"
                },
                if percent > 100 { "danger" } else { "warn" },
            );
            alert.subject = line.category.clone();
            alert.amount = (line.spent - line.budget).abs();
            alert.percent = percent;
            alerts.push(alert);
        }
    }

    if let (Some(average), Some(current)) = (&average, current) {
        for (category, spent) in &current.by_category {
            let usual = average.get(category).copied().unwrap_or(0);
            if usual > 0 && spent - usual >= NOTABLE && spent * 4 >= usual * 5 {
                let mut alert = Alert::new("category_up", "info");
                alert.subject = category.clone();
                alert.amount = spent - usual;
                alert.percent = (spent - usual) * 100 / usual;
                alerts.push(alert);
            }
        }
    }

    // What this month looked like a year ago: the oldest month kept.
    if let Some(year_ago) = months.first()
        && year_ago.spend > 0
    {
        let top = year_ago.by_category.iter().max_by_key(|(_, v)| **v);
        let mut alert = Alert::new("last_year", "info");
        alert.subject = top.map(|(c, _)| c.clone()).unwrap_or_default();
        alert.amount = year_ago.spend;
        alerts.push(alert);
    }

    Insights {
        months,
        history_months,
        this_month_estimate,
        next_month,
        income,
        savings_estimate,
        budgets,
        awaiting_reimbursement,
        alerts,
    }
}
