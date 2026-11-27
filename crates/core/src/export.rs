//! Spreadsheet and printable outputs, and CSV import.

use crate::error::{Error, Result};
use crate::model::{self, Claim, EXPENSE, Expense, ME};
use crate::money;
use crate::report::{Line, Report};
use crate::settlement::COMPANY_PAYER;
use crate::store::{Change, Store};
use crate::views::Data;

pub const CATEGORY_NAMES: [(&str, &str); 13] = [
    ("local", "Local transport"),
    ("metro", "Metro"),
    ("train", "Train"),
    ("flight", "Flight"),
    ("bus", "Bus"),
    ("food", "Food and refreshments"),
    ("lodging", "Lodging"),
    ("event", "Event and tickets"),
    ("fuel", "Fuel, parking, toll"),
    ("phone", "Phone and internet"),
    ("shopping", "Shopping"),
    ("medical", "Medical"),
    ("other", "Other"),
];

/// A category from a newer version has no name here and reads as "Other".
pub fn category_name(id: &str) -> &str {
    CATEGORY_NAMES
        .iter()
        .find(|(key, _)| *key == id)
        .map_or("Other", |(_, name)| name)
}

fn category_id(text: &str) -> &'static str {
    let mut wanted = text.trim().to_lowercase();
    // Files made before the heading was renamed still say "Food".
    if wanted == "food" {
        wanted = "food and refreshments".into();
    }
    CATEGORY_NAMES
        .iter()
        .find(|(id, name)| *id == wanted || name.to_lowercase() == wanted)
        .map_or("other", |(id, _)| id)
}

/// Stops a spreadsheet from running a cell as a formula.
fn safe_cell(text: &str) -> String {
    if text.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{text}")
    } else {
        text.to_owned()
    }
}

pub(crate) fn payer_name(data: &Data, id: &str) -> String {
    match id {
        ME => "Me".into(),
        COMPANY_PAYER => "Company".into(),
        _ => data
            .people
            .iter()
            .find(|p| p.id == id)
            .map_or_else(|| "Removed person".into(), |p| p.data.name.clone()),
    }
}

fn route(e: &Expense) -> String {
    match (e.from.is_empty(), e.to.is_empty()) {
        (true, true) => String::new(),
        _ => format!("{} → {}", e.from, e.to),
    }
}

/// Only what I paid can be claimed by me, whatever the entry says.
fn claim_label(e: &Expense) -> &'static str {
    match (e.paid_by.as_str(), e.claim, e.rejected) {
        (COMPANY_PAYER, _, _) => "Not claimed (company paid)",
        (ME, Claim::Reimbursable, true) => "Rejected",
        (ME, Claim::Reimbursable, false) => "Reimbursable",
        (ME, Claim::Personal, _) => "Personal",
        _ => "Not claimed (paid by another)",
    }
}

fn csv_text(rows: Vec<Vec<String>>) -> Result<String> {
    let mut writer = csv::WriterBuilder::new()
        .flexible(true)
        .from_writer(Vec::new());
    for row in rows {
        writer
            .write_record(&row)
            .map_err(|e| Error::msg(e.to_string()))?;
    }
    let bytes = writer.into_inner().map_err(|e| Error::msg(e.to_string()))?;
    // The mark makes Excel read the file as UTF-8.
    Ok(format!("\u{feff}{}", String::from_utf8_lossy(&bytes)))
}

const COLUMNS: [&str; 12] = [
    "Date",
    "Tour",
    "Category",
    "Description",
    "Route",
    "Mode",
    "Paid by",
    "Claim",
    "Amount (INR)",
    "Bill no",
    "Currency",
    "Original amount",
];

fn row(data: &Data, e: &Expense) -> Vec<String> {
    let tour = data
        .tours
        .iter()
        .find(|t| t.id == e.tour_id)
        .map_or("", |t| t.data.name.as_str());
    let foreign = !e.currency.is_empty() && e.currency != "INR";
    vec![
        e.date.clone(),
        safe_cell(tour),
        category_name(&e.category).to_owned(),
        safe_cell(&e.description),
        safe_cell(&route(e)),
        safe_cell(&e.mode),
        safe_cell(&payer_name(data, &e.paid_by)),
        claim_label(e).to_owned(),
        money::plain(e.home_amount()),
        safe_cell(&e.bill_no),
        if foreign {
            e.currency.clone()
        } else {
            String::new()
        },
        if foreign {
            money::plain(e.amount)
        } else {
            String::new()
        },
    ]
}

