//! Reads typed lines such as "cab home to station 180" into draft entries.
//! Plain rules over words and numbers. It only drafts: the person checks what it
//! understood before anything is saved.

use chrono::{Datelike, Duration, NaiveDate};
use serde::Serialize;

use crate::money;

/// What the line is read against.
pub struct Context<'a> {
    pub today: NaiveDate,
    /// Start and end of the chosen tour.
    pub tour_dates: Option<(NaiveDate, NaiveDate)>,
    pub company_tour: bool,
    /// (id, name) of saved people.
    pub people: &'a [(String, String)],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DraftKind {
    Expense,
    Advance,
    AdvanceReturned,
    Reimbursement,
    Allowance,
}

#[derive(Debug, Clone, Serialize)]
pub struct Draft {
    /// The line as typed.
    pub line: String,
    pub kind: DraftKind,
    pub amount: i64,
    pub date: String,
    /// The line named no date, so one was chosen.
    pub date_assumed: bool,
    pub category: String,
    pub description: String,
    pub from: String,
    pub to: String,
    pub mode: String,
    pub paid_by: String,
    pub claim: String,
    pub split_with: Vec<String>,
    pub bill_no: String,
    /// Things to check before saving.
    pub warnings: Vec<String>,
    /// Set when the line cannot be saved as it is.
    pub problem: String,
}

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

const CATEGORY_WORDS: [(&str, &str); 12] = [
    (
        "food",
        "breakfast lunch dinner food meal meals thali tiffin snacks snack refreshment refreshments nashta khana tea chai coffee milk lassi juice shake water pani drink drinks restaurant dhaba canteen pantry biscuit biscuits samosa kachori pakora pakoda vada poha upma idli dosa uttapam paratha roti chapati puri rice dal biryani pulao noodles maggi momo momos sandwich burger pizza omelette egg chicken fish paneer roll fruit fruits banana apple chips namkeen sweets mithai mishti icecream kulfi soda cake bun toast cutlet",
    ),
    (
        "lodging",
        "hotel lodge lodging room stay hostel homestay dormitory retiring",
    ),
    ("train", "train rail railway irctc tatkal platform"),
    ("flight", "flight airfare airline boarding baggage"),
    ("fuel", "petrol diesel fuel toll parking cng"),
    ("bus", "bus volvo"),
    (
        "local",
        "cab taxi auto uber ola rapido rickshaw toto erickshaw e-rickshaw bike ferry tram coolie porter",
    ),
    ("metro", "metro"),
    (
        "event",
        "entry pass passes stall registration exhibition fair conference",
    ),
    ("phone", "recharge internet sim wifi data"),
    (
        "medical",
        "medicine medicines doctor medical pharmacy tablet",
    ),
    ("shopping", "shopping gift gifts souvenir"),
];

const MODES: [(&str, &str); 14] = [
    ("auto", "Auto"),
    ("cab", "Cab"),
    ("taxi", "Cab"),
    ("uber", "Cab"),
    ("ola", "Cab"),
    ("rapido", "Bike taxi"),
    ("rickshaw", "Rickshaw"),
    ("e-rickshaw", "E-rickshaw"),
    ("erickshaw", "E-rickshaw"),
    ("toto", "E-rickshaw"),
    ("bus", "Bus"),
    ("train", "Train"),
    ("flight", "Flight"),
    // Last: "cab to metro" is a cab ride to the station.
    ("metro", "Metro"),
];

const TRANSPORT: [&str; 6] = ["local", "metro", "train", "flight", "bus", "fuel"];

#[derive(Clone)]
struct Word {
    /// As typed, without surrounding punctuation.
    raw: String,
    low: String,
    used: bool,
}

fn words(line: &str) -> Vec<Word> {
    line.replace('→', " to ")
        .split_whitespace()
        .filter_map(|token| {
            let raw = token
                .trim_matches(|c: char| !(c.is_alphanumeric() || c == '₹'))
                .to_owned();
            (!raw.is_empty()).then(|| Word {
                low: raw.to_lowercase().replace(['\'', '’'], ""),
                raw,
                used: false,
            })
        })
        .collect()
}

fn is(words: &[Word], at: usize, wanted: &[&str]) -> bool {
    words
        .get(at)
        .is_some_and(|w| !w.used && wanted.contains(&w.low.as_str()))
}

fn take(words: &mut [Word], from: usize, count: usize) {
    words
        .iter_mut()
        .skip(from)
        .take(count)
        .for_each(|w| w.used = true);
}

