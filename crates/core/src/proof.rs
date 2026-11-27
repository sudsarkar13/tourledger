//! How each of my payments on a tour was made, and which have a picture of a bill
//! or payment screen to show for them.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::model::{Expense, ME, TourKind};
use crate::settlement::is_reimbursable;
use crate::views::ExpenseView;

/// In the order they are listed. Both kinds of card are counted together.
const MODES: [&str; 6] = ["UPI", "Cash", "Card", "Net banking", "Other", "Not noted"];

pub fn mode_name(method: &str) -> &'static str {
    match method {
        "UPI" => "UPI",
        "Cash" => "Cash",
        "Credit card" | "Debit card" => "Card",
        "Net banking" => "Net banking",
        "" => "Not noted",
        // Written by a newer version.
        _ => "Other",
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Tally {
    pub count: usize,
    pub total: i64,
}

impl Tally {
    fn add(&mut self, amount: i64) {
        self.count += 1;
        self.total += amount;
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Mode {
    pub name: &'static str,
    pub all: Tally,
    /// Of those, the ones with a picture.
    pub pictured: Tally,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct Proof {
    /// Only the modes that were used.
    pub modes: Vec<Mode>,
    pub all: Tally,
    pub pictured: Tally,
    /// Expenses still without a picture.
    pub missing: Vec<String>,
}

/// Only what I paid myself, and on a company tour only what I ask back: these
/// are the payments I may be asked to show.
pub fn counts(kind: TourKind, e: &Expense) -> bool {
    e.paid_by == ME && (kind != TourKind::Company || is_reimbursable(e))
}

pub fn summarise<'a>(
    expenses: impl Iterator<Item = &'a ExpenseView>,
    pictured: &BTreeSet<&str>,
) -> Proof {
    let mut modes = MODES.map(|name| Mode {
        name,
        all: Tally::default(),
        pictured: Tally::default(),
    });
    let mut proof = Proof::default();
    for e in expenses {
        let name = mode_name(&e.data.payment_method);
        let mode = modes.iter_mut().find(|m| m.name == name);
        let has = pictured.contains(e.id.as_str());
        if let Some(mode) = mode {
            mode.all.add(e.home);
            if has {
                mode.pictured.add(e.home);
            }
        }
        proof.all.add(e.home);
        if has {
            proof.pictured.add(e.home);
        } else {
            proof.missing.push(e.id.clone());
        }
    }
    proof.modes = modes.into_iter().filter(|m| m.all.count > 0).collect();
    proof
}