pub fn all_expenses_csv(data: &Data) -> Result<String> {
    let mut rows = vec![COLUMNS.map(String::from).to_vec()];
    rows.extend(data.expenses.iter().map(|e| row(data, &e.data)));
    csv_text(rows)
}

/// The report as a spreadsheet: a summary, then every line under its heading.
pub fn report_csv(report: &Report) -> Result<String> {
    let pair = |(label, value): &(String, String)| vec![label.clone(), safe_cell(value)];
    let total = |label: &str, value: i64| vec![label.to_owned(), money::plain(value)];
    let mut rows = vec![
        vec![report.title.clone()],
        vec!["Company".into(), safe_cell(&report.company)],
        vec!["Tour".into(), safe_cell(&report.tour)],
    ];
    rows.extend(report.claimant.iter().map(pair));
    rows.extend(report.details.iter().map(pair));
    rows.push(vec![]);
    rows.push(vec!["Category".into(), "Amount (INR)".into()]);
    rows.extend(
        report
            .groups
            .iter()
            .filter(|g| !g.lines.is_empty())
            .map(|g| total(&g.name, g.total)),
    );
    rows.push(total("Total", report.total));
    if report.detail {
        rows.push(vec![]);
        let header = [
            "Category",
            "Date",
            "Particulars",
            "For",
            "Paid by",
            "Bill no",
            "Amount (INR)",
            "Pictures",
        ];
        rows.push(header.map(String::from).to_vec());
        let line_row = |group: &str, l: &Line| {
            vec![
                group.to_owned(),
                l.date.clone(),
                safe_cell(&l.particulars),
                safe_cell(&l.people),
                safe_cell(&l.paid_by),
                safe_cell(&l.bill_no),
                money::plain(l.amount),
                l.bills
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(" "),
            ]
        };
        for g in &report.groups {
            rows.extend(g.lines.iter().map(|l| line_row(&g.name, l)));
        }
        if report.company_total > 0 {
            rows.push(vec![]);
            rows.push(vec!["Paid directly by the company (not claimed)".into()]);
            for g in &report.groups {
                rows.extend(g.company_lines.iter().map(|l| line_row(&g.name, l)));
            }
            rows.push(total("Total paid by company", report.company_total));
        }
    } else if report.company_total > 0 {
        rows.push(vec![]);
        rows.push(vec!["Paid directly by the company (not claimed)".into()]);
        rows.extend(
            report
                .groups
                .iter()
                .filter(|g| g.company_total > 0)
                .map(|g| total(&g.name, g.company_total)),
        );
        rows.push(total("Total paid by company", report.company_total));
    }
    if report.is_claim {
        rows.push(vec![]);
        rows.push(total("Total claimed", report.total));
        rows.extend(
            report
                .money
                .iter()
                .map(|(date, label, amount)| total(&format!("{label}, {date}"), *amount)),
        );
        rows.push(total(&report.net_label, report.net.abs()));
        if report.allowance > 0 {
            rows.push(total(
                "Bonus or allowance received (not adjusted)",
                report.allowance,
            ));
        }
    }
    if report.proof && report.proof_summary.all.count > 0 {
        let p = &report.proof_summary;
        rows.push(vec![]);
        rows.push(
            ["I paid with", "Payments", "With a picture", "Amount (INR)"]
                .map(String::from)
                .to_vec(),
        );
        rows.extend(p.modes.iter().map(|m| {
            vec![
                m.name.to_owned(),
                m.all.count.to_string(),
                m.pictured.count.to_string(),
                money::plain(m.all.total),
            ]
        }));
        if !report.unproved.is_empty() {
            rows.push(vec![]);
            rows.push(vec![
                "No bill or payment screenshot attached; paid by me".into(),
            ]);
            rows.extend(report.unproved.iter().map(|l| {
                let particulars = if report.detail {
                    safe_cell(&l.particulars)
                } else {
                    String::new()
                };
                vec![
                    l.date.clone(),
                    l.category.clone(),
                    l.mode.clone(),
                    particulars,
                    money::plain(l.amount),
                ]
            }));
            rows.push(total("Total without a picture", report.unproved_total));
        }
    }
    if !report.stages.is_empty() {
        rows.push(vec![]);
        rows.push(vec!["Stage of the trip".into(), "Amount (INR)".into()]);
        rows.extend(
            report
                .stages
                .iter()
                .map(|s| vec![s.name.clone(), money::plain(s.total)]),
        );
    }
    if !report.by_person.is_empty() {
        rows.push(vec![]);
        rows.push(vec!["Spent for".into(), "Amount (INR)".into()]);
        rows.extend(
            report
                .by_person
                .iter()
                .map(|(name, value)| vec![safe_cell(name), money::plain(*value)]),
        );
    }
    csv_text(rows)
}