/// Removes the first run of words matching any phrase.
fn phrase(words: &mut [Word], phrases: &[&str]) -> bool {
    for wanted in phrases {
        let parts: Vec<&str> = wanted.split(' ').collect();
        for at in 0..words.len() {
            if parts
                .iter()
                .enumerate()
                .all(|(i, p)| is(words, at + i, &[p]))
            {
                take(words, at, parts.len());
                return true;
            }
        }
    }
    false
}

fn month_number(word: &str) -> Option<u32> {
    let short = word.get(..3)?;
    let full = [
        "january",
        "february",
        "march",
        "april",
        "may",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
    ];
    let index = MONTHS.iter().position(|m| *m == short)?;
    (word == short || word == "sept" || full[index] == word).then_some(index as u32 + 1)
}

/// "15" or "15th".
fn day_number(word: &str) -> Option<u32> {
    let digits = ["st", "nd", "rd", "th"]
        .iter()
        .find_map(|s| word.strip_suffix(s))
        .unwrap_or(word);
    (digits.len() <= 2)
        .then(|| digits.parse().ok())
        .flatten()
        .filter(|d| (1..=31).contains(d))
}

/// The year that puts a day and month nearest to what is being recorded.
fn with_year(day: u32, month: u32, context: &Context) -> Option<NaiveDate> {
    let anchor = context.tour_dates.map_or(context.today, |(start, _)| start);
    [anchor.year() - 1, anchor.year(), anchor.year() + 1]
        .into_iter()
        .filter_map(|y| NaiveDate::from_ymd_opt(y, month, day))
        .min_by_key(|d| (*d - anchor).num_days().abs())
}

fn numeric_date(word: &str, context: &Context) -> Option<NaiveDate> {
    let parts: Vec<&str> = word.split(['/', '-', '.']).collect();
    let numbers: Option<Vec<u32>> = parts
        .iter()
        .map(|p| {
            p.chars()
                .all(|c| c.is_ascii_digit())
                .then(|| p.parse().ok())
                .flatten()
        })
        .collect();
    let numbers = numbers?;
    match (numbers.as_slice(), word.contains('.')) {
        // A dot alone is a decimal amount, not a date.
        ([d, m], false) => with_year(*d, *m, context),
        ([d, m, y], _) => {
            let year = if *y < 100 { 2000 + *y } else { *y };
            NaiveDate::from_ymd_opt(i32::try_from(year).ok()?, *m, *d)
        }
        _ => None,
    }
}

fn find_date(words: &mut [Word], context: &Context) -> Option<NaiveDate> {
    if phrase(words, &["day before yesterday"]) {
        return Some(context.today - Duration::days(2));
    }
    if phrase(words, &["yesterday"]) {
        return Some(context.today - Duration::days(1));
    }
    if phrase(words, &["today"]) {
        return Some(context.today);
    }
    for at in 0..words.len() {
        if words[at].used {
            continue;
        }
        let low = words[at].low.clone();
        let next = words.get(at + 1).map(|w| w.low.clone()).unwrap_or_default();
        // "15 nov" or "nov 15"
        let pair = day_number(&low)
            .zip(month_number(&next))
            .or_else(|| day_number(&next).zip(month_number(&low)));
        let year = words
            .get(at + 2)
            .filter(|w| !w.used && w.low.len() == 4)
            .and_then(|w| w.low.parse::<i32>().ok())
            .filter(|y| (2000..=2100).contains(y));
        if let Some((day, month)) = pair
            && let Some(date) = match year {
                Some(year) => NaiveDate::from_ymd_opt(year, month, day),
                None => with_year(day, month, context),
            }
        {
            take(words, at, 2 + usize::from(year.is_some()));
            if at > 0 && is(words, at - 1, &["on"]) {
                take(words, at - 1, 1);
            }
            return Some(date);
        }
        if let Some(date) = numeric_date(&low, context) {
            take(words, at, 1);
            if at > 0 && is(words, at - 1, &["on"]) {
                take(words, at - 1, 1);
            }
            return Some(date);
        }
    }
    // "on 15th": a day within the tour's month. Tried last, so that a named
    // month ("on 14 Nov") wins.
    for at in 0..words.len() {
        if is(words, at, &["on"])
            && let Some(day) = words.get(at + 1).and_then(|w| day_number(&w.low))
        {
            let anchor = context.tour_dates.map_or(context.today, |(start, _)| start);
            if let Some(date) = NaiveDate::from_ymd_opt(anchor.year(), anchor.month(), day) {
                take(words, at, 2);
                return Some(date);
            }
        }
    }
    None
}

