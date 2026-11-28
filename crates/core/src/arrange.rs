//! The order a tour's entries are listed in on a report: under each heading by
//! date, unless arranged by hand.
//!
//! The chosen order is kept on the tour, not on the entries, so that arranging
//! rewrites one record and never overwrites an entry edited on another device.

use std::collections::HashMap;

use serde::Serialize;

use crate::views::ExpenseView;

/// Heading, and the categories filed under it. Anything else goes under the last.
pub const HEADINGS: [(&str, &[&str]); 6] = [
    (
        "Travelling expenses",
        &["local", "metro", "train", "flight", "bus", "fuel"],
    ),
    ("Food and refreshments", &["food"]),
    ("Stay", &["lodging"]),
    ("Event and tickets", &["event"]),
    ("Event purchases", &["event_purchase"]),
    ("Miscellaneous", &["phone", "shopping", "medical", "other"]),
];

/// As many as one tour record can hold within the size allowed for it.
pub const MAX_ARRANGED: usize = 1000;

pub fn heading_of(category: &str) -> usize {
    HEADINGS
        .iter()
        .position(|(_, categories)| categories.contains(&category))
        .unwrap_or(HEADINGS.len() - 1)
}

#[derive(Debug, Clone, Serialize)]
pub struct Section {
    pub name: &'static str,
    pub ids: Vec<String>,
}

fn by_date(entries: &mut [&ExpenseView]) {
    entries.sort_by(|a, b| (&a.data.date, &a.id).cmp(&(&b.data.date, &b.id)));
}

/// Entries named in `order` keep that sequence. One added since is placed after
/// the last entry that is not dated later than it.
fn in_order<'a>(mut entries: Vec<&'a ExpenseView>, order: &[String]) -> Vec<&'a ExpenseView> {
    by_date(&mut entries);
    let rank: HashMap<&str, usize> = order
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();
    let (mut placed, rest): (Vec<_>, Vec<_>) = entries
        .into_iter()
        .partition(|e| rank.contains_key(e.id.as_str()));
    placed.sort_by_key(|e| rank[e.id.as_str()]);
    for entry in rest {
        let at = placed
            .iter()
            .rposition(|p| p.data.date <= entry.data.date)
            .map_or(0, |i| i + 1);
        placed.insert(at, entry);
    }
    placed
}

/// One tour's entries under the headings that have any, in the order to list them.
pub fn sections<'a>(
    entries: impl Iterator<Item = &'a ExpenseView>,
    order: &[String],
) -> Vec<(&'static str, Vec<&'a ExpenseView>)> {
    let mut found: [Vec<&ExpenseView>; HEADINGS.len()] = Default::default();
    for e in entries {
        found[heading_of(&e.data.category)].push(e);
    }
    HEADINGS
        .iter()
        .zip(found)
        .filter(|(_, list)| !list.is_empty())
        .map(|((name, _), list)| (*name, in_order(list, order)))
        .collect()
}

/// What to store for entries arranged as `wanted`. A heading left in date order
/// is not recorded, so it goes on following the dates.
pub fn to_store<'a>(
    entries: impl Iterator<Item = &'a ExpenseView> + Clone,
    wanted: &[String],
) -> Vec<String> {
    let plain = sections(entries.clone(), &[]);
    sections(entries, wanted)
        .into_iter()
        .zip(plain)
        .filter(|((_, arranged), (_, dated))| {
            arranged
                .iter()
                .map(|e| &e.id)
                .ne(dated.iter().map(|e| &e.id))
        })
        .flat_map(|((_, arranged), _)| arranged)
        .map(|e| e.id.clone())
        .take(MAX_ARRANGED)
        .collect()
}
