//! Everything the UI shows, built from the store in one pass.
//!
//! One person's data is small, so the UI reloads a whole snapshot after each
//! change. Revisit if a ledger ever reaches tens of thousands of entries.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::insights::{self, Insights};
use crate::loans;
use crate::model::*;
use crate::portfolio::{self, Portfolio};
use crate::settlement::{self, Summary};
use crate::store::Store;

#[derive(Debug, Serialize)]
pub struct Item<T> {
    pub id: String,
    #[serde(flatten)]
    pub data: T,
}

#[derive(Debug, Serialize)]
pub struct ExpenseView {
    pub id: String,
    #[serde(flatten)]
    pub data: Expense,
    /// `amount` in rupees, converted here so every screen agrees to the paisa.
    pub home: i64,
}

#[derive(Debug, Serialize)]
pub struct TourView {
    pub id: String,
    #[serde(flatten)]
    pub data: Tour,
    pub summary: Summary,
    /// Where a company claim stands; empty for a personal trip.
    pub claim_state: &'static str,
}

#[derive(Debug, Serialize)]
pub struct LoanView {
    pub id: String,
    #[serde(flatten)]
    pub data: Loan,
    pub status: loans::Status,
}

#[derive(Debug, Serialize)]
pub struct AttachmentView {
    pub id: String,
    #[serde(flatten)]
    pub data: Attachment,
    /// False while the picture has not yet arrived from another device.
    pub present: bool,
}

#[derive(Debug, Serialize)]
pub struct Data {
    pub profile: Profile,
    pub companies: Vec<Item<Company>>,
    pub people: Vec<Item<Person>>,
    pub quick: Vec<Item<QuickEntry>>,
    pub tours: Vec<TourView>,
    pub expenses: Vec<ExpenseView>,
    pub transfers: Vec<Item<Transfer>>,
    pub loans: Vec<LoanView>,
    pub payments: Vec<Item<Payment>>,
    pub budgets: Vec<Item<Budget>>,
    /// Oldest first.
    pub attachments: Vec<AttachmentView>,
    /// Expenses and advances whose tour was deleted on another device.
    pub orphans: usize,
    /// Across everything. Positive: they owe me. Negative: I owe them.
    pub balances: BTreeMap<String, i64>,
}

#[derive(Debug, Serialize)]
pub struct Snapshot {
    pub revision: u64,
    #[serde(flatten)]
    pub data: Data,
    pub insights: Insights,
    pub portfolio: Portfolio,
}

fn claim_state(tour: &Tour, s: &Summary) -> &'static str {
    if tour.kind != TourKind::Company {
        ""
    } else if tour.status == TourStatus::Settled {
        "settled"
    } else if s.claimable == 0 && s.advance == 0 {
        "none"
    } else if s.net == 0 {
        "paid"
    } else if s.net < 0 {
        "return_due"
    } else if s.reimbursed > 0 {
        "part_paid"
    } else if tour.status == TourStatus::Submitted || !tour.submitted_on.is_empty() {
        "sent"
    } else {
        "not_sent"
    }
}

pub fn load<T: DeserializeOwned>(store: &Store, kind: &str) -> Vec<Item<T>> {
    store
        .live(kind)
        .filter_map(|r| {
            serde_json::from_value(r.body.clone())
                .ok()
                .map(|data| Item {
                    id: r.id.clone(),
                    data,
                })
        })
        .collect()
}

pub fn data(store: &Store, today: NaiveDate) -> Data {
    let profile = store
        .get(PROFILE, ME)
        .and_then(|r| serde_json::from_value(r.body.clone()).ok())
        .unwrap_or_default();

    let mut companies: Vec<Item<Company>> = load(store, COMPANY);
    companies.sort_by_key(|c| c.data.name.to_lowercase());
    let mut people: Vec<Item<Person>> = load(store, PERSON);
    people.sort_by_key(|p| p.data.name.to_lowercase());
    let mut quick: Vec<Item<QuickEntry>> = load(store, QUICK);
    quick.sort_by_key(|q| q.data.label.to_lowercase());

    let mut expenses: Vec<ExpenseView> = load::<Expense>(store, EXPENSE)
        .into_iter()
        .map(|e| ExpenseView {
            home: e.data.home_amount(),
            id: e.id,
            data: e.data,
        })
        .collect();
    expenses.sort_by(|a, b| (&b.data.date, &b.id).cmp(&(&a.data.date, &a.id)));
    let mut transfers: Vec<Item<Transfer>> = load(store, TRANSFER);
    transfers.sort_by(|a, b| (&b.data.date, &b.id).cmp(&(&a.data.date, &a.id)));

    // A tour deleted on one device while another added to it leaves entries
    // pointing nowhere. Show them as outside any tour so they can be moved.
    let gone = |tour: &str| !tour.is_empty() && store.get(TOUR, tour).is_none();
    let mut orphans = 0;
    for e in expenses.iter_mut().filter(|e| gone(&e.data.tour_id)) {
        e.data.tour_id.clear();
        orphans += 1;
    }
    orphans += transfers.iter().filter(|t| gone(&t.data.tour_id)).count();

    let mut tours: Vec<TourView> = load::<Tour>(store, TOUR)
        .into_iter()
        .map(|t| {
            let summary = settlement::summarise(
                expenses
                    .iter()
                    .filter(|e| e.data.tour_id == t.id)
                    .map(|e| &e.data),
                transfers
                    .iter()
                    .filter(|x| x.data.tour_id == t.id)
                    .map(|x| &x.data),
            );
            TourView {
                claim_state: claim_state(&t.data, &summary),
                id: t.id,
                data: t.data,
                summary,
            }
        })
        .collect();
    tours.sort_by(|a, b| (&b.data.start_date, &b.id).cmp(&(&a.data.start_date, &a.id)));

    let mut payments: Vec<Item<Payment>> = load(store, PAYMENT);
    payments.sort_by(|a, b| (&b.data.date, &b.id).cmp(&(&a.data.date, &a.id)));
    let mut loans: Vec<LoanView> = load::<Loan>(store, LOAN)
        .into_iter()
        .map(|l| {
            let status = loans::status(
                &l.data,
                payments
                    .iter()
                    .filter(|p| p.data.loan_id == l.id)
                    .map(|p| &p.data),
                today,
            );
            LoanView {
                id: l.id,
                data: l.data,
                status,
            }
        })
        .collect();
    loans.sort_by_key(|l| (l.data.closed, l.data.name.to_lowercase()));

    let balances = settlement::summarise(
        expenses.iter().map(|e| &e.data),
        transfers.iter().map(|t| &t.data),
    )
    .balances;

    Data {
        profile,
        companies,
        people,
        quick,
        tours,
        expenses,
        transfers,
        loans,
        payments,
        budgets: load(store, BUDGET),
        attachments: {
            let blobs = store.blobs();
            let mut list: Vec<AttachmentView> = load::<Attachment>(store, ATTACHMENT)
                .into_iter()
                .map(|a| AttachmentView {
                    present: blobs.has(&a.data.hash),
                    id: a.id,
                    data: a.data,
                })
                .collect();
            list.sort_by(|a, b| a.id.cmp(&b.id));
            list
        },
        balances,
        orphans,
    }
}

pub fn snapshot(store: &Store, today: NaiveDate) -> Snapshot {
    let data = data(store, today);
    let insights = insights::build(&data, today);
    let portfolio = portfolio::build(
        &data.loans,
        &data.payments,
        data.profile.monthly_income,
        today,
    );
    Snapshot {
        revision: store.revision(),
        data,
        insights,
        portfolio,
    }
}
