//! Money is an integer count of minor units (paise). No floating point in sums.

use crate::error::{Error, Result};

/// 100 crore rupees: far above any real entry, far below overflow.
pub const MAX_AMOUNT: i64 = 100_000_000_000;
/// Exchange rates are home units per foreign unit, times this.
pub const RATE_SCALE: i64 = 1_000_000;

/// Converts to the home currency, rounding half up.
pub fn convert(minor: i64, rate_ppm: i64) -> i64 {
    let product = i128::from(minor) * i128::from(rate_ppm);
    let half = i128::from(RATE_SCALE / 2);
    ((product + half) / i128::from(RATE_SCALE)) as i64
}

pub fn check_amount(field: &'static str, minor: i64) -> Result<()> {
    if minor <= 0 {
        return Err(Error::invalid(field, "Enter an amount above zero."));
    }
    if minor > MAX_AMOUNT {
        return Err(Error::invalid(field, "That amount is too large."));
    }
    Ok(())
}

/// Splits `total` into `parts` shares that add back to it exactly. The first
/// shares take the leftover paise.
pub fn split(total: i64, parts: usize) -> Vec<i64> {
    if parts == 0 {
        return Vec::new();
    }
    let n = parts as i64;
    let base = total / n;
    let extra = total % n;
    (0..n).map(|i| base + i64::from(i < extra)).collect()
}

/// "1234.50" from 123450. For files; screens format in the UI.
pub fn plain(minor: i64) -> String {
    let sign = if minor < 0 { "-" } else { "" };
    let v = minor.unsigned_abs();
    format!("{sign}{}.{:02}", v / 100, v % 100)
}

/// "1,23,456.50": the Indian grouping, for documents.
pub fn indian(minor: i64) -> String {
    let text = plain(minor);
    let (sign, text) = text
        .strip_prefix('-')
        .map_or(("", text.as_str()), |rest| ("-", rest));
    let (whole, paise) = text.split_once('.').unwrap_or((text, "00"));
    let split = whole.len().saturating_sub(3);
    let (mut head, tail) = whole.split_at(split);
    let mut groups = Vec::new();
    while head.len() > 2 {
        let (rest, group) = head.split_at(head.len() - 2);
        groups.push(group);
        head = rest;
    }
    if !head.is_empty() {
        groups.push(head);
    }
    groups.reverse();
    groups.push(tail);
    format!("{sign}{}.{paise}", groups.join(","))
}

fn under_hundred(n: u64) -> String {
    const ONES: [&str; 20] = [
        "",
        "One",
        "Two",
        "Three",
        "Four",
        "Five",
        "Six",
        "Seven",
        "Eight",
        "Nine",
        "Ten",
        "Eleven",
        "Twelve",
        "Thirteen",
        "Fourteen",
        "Fifteen",
        "Sixteen",
        "Seventeen",
        "Eighteen",
        "Nineteen",
    ];
    const TENS: [&str; 10] = [
        "", "", "Twenty", "Thirty", "Forty", "Fifty", "Sixty", "Seventy", "Eighty", "Ninety",
    ];
    if n < 20 {
        ONES[n as usize].into()
    } else {
        format!("{} {}", TENS[(n / 10) as usize], ONES[(n % 10) as usize])
            .trim_end()
            .into()
    }
}

fn number_words(n: u64) -> String {
    let mut parts = Vec::new();
    if n >= 10_000_000 {
        parts.push(format!("{} Crore", number_words(n / 10_000_000)));
    }
    for (value, name) in [(100_000, "Lakh"), (1_000, "Thousand")] {
        let count = (n % (value * 100)) / value;
        if count > 0 {
            parts.push(format!("{} {name}", under_hundred(count)));
        }
    }
    let hundreds = (n % 1_000) / 100;
    if hundreds > 0 {
        parts.push(format!("{} Hundred", under_hundred(hundreds)));
    }
    if !n.is_multiple_of(100) {
        parts.push(under_hundred(n % 100));
    }
    parts.join(" ")
}

/// "Rupees One Thousand Five Hundred Five and Fifty Paise only", as claim forms ask.
pub fn words(minor: i64) -> String {
    let value = minor.unsigned_abs();
    let rupees = match value / 100 {
        0 => "Zero".into(),
        n => number_words(n),
    };
    match value % 100 {
        0 => format!("Rupees {rupees} only"),
        p => format!("Rupees {rupees} and {} Paise only", under_hundred(p)),
    }
}

/// Reads "1,234.5", "₹ 1234" and the like.
pub fn parse(text: &str) -> Option<i64> {
    let cleaned: String = text
        .trim()
        .trim_start_matches('₹')
        .trim()
        .chars()
        .filter(|c| *c != ',')
        .collect();
    let (whole, frac) = cleaned.split_once('.').unwrap_or((&cleaned, ""));
    if whole.is_empty() || frac.len() > 2 {
        return None;
    }
    if !whole
        .chars()
        .chain(frac.chars())
        .all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let paise: i64 = format!("{frac:0<2}").parse().ok()?;
    whole
        .parse::<i64>()
        .ok()?
        .checked_mul(100)?
        .checked_add(paise)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_without_losing_paise() {
        assert_eq!(split(100, 3), vec![34, 33, 33]);
        assert_eq!(split(100, 3).iter().sum::<i64>(), 100);
    }

    #[test]
    fn converts_foreign_amounts() {
        // 10.00 USD at 88.4567
        assert_eq!(convert(1_000, 88_456_700), 88_457);
    }

    #[test]
    fn writes_indian_grouping_and_words() {
        assert_eq!(indian(1_234_567_890), "1,23,45,678.90");
        assert_eq!(indian(100_000), "1,000.00");
        assert_eq!(indian(-5), "-0.05");
        assert_eq!(
            words(150_550),
            "Rupees One Thousand Five Hundred Five and Fifty Paise only"
        );
        assert_eq!(
            words(12_100_015 * 100),
            "Rupees One Crore Twenty One Lakh Fifteen only"
        );
        assert_eq!(words(0), "Rupees Zero only");
    }

    #[test]
    fn parses_typed_amounts() {
        assert_eq!(parse("1,234.5"), Some(123_450));
        assert_eq!(parse("₹ 80"), Some(8_000));
        assert_eq!(parse("-5"), None);
        assert_eq!(parse("1.234"), None);
        assert_eq!(parse(""), None);
    }
}