/// Reads columns named date, amount, and optionally category, description,
/// from, to. Dates are YYYY-MM-DD or DD/MM/YYYY. Either every row is stored or,
/// if any row is wrong, none.
pub fn import_csv(store: &mut Store, content: &str) -> Result<usize> {
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(content.trim_start_matches('\u{feff}').as_bytes());
    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| Error::msg(format!("Could not read the file: {e}")))?
        .iter()
        .map(str::to_lowercase)
        .collect();
    let column = |name: &str| headers.iter().position(|h| h.starts_with(name));
    let (Some(date_col), Some(amount_col)) = (column("date"), column("amount")) else {
        return Err(Error::msg(
            "The first row needs the column names date and amount.",
        ));
    };

    let mut changes = Vec::new();
    for (index, record) in reader.records().enumerate() {
        let line = index + 2;
        let record = record.map_err(|e| Error::msg(format!("Line {line}: {e}")))?;
        if record.iter().all(str::is_empty) {
            continue;
        }
        let get = |name: &str| {
            column(name)
                .and_then(|i| record.get(i))
                .unwrap_or("")
                .to_owned()
        };
        let raw_date = record.get(date_col).unwrap_or("");
        let date = ["%Y-%m-%d", "%d/%m/%Y", "%d-%m-%Y"]
            .iter()
            .find_map(|f| chrono::NaiveDate::parse_from_str(raw_date, f).ok())
            .ok_or_else(|| {
                Error::msg(format!("Line {line}: the date '{raw_date}' is not valid."))
            })?;
        let raw_amount = record.get(amount_col).unwrap_or("");
        let amount = money::parse(raw_amount).ok_or_else(|| {
            Error::msg(format!(
                "Line {line}: the amount '{raw_amount}' is not valid."
            ))
        })?;
        let expense = Expense {
            date: date.format("%Y-%m-%d").to_string(),
            category: category_id(&get("category")).into(),
            description: get("description"),
            from: get("from"),
            to: get("to"),
            amount,
            paid_by: ME.into(),
            ..Default::default()
        };
        let id = uuid::Uuid::now_v7().to_string();
        let body = model::validate(EXPENSE, &id, &serde_json::to_value(expense)?)
            .map_err(|e| Error::msg(format!("Line {line}: {e}")))?;
        changes.push(Change {
            kind: EXPENSE.into(),
            id,
            deleted: false,
            body,
        });
    }
    let count = changes.len();
    if count == 0 {
        return Err(Error::msg("The file has no rows to import."));
    }
    store.commit(changes)?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formulas_are_neutralised() {
        assert_eq!(safe_cell("=1+1"), "'=1+1");
    }

    #[test]
    fn company_paid_lines_are_never_labelled_as_claims_or_personal() {
        let mut e = Expense {
            paid_by: COMPANY_PAYER.into(),
            ..Default::default()
        };
        assert_eq!(claim_label(&e), "Not claimed (company paid)");
        e.claim = Claim::Reimbursable;
        assert_eq!(claim_label(&e), "Not claimed (company paid)");
        e.paid_by = "a-colleague".into();
        assert_eq!(claim_label(&e), "Not claimed (paid by another)");
    }

    #[test]
    fn imports_all_rows_or_none() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Store::open(dir.path(), "a").unwrap();
        let bad = "date,amount,category\n2026-11-14,100,Food\nsoon,5,Food\n";
        assert!(import_csv(&mut s, bad).is_err());
        assert_eq!(s.all().count(), 0);
        let good = "Date,Amount,Category,Description\n14/11/2026,\"1,250.50\",Food,Lunch\n";
        assert_eq!(import_csv(&mut s, good).unwrap(), 1);
        let saved = s.live(EXPENSE).next().unwrap();
        assert_eq!(saved.body["amount"], 125_050);
        assert_eq!(saved.body["category"], "food");
    }
}