fn amount_of(word: &str) -> Option<(i64, bool)> {
    let mut text = word;
    let mut marked = false;
    for marker in ["₹", "rs.", "rs", "inr"] {
        if let Some(rest) = text.strip_prefix(marker) {
            text = rest;
            marked = true;
            break;
        }
    }
    for suffix in ["/-", "rs", "₹"] {
        if let Some(rest) = text.strip_suffix(suffix) {
            text = rest;
            marked = true;
        }
    }
    let mut times = 1;
    for (suffix, factor) in [
        ("lakhs", 100_000),
        ("lakh", 100_000),
        ("lac", 100_000),
        ("k", 1_000),
    ] {
        if let Some(rest) = text.strip_suffix(suffix) {
            text = rest;
            times = factor;
            break;
        }
    }
    let value = money::parse(text.trim_start_matches('.'))?.checked_mul(times)?;
    (value > 0 && value <= money::MAX_AMOUNT).then_some((value, marked))
}

/// "2 thousand", "1.5 lakh": the word after a number that multiplies it.
fn multiplier(word: &str) -> Option<i64> {
    match word {
        "thousand" | "thousands" | "k" => Some(1_000),
        "lakh" | "lakhs" | "lac" | "lacs" => Some(100_000),
        _ => None,
    }
}

/// The amount, and a warning when more than one number could have been it.
fn find_amount(words: &mut [Word]) -> (i64, Option<String>) {
    let mut found: Vec<(usize, i64, bool)> = Vec::new();
    for at in 0..words.len() {
        if words[at].used {
            continue;
        }
        let Some((mut value, mut marked)) = amount_of(&words[at].low) else {
            continue;
        };
        if let Some(times) = words.get(at + 1).and_then(|w| multiplier(&w.low)) {
            value = value.saturating_mul(times).min(money::MAX_AMOUNT);
            words[at + 1].used = true;
        }
        if at > 0 && is(words, at - 1, &["rs", "inr", "₹", "rs."]) {
            marked = true;
        }
        if is(words, at + 1, &["rs", "rupees", "inr"]) {
            marked = true;
        }
        found.push((at, value, marked));
    }
    let marked_only: Vec<_> = found.iter().copied().filter(|f| f.2).collect();
    let pool = if marked_only.is_empty() {
        &found
    } else {
        &marked_only
    };
    // The first marked amount (a bank message puts the balance later), else the
    // largest number: counts like "3 nights" are smaller than their price.
    let chosen = if marked_only.is_empty() {
        pool.iter().copied().max_by_key(|f| f.1)
    } else {
        pool.first().copied()
    };
    let Some((at, value, _)) = chosen else {
        return (0, None);
    };
    take(words, at, 1);
    if at > 0 && is(words, at - 1, &["rs", "inr", "₹", "rs."]) {
        take(words, at - 1, 1);
    }
    if is(words, at + 1, &["rs", "rupees", "inr"]) {
        take(words, at + 1, 1);
    }
    let warning = (pool.len() > 1).then(|| {
        format!(
            "More than one number; used {}.",
            money::indian(value).trim_end_matches(".00")
        )
    });
    (value, warning)
}

/// A saved person named at this word: any full name first, then a first name.
fn person_at<'a>(words: &[Word], at: usize, context: &'a Context) -> Option<(&'a str, usize)> {
    let parts = |name: &str| -> Vec<String> {
        name.to_lowercase()
            .split_whitespace()
            .map(String::from)
            .collect()
    };
    let full = context.people.iter().find_map(|(id, name)| {
        let parts = parts(name);
        (!parts.is_empty()
            && parts
                .iter()
                .enumerate()
                .all(|(i, p)| is(words, at + i, &[p])))
        .then_some((id.as_str(), parts.len()))
    });
    full.or_else(|| {
        context.people.iter().find_map(|(id, name)| {
            parts(name)
                .first()
                .filter(|first| is(words, at, &[first]))
                .map(|_| (id.as_str(), 1))
        })
    })
}

/// True when only a first name was typed and two saved people share it.
fn shared_first_name(word: &str, context: &Context) -> bool {
    context
        .people
        .iter()
        .filter(|(_, name)| name.to_lowercase().split_whitespace().next() == Some(word))
        .count()
        > 1
}

