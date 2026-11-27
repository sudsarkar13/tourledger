//! What a tour cost, who paid, and who owes whom.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::model::{Claim, Expense, ME, Transfer, TransferKind};
use crate::money;

pub const COMPANY_PAYER: &str = "company";

/// A claim is made per tour, so without one the cost is mine.
pub fn is_reimbursable(e: &Expense) -> bool {
    e.claim == Claim::Reimbursable && !e.rejected && !e.tour_id.is_empty()
}

/// How one expense divides between the people sharing it. Empty when it is not
/// shared: paid by the company, claimed from the company, or no split chosen.
fn shares(e: &Expense) -> Vec<(&str, i64)> {
    if e.paid_by == COMPANY_PAYER || is_reimbursable(e) {
        return Vec::new();
    }
    let mut members: Vec<&str> = e.split_with.iter().map(String::as_str).collect();
    members.sort_unstable();
    members.dedup();
    let amounts = money::split(e.home_amount(), members.len());
    members.into_iter().zip(amounts).collect()
}

/// What this expense finally costs me, ignoring who handed over the cash.
pub fn my_cost(e: &Expense) -> i64 {
    if e.paid_by == COMPANY_PAYER || is_reimbursable(e) {
        return 0;
    }
    let shares = shares(e);
    if shares.is_empty() {
        return if e.paid_by == ME { e.home_amount() } else { 0 };
    }
    shares
        .iter()
        .find(|(who, _)| *who == ME)
        .map_or(0, |(_, amount)| *amount)
}

#[derive(Debug, Default, Serialize)]
pub struct Summary {
    pub total_cost: i64,
    /// Cash that left my pocket.
    pub paid_by_me: i64,
    pub company_paid: i64,
    pub colleague_paid: i64,
    /// My own cost after splits; excludes anything being claimed.
    pub my_cost: i64,
    pub claimable: i64,
    pub rejected: i64,
    pub advance: i64,
    pub advance_returned: i64,
    pub reimbursed: i64,
    /// Bonus or allowance kept; never set against the claim.
    pub allowance: i64,
    /// Positive: the company owes me. Negative: I owe the company.
    pub net: i64,
    /// Per person. Positive: they owe me. Negative: I owe them.
    pub balances: BTreeMap<String, i64>,
    pub by_category: BTreeMap<String, i64>,
}

pub fn summarise<'a>(
    expenses: impl IntoIterator<Item = &'a Expense>,
    transfers: impl IntoIterator<Item = &'a Transfer>,
) -> Summary {
    let mut s = Summary::default();
    for e in expenses {
        let amount = e.home_amount();
        s.total_cost += amount;
        *s.by_category.entry(e.category.clone()).or_default() += amount;
        s.my_cost += my_cost(e);
        if e.rejected && e.claim == Claim::Reimbursable && e.paid_by == ME {
            s.rejected += amount;
        }
        match e.paid_by.as_str() {
            COMPANY_PAYER => s.company_paid += amount,
            ME => {
                s.paid_by_me += amount;
                if is_reimbursable(e) {
                    s.claimable += amount;
                }
                for (who, share) in shares(e) {
                    if who != ME {
                        *s.balances.entry(who.to_owned()).or_default() += share;
                    }
                }
            }
            payer => {
                s.colleague_paid += amount;
                if let Some((_, share)) = shares(e).iter().find(|(who, _)| *who == ME) {
                    *s.balances.entry(payer.to_owned()).or_default() -= share;
                }
            }
        }
    }
    for t in transfers {
        match t.kind {
            TransferKind::Advance => s.advance += t.amount,
            TransferKind::AdvanceReturned => s.advance_returned += t.amount,
            TransferKind::Reimbursement => s.reimbursed += t.amount,
            TransferKind::Allowance => s.allowance += t.amount,
            TransferKind::ReceivedFrom => {
                *s.balances.entry(t.person_id.clone()).or_default() -= t.amount
            }
            TransferKind::PaidTo => *s.balances.entry(t.person_id.clone()).or_default() += t.amount,
        }
    }
    s.net = s.claimable - s.advance + s.advance_returned - s.reimbursed;
    s.balances.retain(|_, v| *v != 0);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expense(amount: i64, paid_by: &str, claim: Claim, split: &[&str]) -> Expense {
        Expense {
            tour_id: "tour".into(),
            category: "food".into(),
            amount,
            paid_by: paid_by.into(),
            claim,
            split_with: split.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    fn transfer(kind: TransferKind, amount: i64, person: &str) -> Transfer {
        Transfer {
            kind,
            amount,
            person_id: person.into(),
            ..Default::default()
        }
    }

    /// The trade fair case: auto to the station and food claimed, rail tickets
    /// booked by the company, an advance taken beforehand.
    #[test]
    fn trade_fair_claim_less_advance() {
        let expenses = [
            expense(25_000, ME, Claim::Reimbursable, &[]),
            expense(184_000, COMPANY_PAYER, Claim::Reimbursable, &[]),
            expense(320_000, ME, Claim::Reimbursable, &[]),
        ];
        let s = summarise(&expenses, &[transfer(TransferKind::Advance, 200_000, "")]);
        assert_eq!(s.total_cost, 529_000);
        assert_eq!(s.claimable, 345_000);
        assert_eq!(s.net, 145_000);
        assert_eq!(s.my_cost, 0);
    }

    #[test]
    fn unspent_advance_is_owed_back_until_returned() {
        let expenses = [expense(30_000, ME, Claim::Reimbursable, &[])];
        let mut transfers = vec![transfer(TransferKind::Advance, 50_000, "")];
        assert_eq!(summarise(&expenses, &transfers).net, -20_000);
        transfers.push(transfer(TransferKind::AdvanceReturned, 20_000, ""));
        assert_eq!(summarise(&expenses, &transfers).net, 0);
    }

    #[test]
    fn a_bonus_is_not_set_against_the_claim() {
        let expenses = [expense(30_000, ME, Claim::Reimbursable, &[])];
        let s = summarise(&expenses, &[transfer(TransferKind::Allowance, 500_000, "")]);
        assert_eq!((s.net, s.allowance), (30_000, 500_000));
    }

    #[test]
    fn rejected_line_becomes_my_cost() {
        let mut e = expense(10_000, ME, Claim::Reimbursable, &[]);
        e.rejected = true;
        let s = summarise([&e], []);
        assert_eq!((s.claimable, s.my_cost, s.rejected), (0, 10_000, 10_000));
    }

    #[test]
    fn a_claim_without_a_tour_still_counts_as_my_cost() {
        let mut e = expense(10_000, ME, Claim::Reimbursable, &[]);
        e.tour_id.clear();
        assert_eq!(my_cost(&e), 10_000);
    }

    #[test]
    fn shared_costs_track_who_owes_whom() {
        let expenses = [
            expense(300, ME, Claim::Personal, &[ME, "ravi", "asha"]),
            expense(200, "ravi", Claim::Personal, &[ME, "ravi"]),
            expense(500, "asha", Claim::Personal, &[]),
        ];
        let s = summarise(
            &expenses,
            &[transfer(TransferKind::ReceivedFrom, 100, "asha")],
        );
        // Ravi's 100 owed to me cancels the 100 I owe him; Asha has paid up.
        assert!(s.balances.is_empty(), "{:?}", s.balances);
        assert_eq!(s.my_cost, 200);
        assert_eq!(s.colleague_paid, 700);
    }
}
