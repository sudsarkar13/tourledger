//! Drafts an entry from a pasted bank or lender message. It only suggests; the
//! person confirms before anything is saved.

use chrono::NaiveDate;
use serde::Serialize;

use crate::money;

#[derive(Debug, Serialize, PartialEq)]
pub struct Draft {
    pub amount: i64,
    pub date: String,
    pub is_loan: bool,
    /// Money came in rather than went out.
    pub is_credit: bool,
    pub description: String,
}

/// The first amount written after a currency marker. Banks put the transaction
/// amount before any balance.
fn amount(text: &str) -> Option<i64> {
    let lower = text.to_ascii_lowercase();
    let mut best: Option<(usize, i64)> = None;
    for marker in ["₹", "inr", "rs.", "rs "] {
        for (at, _) in lower.match_indices(marker) {
            let rest = lower[at + marker.len()..].trim_start_matches([' ', '.', ':']);
            let number: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == ',' || *c == '.')
                .collect();
            if let Some(value) = money::parse(number.trim_end_matches(['.', ',']))
                && value > 0
                && best.is_none_or(|(first, _)| at < first)
            {
                best = Some((at, value));
            }
        }
    }
    best.map(|(_, v)| v)
}

fn date(text: &str) -> Option<NaiveDate> {
    text.split(|c: char| c.is_whitespace() || c == ',')
        .map(|token| token.trim_matches(|c: char| !c.is_ascii_alphanumeric()))
        .find_map(|token| {
            // Two-digit years first: %Y would accept "26" as the year 26.
            [
                "%d-%m-%y", "%d/%m/%y", "%d-%b-%y", "%d%b%y", "%d-%m-%Y", "%d/%m/%Y", "%d-%b-%Y",
                "%Y-%m-%d",
            ]
            .iter()
            .filter_map(|f| NaiveDate::parse_from_str(token, f).ok())
            .find(|d| (2000..=2100).contains(&chrono::Datelike::year(d)))
        })
}

pub fn parse(text: &str, today: NaiveDate) -> Option<Draft> {
    let amount = amount(text)?;
    let lower = text.to_ascii_lowercase();
    let words: Vec<&str> = lower.split(|c: char| !c.is_ascii_alphanumeric()).collect();
    let has = |list: &[&str]| words.iter().any(|w| list.contains(w));
    Some(Draft {
        amount,
        date: date(text).unwrap_or(today).format("%Y-%m-%d").to_string(),
        is_loan: has(&["emi", "loan", "nach", "ecs"]),
        is_credit: has(&["credited", "received", "refund"]) && !has(&["debited"]),
        description: text
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(140)
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> NaiveDate {
        "2026-11-27".parse().unwrap()
    }

    #[test]
    fn reads_an_emi_debit() {
        let d = parse(
            "Rs.8,884.88 debited from A/c XX1234 on 05-11-26 towards EMI for Loan 998. Avl bal Rs.20,000",
            today(),
        )
        .unwrap();
        assert_eq!((d.amount, d.date.as_str()), (888_488, "2026-11-05"));
        assert!(d.is_loan && !d.is_credit);
    }

    #[test]
    fn reads_a_upi_payment() {
        let d = parse("You paid ₹ 150 to Sharma Auto", today()).unwrap();
        assert_eq!((d.amount, d.date.as_str()), (15_000, "2026-11-27"));
        assert!(!d.is_loan);
    }

    #[test]
    fn gives_up_without_an_amount() {
        assert_eq!(parse("Your OTP is 123456", today()), None);
    }
}