fn find_payer(words: &mut [Word], context: &Context, warnings: &mut Vec<String>) -> String {
    if phrase(
        words,
        &[
            "paid by the company",
            "paid by the office",
            "paid by company",
            "company paid",
            "by the company",
            "by company",
            "booked by the company",
            "booked by company",
            "company booked",
            "paid by office",
            "office paid",
        ],
    ) {
        return "company".into();
    }
    if phrase(words, &["paid by me", "i paid", "by me", "self paid"]) {
        return "me".into();
    }
    for at in 0..words.len() {
        // "Ravi paid"
        if let Some((id, len)) = person_at(words, at, context)
            && is(words, at + len, &["paid"])
        {
            if len == 1 && shared_first_name(&words[at].low, context) {
                warnings.push("Two people share that name. Check who paid.".into());
            }
            take(words, at, len + 1);
            return id.to_owned();
        }
        // "paid by Ravi"
        if is(words, at, &["paid"])
            && is(words, at + 1, &["by"])
            && let Some((id, len)) = person_at(words, at + 2, context)
        {
            if len == 1 && shared_first_name(&words[at + 2].low, context) {
                warnings.push("Two people share that name. Check who paid.".into());
            }
            take(words, at, len + 2);
            return id.to_owned();
        }
    }
    "me".into()
}

fn in_list(list: &str, word: &str) -> bool {
    list.split(' ').any(|w| w == word)
}

/// The category these words belong to, by the first list that has one.
fn category_among<'a>(words: impl Iterator<Item = &'a str> + Clone) -> Option<&'static str> {
    CATEGORY_WORDS
        .iter()
        .find(|(_, list)| words.clone().any(|w| in_list(list, w)))
        .map(|(id, _)| *id)
}

/// The category a note such as "powder tea in train" belongs to, if any word
/// says. A vehicle with a "to" is a ride, whatever it passes by.
pub fn categorise(text: &str) -> Option<&'static str> {
    let low: Vec<String> = words(text)
        .into_iter()
        .map(|w| w.low.replace("ice-cream", "icecream"))
        .collect();
    let has = |word: &str| low.iter().any(|w| w == word);
    if has("to")
        && let Some(category) = ride_category(mode_among(&low))
    {
        return Some(category);
    }
    category_among(low.iter().map(String::as_str))
}

/// The vehicle a line names, preferring anything over the metro.
fn mode_among(words: &[String]) -> &'static str {
    MODES
        .iter()
        .find(|(word, _)| words.iter().any(|w| w == word))
        .map_or("", |(_, name)| name)
}

/// The category of a trip from one place to another by this vehicle. Trains,
/// flights and buses are judged by their own words.
fn ride_category(mode: &str) -> Option<&'static str> {
    match mode {
        "" | "Train" | "Flight" | "Bus" => None,
        "Metro" => Some("metro"),
        _ => Some("local"),
    }
}

/// A name that paid but is not saved. Reading it as "me" would claim money
/// someone else spent.
fn unknown_payer(words: &[Word]) -> Option<String> {
    let ordinary = |word: &str| {
        CATEGORY_WORDS.iter().any(|(_, list)| in_list(list, word))
            || MODES.iter().any(|(m, _)| *m == word)
            || [
                "i", "me", "we", "self", "fare", "ticket", "tickets", "bill", "amount", "cash",
                "already", "also", "was", "advance", "fee", "fees", "charge", "charges",
            ]
            .contains(&word)
    };
    (0..words.len()).find_map(|at| {
        if !is(words, at, &["paid"]) {
            return None;
        }
        let name = if is(words, at + 1, &["by"]) {
            words.get(at + 2)
        } else {
            at.checked_sub(1).and_then(|i| words.get(i))
        }?;
        let looks_like_name = !name.used
            && name.raw.chars().next().is_some_and(char::is_uppercase)
            && name.raw.chars().all(char::is_alphabetic)
            && !ordinary(&name.low);
        looks_like_name.then(|| name.raw.clone())
    })
}

/// "with Ravi", "for me and Meera".
fn find_sharers(words: &mut [Word], context: &Context) -> Vec<String> {
    let mut sharers: Vec<String> = Vec::new();
    for at in 0..words.len() {
        let with = is(words, at, &["with"]);
        if !with && !is(words, at, &["for"]) {
            continue;
        }
        let mut next = at + 1;
        let mut found = Vec::new();
        loop {
            if is(words, next, &["and", "&"]) {
                next += 1;
            } else if is(words, next, &["me", "myself"]) {
                found.push("me".to_owned());
                next += 1;
            } else if let Some((id, len)) = person_at(words, next, context) {
                found.push(id.to_owned());
                next += len;
            } else {
                break;
            }
        }
        if !found.is_empty() {
            take(words, at, next - at);
            // "with Ravi" means Ravi and me.
            if with {
                found.push("me".to_owned());
            }
            sharers = found;
            break;
        }
    }
    sharers.sort();
    sharers.dedup();
    sharers
}

