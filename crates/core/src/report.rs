//! A tour's expenses sorted into the headings a claim is read by. The screen, the
//! PDF and the spreadsheet are all drawn from this one structure.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::entry;
use crate::export::{category_name, payer_name};
use crate::model::{Expense, ME, Tour, TourKind, TransferKind};
use crate::money;
use crate::settlement::{COMPANY_PAYER, is_reimbursable};
use crate::views::{Data, TourView};

/// Heading, and the categories filed under it.
const HEADINGS: [(&str, &[&str]); 5] = [
    (
        "Travelling expenses",
        &["local", "train", "flight", "bus", "fuel"],
    ),
    ("Food and refreshments", &["food"]),
    ("Stay", &["lodging"]),
    ("Event and tickets", &["event"]),
    ("Miscellaneous", &["phone", "shopping", "medical", "other"]),
];

#[derive(Debug, Clone, Serialize)]
pub struct Line {
    pub id: String,
    pub date: String,
    pub category: String,
    pub particulars: String,
    /// Who it was spent for.
    pub people: String,
    pub paid_by: String,
    pub bill_no: String,
    pub amount: i64,
    /// Numbers of the pictures attached to this entry.
    pub bills: Vec<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Bill {
    pub number: u32,
    pub hash: String,
    /// The transaction it supports.
    pub label: String,
    pub width: u32,
    pub height: u32,
    pub full_page: bool,
    pub present: bool,
}

#[derive(Debug, Default, Serialize)]
pub struct Group {
    pub name: String,
    /// On a company claim: what I paid and ask back. Otherwise: everything.
    pub lines: Vec<Line>,
    pub total: i64,
    /// Booked and paid by the company, listed for the record.
    pub company_lines: Vec<Line>,
    pub company_total: i64,
}

/// Spending before the event (the journey out), during it, or after it.
#[derive(Debug, Serialize)]
pub struct Stage {
    pub name: String,
    pub total: i64,
}

/// A listed entry whose note points to another category than it is filed under.
#[derive(Debug, Serialize)]
pub struct Doubt {
    pub id: String,
    pub particulars: String,
    pub amount: i64,
    pub filed: String,
    pub suggested: String,
}

#[derive(Debug, Serialize)]
pub struct Report {
    /// Whether every entry is handed over, or only totals by category.
    pub detail: bool,
    /// True for a claim on a company, false for a personal expense report.
    pub is_claim: bool,
    pub title: String,
    pub company: String,
    pub tour: String,
    /// Label and value pairs about the claimant and the tour.
    pub claimant: Vec<(String, String)>,
    pub details: Vec<(String, String)>,
    pub groups: Vec<Group>,
    pub total: i64,
    pub company_total: i64,
    /// Entries left off a claim: personal, rejected or paid by someone else.
    pub left_out_count: usize,
    pub left_out_total: i64,
    /// Dated money between me and the company.
    pub money: Vec<(String, String, i64)>,
    pub advance: i64,
    pub advance_returned: i64,
    pub reimbursed: i64,
    /// Only when something fell outside the event's dates.
    pub stages: Vec<Stage>,
    /// Never changed by itself: shown for the person to decide.
    pub check: Vec<Doubt>,
    /// Supporting pictures, for listed entries only.
    pub bills: Vec<Bill>,
    /// Listed entries that have no picture.
    pub without_bill: usize,
    pub allowance: i64,
    /// What was spent for each person, when more than one is involved.
    pub by_person: Vec<(String, i64)>,
    /// Positive: the company pays me. Negative: I return it.
    pub net: i64,
    pub net_label: String,
    pub net_words: String,
}

fn readable(iso: &str) -> String {
    chrono::NaiveDate::parse_from_str(iso, "%Y-%m-%d")
        .map_or_else(|_| iso.to_owned(), |d| d.format("%-d %b %Y").to_string())
}

fn route(e: &Expense) -> String {
    if e.from.is_empty() || e.to.is_empty() {
        String::new()
    } else {
        format!("{} → {}", e.from, e.to)
    }
}

fn particulars(e: &Expense) -> String {
    let mut parts: Vec<String> = [e.description.clone(), route(e), e.mode.clone()]
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() {
        parts.push(category_name(&e.category).to_owned());
    }
    if !e.currency.is_empty() && e.currency != "INR" {
        parts.push(format!("{} {}", e.currency, money::indian(e.amount)));
    }
    parts.join(", ")
}

/// My name as entered in the profile, which is how a claim names me.
fn my_name(data: &Data) -> String {
    let name = data.profile.name.trim();
    if name.is_empty() {
        "Self".to_owned()
    } else {
        name.to_owned()
    }
}

/// Who it was spent for: "Both" for the two who travelled together, "All" for a
/// whole party of three or more, otherwise their names.
fn people(data: &Data, tour: &Tour, e: &Expense) -> String {
    let sharers: BTreeSet<&str> = e.split_with.iter().map(String::as_str).collect();
    if sharers.is_empty() {
        return String::new();
    }
    let mut party: BTreeSet<&str> = tour.participants.iter().map(String::as_str).collect();
    party.insert(ME);
    let with_me = sharers.contains(ME);
    // With no travellers listed, the two named are the two who travelled.
    if with_me && sharers.len() == 2 && (party.len() == 1 || party == sharers) {
        return "Both".to_owned();
    }
    if party.len() > 2 && party == sharers {
        return "All".to_owned();
    }
    let mut names: Vec<String> = sharers
        .iter()
        .filter(|id| **id != ME)
        .map(|id| payer_name(data, id))
        .collect();
    names.sort();
    if with_me {
        names.insert(0, my_name(data));
    }
    names.join(", ")
}

/// `detail` adds every entry; without it only totals by category are handed over.
pub fn build(data: &Data, tour: &TourView, detail: bool) -> Report {
    let t = &tour.data;
    let is_claim = t.kind == TourKind::Company;
    let company = data.companies.iter().find(|c| c.id == t.company_id);

    let mut expenses: Vec<_> = data
        .expenses
        .iter()
        .filter(|e| e.data.tour_id == tour.id)
        .collect();
    expenses.sort_by(|a, b| (&a.data.date, &a.id).cmp(&(&b.data.date, &b.id)));

    let mut groups: Vec<Group> = HEADINGS
        .iter()
        .map(|(name, _)| Group {
            name: (*name).to_owned(),
            ..Default::default()
        })
        .collect();
    let (mut left_out_count, mut left_out_total) = (0, 0);
    let mut by_person: BTreeMap<String, i64> = BTreeMap::new();

    for e in &expenses {
        let x = &e.data;
        let line = Line {
            id: e.id.clone(),
            date: x.date.clone(),
            category: category_name(&x.category).to_owned(),
            particulars: particulars(x),
            people: people(data, t, x),
            paid_by: if x.paid_by == ME {
                "Self".into()
            } else {
                payer_name(data, &x.paid_by)
            },
            bill_no: x.bill_no.clone(),
            amount: e.home,
            bills: Vec::new(),
        };
        let at = HEADINGS
            .iter()
            .position(|(_, categories)| categories.contains(&x.category.as_str()))
            .unwrap_or(HEADINGS.len() - 1);
        let group = &mut groups[at];
        if !is_claim || (x.paid_by == ME && is_reimbursable(x)) {
            // Shared equally between those it was spent for; mine if unsaid.
            let mut sharers: Vec<&str> = x.split_with.iter().map(String::as_str).collect();
            if sharers.is_empty() {
                sharers.push(ME);
            }
            for (who, share) in sharers.iter().zip(money::split(line.amount, sharers.len())) {
                let name = if *who == ME {
                    my_name(data)
                } else {
                    payer_name(data, who)
                };
                *by_person.entry(name).or_default() += share;
            }
            group.total += line.amount;
            group.lines.push(line);
        } else if x.paid_by == COMPANY_PAYER {
            group.company_total += line.amount;
            group.company_lines.push(line);
        } else {
            left_out_count += 1;
            left_out_total += line.amount;
        }
    }
    groups.retain(|g| !g.lines.is_empty() || !g.company_lines.is_empty());

    // Bills are numbered in the order the lines are read. Entries left off the
    // report take their pictures with them.
    let mut bills: Vec<Bill> = Vec::new();
    let mut without_bill = 0;
    for group in &mut groups {
        let heading = group.name.clone();
        for line in &mut group.lines {
            for a in data
                .attachments
                .iter()
                .filter(|a| a.data.expense_id == line.id)
            {
                let number = bills.len() as u32 + 1;
                line.bills.push(number);
                bills.push(Bill {
                    number,
                    hash: a.data.hash.clone(),
                    label: if detail {
                        format!(
                            "Bill {number}: {}, {}, {}, ₹{}",
                            readable(&line.date),
                            line.category,
                            line.particulars,
                            money::indian(line.amount)
                        )
                    } else {
                        format!(
                            "Bill {number}: {}, {heading}, ₹{}",
                            readable(&line.date),
                            money::indian(line.amount)
                        )
                    },
                    width: a.data.width,
                    height: a.data.height,
                    full_page: a.data.full_page,
                    present: a.present,
                });
            }
            if line.bills.is_empty() {
                without_bill += 1;
            }
        }
    }

    let mut transfers: Vec<_> = data
        .transfers
        .iter()
        .filter(|x| x.data.tour_id == tour.id)
        .collect();
    transfers.sort_by(|a, b| a.data.date.cmp(&b.data.date));
    let money_rows = transfers
        .iter()
        .filter_map(|x| {
            let label = match x.data.kind {
                TransferKind::Advance => "Cash received from company",
                TransferKind::AdvanceReturned => "Cash returned to company",
                TransferKind::Reimbursement => "Reimbursement received",
                _ => return None,
            };
            Some((x.data.date.clone(), label.to_owned(), x.data.amount))
        })
        .collect();

    let text = |label: &str, value: &str| (label.to_owned(), value.to_owned());
    let keep = |pairs: Vec<(String, String)>| -> Vec<(String, String)> {
        pairs.into_iter().filter(|(_, v)| !v.is_empty()).collect()
    };
    let travellers: Vec<String> = t
        .participants
        .iter()
        .map(|id| payer_name(data, id))
        .collect();

    let s = &tour.summary;
    let net = if is_claim { s.net } else { 0 };
    let net_label = if net > 0 {
        "Amount payable to me by the company"
    } else if net < 0 {
        "Amount to be returned by me to the company"
    } else {
        "Nothing due either way"
    };

    let mut by_person: Vec<(String, i64)> = by_person.into_iter().collect();
    let me = my_name(data);
    by_person.sort_by_key(|(name, _)| (*name != me, name.clone()));
    if by_person.len() < 2 {
        by_person.clear();
    }
    let purpose = match (t.kind, t.purpose.as_str()) {
        (TourKind::Individual, _) => "Personal trip",
        (_, "wellness") => "Company wellness trip",
        (_, "reward") => "Company reward or bonus trip",
        _ => "Business",
    };

    let listed: BTreeSet<&str> = groups
        .iter()
        .flat_map(|g| g.lines.iter().map(|l| l.id.as_str()))
        .collect();
    let mut stage_totals = [0i64; 3];
    let mut check = Vec::new();
    for e in expenses.iter().filter(|e| listed.contains(e.id.as_str())) {
        let x = &e.data;
        let at = if x.date < t.start_date {
            0
        } else if x.date > t.end_date {
            2
        } else {
            1
        };
        stage_totals[at] += e.home;
        // Only the two "catch-all" filings are questioned; any other was chosen.
        if matches!(x.category.as_str(), "local" | "other")
            && let Some(suggested) = entry::categorise(&x.description)
            && suggested != x.category
        {
            check.push(Doubt {
                id: e.id.clone(),
                particulars: particulars(x),
                amount: e.home,
                filed: x.category.clone(),
                suggested: suggested.into(),
            });
        }
    }
    let stages = if stage_totals[0] == 0 && stage_totals[2] == 0 {
        Vec::new()
    } else {
        [
            format!("Journey before {}", readable(&t.start_date)),
            format!(
                "Event days, {} to {}",
                readable(&t.start_date),
                readable(&t.end_date)
            ),
            format!("Return after {}", readable(&t.end_date)),
        ]
        .into_iter()
        .zip(stage_totals)
        .filter(|(_, total)| *total != 0)
        .map(|(name, total)| Stage { name, total })
        .collect()
    };

    Report {
        detail,
        stages,
        check,
        bills,
        without_bill,
        allowance: s.allowance,
        by_person,
        is_claim,
        title: if is_claim {
            "Tour expense claim".into()
        } else {
            "Tour expense report".into()
        },
        company: company.map(|c| c.data.name.clone()).unwrap_or_default(),
        tour: t.name.clone(),
        claimant: keep(vec![
            text("Name", &data.profile.name),
            text("Employee ID", company.map_or("", |c| &c.data.employee_id)),
            text("Designation", company.map_or("", |c| &c.data.designation)),
            text("Department", company.map_or("", |c| &c.data.department)),
            text("Phone", &data.profile.phone),
            text("UPI ID", &data.profile.upi_id),
        ]),
        details: keep(vec![
            text("Purpose", purpose),
            text("Event", &t.event_name),
            text("Organiser", &t.organiser),
            text("Venue", &t.venue),
            text("City", &t.city),
            text("From", &readable(&t.start_date)),
            text("To", &readable(&t.end_date)),
            text("Travelled with", &travellers.join(", ")),
        ]),
        total: groups.iter().map(|g| g.total).sum(),
        company_total: groups.iter().map(|g| g.company_total).sum(),
        groups,
        left_out_count,
        left_out_total,
        money: money_rows,
        advance: s.advance,
        advance_returned: s.advance_returned,
        reimbursed: s.reimbursed,
        net,
        net_label: net_label.into(),
        net_words: money::words(net),
    }
}