fn capitalise(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

fn joined(words: &[&Word]) -> String {
    words
        .iter()
        .map(|w| w.raw.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/// "home to station" among the words left over.
fn find_route(words: &mut [Word]) -> (String, String) {
    let free: Vec<usize> = (0..words.len()).filter(|i| !words[*i].used).collect();
    let Some(to_at) = free.iter().position(|i| words[*i].low == "to") else {
        return (String::new(), String::new());
    };
    let skip = |low: &str| {
        MODES.iter().any(|(m, _)| *m == low)
            || [
                "from", "by", "fare", "ticket", "tickets", "ride", "paid", "for", "claim",
            ]
            .contains(&low)
    };
    let before: Vec<usize> = free[..to_at]
        .iter()
        .copied()
        .rev()
        .take_while(|i| !skip(&words[*i].low))
        .collect();
    let after: Vec<usize> = free[to_at + 1..]
        .iter()
        .copied()
        .take_while(|i| !skip(&words[*i].low))
        .collect();
    if before.is_empty() || after.is_empty() {
        return (String::new(), String::new());
    }
    let from: Vec<&Word> = before.iter().rev().map(|i| &words[*i]).collect();
    let to: Vec<&Word> = after.iter().map(|i| &words[*i]).collect();
    let route = (capitalise(&joined(&from)), capitalise(&joined(&to)));
    for i in before.into_iter().chain(after).chain([free[to_at]]) {
        words[i].used = true;
    }
    phrase(words, &["from"]);
    route
}

/// Words that can appear in a line that is only money between me and the
/// company. Any other word means the line is about buying something.
const MONEY_WORDS: [&str; 38] = [
    "advance",
    "received",
    "receive",
    "got",
    "returned",
    "return",
    "back",
    "from",
    "to",
    "the",
    "company",
    "office",
    "employer",
    "gave",
    "given",
    "cash",
    "in",
    "hand",
    "reimbursement",
    "reimbursed",
    "claim",
    "amount",
    "of",
    "by",
    "me",
    "my",
    "as",
    "for",
    "tour",
    "trip",
    "bonus",
    "allowance",
    "i",
    "rupees",
    "on",
    "was",
    "paid",
    "us",
];

/// Decides from the words, wherever the amount stood among them.
fn company_money(words: &mut [Word], warnings: &mut Vec<String>) -> DraftKind {
    let free: Vec<&str> = words
        .iter()
        .filter(|w| !w.used)
        .map(|w| w.low.as_str())
        .collect();
    let any = |list: &[&str]| free.iter().any(|w| list.contains(w));
    let bonus = any(&["bonus", "allowance"]);
    let advance = any(&["advance"]);
    let reimbursed = any(&["reimbursement", "reimbursed"]);
    let returned = any(&["returned", "return"]);
    let from_company = any(&["company", "office", "employer"]);
    let received = any(&["received", "receive", "got", "gave", "given"]);
    let in_hand = any(&["hand"]) && any(&["cash", "in"]);

    let only_money = free.iter().all(|w| MONEY_WORDS.contains(w));
    // "paid advance 2000" is money going out to someone.
    let paid_first = free.first() == Some(&"paid");
    let kind = if !only_money || paid_first {
        None
    } else if returned && (advance || from_company || any(&["cash", "back"])) {
        Some(DraftKind::AdvanceReturned)
    } else if bonus && (from_company || received) {
        Some(DraftKind::Allowance)
    } else if reimbursed {
        Some(DraftKind::Reimbursement)
    } else if advance || in_hand || (received && (from_company || any(&["cash"]))) {
        Some(DraftKind::Advance)
    } else {
        None
    };
    match kind {
        Some(kind) => {
            words.iter_mut().for_each(|w| w.used = true);
            kind
        }
        None => {
            if advance || returned || (from_company && received) || (paid_first && reimbursed) {
                warnings.push(
                    "Read as an expense. If this is money to or from the company, change it."
                        .into(),
                );
            }
            DraftKind::Expense
        }
    }
}

pub fn parse_line(line: &str, context: &Context) -> Draft {
    let mut w = words(line);
    let mut warnings = Vec::new();

    let mut bill_no = String::new();
    for at in 0..w.len() {
        if is(&w, at, &["bill", "invoice", "receipt"]) {
            let mut next = at + 1;
            if is(&w, next, &["no", "number"]) {
                next += 1;
            }
            if w.get(next)
                .is_some_and(|x| x.low.chars().any(|c| c.is_ascii_digit()))
                && next > at + 1
            {
                bill_no = w[next].raw.clone();
                take(&mut w, at, next - at + 1);
                break;
            }
        }
    }

    let date = find_date(&mut w, context);
    let date_assumed = date.is_none();
    let date = date.unwrap_or_else(|| match context.tour_dates {
        Some((start, _)) if context.today < start => start,
        Some((_, end)) if context.today > end => end,
        _ => context.today,
    });
    if let Some((start, end)) = context.tour_dates
        && !date_assumed
        && (date < start - Duration::days(7) || date > end + Duration::days(7))
    {
        warnings.push("This date is outside the tour.".to_owned());
    }

    let (amount, amount_warning) = find_amount(&mut w);
    warnings.extend(amount_warning);

    let kind = company_money(&mut w, &mut warnings);

    let paid_by = if kind == DraftKind::Expense {
        find_payer(&mut w, context, &mut warnings)
    } else {
        "me".into()
    };
    let split_with = find_sharers(&mut w, context);

    let said_personal = phrase(
        &mut w,
        &[
            "not to claim",
            "not to be claimed",
            "dont claim",
            "do not claim",
            "not claim",
            "not claimable",
            "not claimed",
            "not reimbursable",
            "non claimable",
            "non-claimable",
            "non reimbursable",
            "non-reimbursable",
            "no claim",
            "my own",
            "own expense",
            "my expense",
            "self expense",
            "personal",
        ],
    );
    let said_claim = phrase(
        &mut w,
        &[
            "to be reimbursed",
            "for reimbursement",
            "to claim",
            "claimable",
            "reimbursable",
            "reimbursed",
            "reimbursement",
            "claim",
        ],
    );
    let claim = if said_personal {
        "personal"
    } else if said_claim || context.company_tour {
        "reimbursable"
    } else {
        "personal"
    };

    // The category is judged before the route is taken out, so that
    // "Hotel to station" by cab stays a cab ride.
    let free: Vec<String> = w
        .iter()
        .filter(|x| !x.used)
        .map(|x| x.low.clone())
        .collect();
    let mode = mode_among(&free);
    let category = ride_category(mode)
        .filter(|_| free.iter().any(|f| f == "to"))
        .unwrap_or_else(|| category_among(free.iter().map(String::as_str)).unwrap_or("other"));
    let transport = TRANSPORT.contains(&category);
    let (from, to) = if transport || category == "other" {
        find_route(&mut w)
    } else {
        (String::new(), String::new())
    };
    // "home to station 180" with no vehicle named is still local travel.
    let category = if category == "other" && !from.is_empty() {
        "local"
    } else {
        category
    };
    let mode = if TRANSPORT.contains(&category) {
        mode
    } else {
        ""
    };

    let mut left: Vec<&Word> = w
        .iter()
        .filter(|x| !x.used)
        .filter(|x| !(!mode.is_empty() && MODES.iter().any(|(m, _)| *m == x.low)))
        .collect();
    let filler = [
        "paid", "for", "of", "the", "rs", "by", "on", "spent", "fare", "from", "received",
        "company", "cash", "₹",
    ];
    while left
        .first()
        .is_some_and(|x| filler.contains(&x.low.as_str()))
    {
        left.remove(0);
    }
    while left
        .last()
        .is_some_and(|x| filler.contains(&x.low.as_str()))
    {
        left.pop();
    }
    let description = capitalise(&joined(&left));

    let problem = if amount == 0 {
        "No amount found. Add the rupees spent.".to_owned()
    } else if kind == DraftKind::Expense
        && paid_by == "me"
        && let Some(name) = unknown_payer(&words(line))
        && !context
            .people
            .iter()
            .any(|(_, n)| n.to_lowercase().contains(&name.to_lowercase()))
    {
        format!("{name} is not saved. Add them under More → People, or write \"I paid\".")
    } else {
        String::new()
    };

    Draft {
        line: line.trim().to_owned(),
        kind,
        amount,
        date: date.format("%Y-%m-%d").to_string(),
        date_assumed,
        category: category.to_owned(),
        description,
        from,
        to,
        mode: mode.to_owned(),
        paid_by,
        claim: claim.to_owned(),
        split_with,
        bill_no,
        warnings,
        problem,
    }
}

/// One draft for each line that has anything on it.
pub fn parse(text: &str, context: &Context) -> Vec<Draft> {
    lines(text).map(|line| parse_line(line, context)).collect()
}

pub fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines()
        .flat_map(|line| line.split(';'))
        .filter(|line| !line.trim().is_empty())
}

pub const MAX_LINES: usize = 100;

#[cfg(test)]
mod tests {
    use super::*;

    fn date(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    fn read(line: &str) -> Draft {
        let people = [
            ("r".to_owned(), "Ravi Kumar".to_owned()),
            ("m".to_owned(), "Meera".to_owned()),
        ];
        parse_line(
            line,
            &Context {
                today: date("2026-11-27"),
                tour_dates: Some((date("2026-11-14"), date("2026-11-23"))),
                company_tour: true,
                people: &people,
            },
        )
    }

    #[test]
    fn a_typed_metro_ride_is_metro_and_a_ride_to_the_metro_is_not() {
        for (line, category, mode) in [
            ("metro home to station 40", "metro", "Metro"),
            ("metro 60", "metro", "Metro"),
            ("cab home to metro 120", "local", "Cab"),
            ("auto to metro station 80", "local", "Auto"),
            ("rickshaw home to metro 20", "local", "Rickshaw"),
        ] {
            let d = read(line);
            assert_eq!(
                (d.category.as_str(), d.mode.as_str()),
                (category, mode),
                "{line}"
            );
        }
    }

    #[test]
    fn a_ride_with_a_route() {
        let d = read("cab home to Howrah station 180 on 14 Nov");
        assert_eq!(
            (d.amount, d.category.as_str(), d.mode.as_str()),
            (18_000, "local", "Cab")
        );
        assert_eq!((d.from.as_str(), d.to.as_str()), ("Home", "Howrah station"));
        assert_eq!((d.date.as_str(), d.date_assumed), ("2026-11-14", false));
        assert_eq!(
            (d.paid_by.as_str(), d.claim.as_str()),
            ("me", "reimbursable")
        );
        assert!(d.problem.is_empty() && d.warnings.is_empty(), "{d:?}");
    }

    #[test]
    fn a_ride_from_a_hotel_is_travel_not_lodging() {
        let d = read("auto from hotel to Bharat Mandapam 120");
        assert_eq!(d.category, "local");
        assert_eq!(
            (d.from.as_str(), d.to.as_str()),
            ("Hotel", "Bharat Mandapam")
        );
        // No date given: the tour is over, so its last day is assumed.
        assert_eq!((d.date.as_str(), d.date_assumed), ("2026-11-23", true));
    }

    #[test]
    fn food_shared_with_a_colleague() {
        let d = read("lunch with Ravi ₹640 15/11");
        assert_eq!((d.amount, d.category.as_str()), (64_000, "food"));
        assert_eq!(d.split_with, ["me", "r"]);
        assert_eq!(
            (d.date.as_str(), d.description.as_str()),
            ("2026-11-15", "Lunch")
        );
    }

    #[test]
    fn who_paid() {
        assert_eq!(read("hotel 3 nights 9600 company paid").paid_by, "company");
        assert_eq!(read("hotel 3 nights 9600 company paid").amount, 960_000);
        assert_eq!(read("Ravi paid dinner 1500").paid_by, "r");
        assert_eq!(read("tea 40 paid by Meera").paid_by, "m");
        assert_eq!(read("shopping 900 personal").claim, "personal");
    }

    #[test]
    fn money_to_and_from_the_company() {
        let d = read("company gave 2000 cash on 13 nov");
        assert_eq!((d.kind, d.amount), (DraftKind::Advance, 200_000));
        assert_eq!(read("advance 2000").kind, DraftKind::Advance);
        assert_eq!(
            read("reimbursement received 1505.50").kind,
            DraftKind::Reimbursement
        );
        assert_eq!(
            read("returned advance 300").kind,
            DraftKind::AdvanceReturned
        );
    }

    #[test]
    fn company_money_is_read_whatever_the_word_order() {
        use DraftKind::*;
        for (line, kind, amount) in [
            ("returned 300 advance", AdvanceReturned, 30_000),
            ("advance 300 returned", AdvanceReturned, 30_000),
            ("returned 300 to company", AdvanceReturned, 30_000),
            ("received 2000 from company", Advance, 200_000),
            ("got 2000 from office", Advance, 200_000),
            ("company gave 2 thousand", Advance, 200_000),
            ("advance 1.5 lakh", Advance, 15_000_000),
            ("company gave 5000 bonus", Allowance, 500_000),
        ] {
            let d = read(line);
            assert_eq!((d.kind, d.amount), (kind, amount), "{line}");
        }
    }

    #[test]
    fn buying_something_is_never_company_money() {
        for line in [
            "train ticket advance booking 850",
            "paid advance 2000 to hotel",
            "cash given to porter 50",
            "lunch 300 to be reimbursed",
            "cab 180 for reimbursement",
            "bonus trip taxi 200",
        ] {
            assert_eq!(read(line).kind, DraftKind::Expense, "{line}");
        }
        assert!(!read("paid advance 2000 to hotel").warnings.is_empty());
        assert_eq!(read("lunch 300 to be reimbursed").claim, "reimbursable");
    }

    #[test]
    fn refusing_a_claim_is_honoured() {
        for line in [
            "shopping 900 not to claim",
            "shopping 900 don't claim",
            "shopping 900 not reimbursable",
            "shopping 900 non-claimable",
            "shopping 900 my expense",
        ] {
            assert_eq!(read(line).claim, "personal", "{line}");
        }
    }

    #[test]
    fn who_paid_is_never_guessed() {
        assert_eq!(read("hotel 9600 paid by the company").paid_by, "company");
        assert!(read("Amit paid dinner 600").problem.contains("Amit"));
        assert!(read("dinner 600 paid by Amit").problem.contains("Amit"));
        assert!(read("Auto paid 60").problem.is_empty());
        let people = [
            ("k".to_owned(), "Ravi Kumar".to_owned()),
            ("s".to_owned(), "Ravi Sharma".to_owned()),
        ];
        let context = Context {
            today: date("2026-11-27"),
            tour_dates: None,
            company_tour: true,
            people: &people,
        };
        assert_eq!(parse_line("Ravi Sharma paid 90", &context).paid_by, "s");
        assert!(!parse_line("Ravi paid 90", &context).warnings.is_empty());
    }

    #[test]
    fn amounts_in_thousands_and_dates_with_years() {
        assert_eq!(read("hotel 2 nights 5k").amount, 500_000);
        let d = read("lunch 300 on 14 Nov 2026");
        assert_eq!((d.amount, d.date.as_str()), (30_000, "2026-11-14"));
        assert!(d.warnings.is_empty());
    }

    #[test]
    fn small_food_and_drink_on_the_way_is_food() {
        for line in [
            "powder tea 20",
            "chai 10",
            "ready made chai 10 in train",
            "water bottle 20 at platform",
            "coffee on flight 250",
            "2 samosa 30",
            "biscuit 10",
            "nashta 60",
        ] {
            let d = read(line);
            assert_eq!(d.category, "food", "{line}: {d:?}");
        }
        assert_eq!(read("tea 20 on 12 nov").date, "2026-11-12");
        assert_eq!(categorise("Powder tea inside train"), Some("food"));
        assert_eq!(categorise("auto home to station"), Some("local"));
        assert_eq!(categorise("coolie"), Some("local"));
        assert_eq!(categorise("metro"), Some("metro"));
        assert_eq!(categorise("metro home to station"), Some("metro"));
        // The metro as the destination of another ride.
        assert_eq!(categorise("cab to metro"), Some("local"));
        assert_eq!(categorise("auto home to metro station"), Some("local"));
        assert_eq!(categorise("cab metro station"), Some("local"));
        assert_eq!(categorise("newspaper"), None);
    }

    #[test]
    fn says_when_it_is_unsure() {
        assert!(!read("lunch").problem.is_empty());
        assert_eq!(read("2 coffees 60").amount, 6_000);
        assert!(!read("2 coffees 60").warnings.is_empty());
        let d = read("stationery bill no 4521 240");
        assert_eq!((d.amount, d.bill_no.as_str()), (24_000, "4521"));
        assert!(!read("tea 20 on 3 jan").warnings.is_empty());
    }

    #[test]
    fn one_draft_per_line() {
        let context = Context {
            today: date("2026-11-27"),
            tour_dates: None,
            company_tour: false,
            people: &[],
        };
        let drafts = parse("auto 50 yesterday\n\nsnacks 30; metro 60", &context);
        assert_eq!(drafts.len(), 3);
        assert_eq!(drafts[0].date, "2026-11-26");
        assert_eq!(drafts[1].claim, "personal");
    }
}
