//! The whole surface between a UI and the data: one request in, JSON out.

use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::error::{Error, Result};
use crate::model::*;
use crate::settlement::COMPANY_PAYER;
use crate::store::{Change, Record, Store};
use crate::{blobs, entry, export, loans, message, pdf, report, share, views};

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    Snapshot {
        today: String,
    },
    Save {
        kind: String,
        #[serde(default)]
        id: Option<String>,
        body: Value,
    },
    Delete {
        kind: String,
        id: String,
    },
    LoanPreview {
        principal: i64,
        rate_bps: u32,
        tenure_months: u32,
    },
    ParseMessage {
        text: String,
        today: String,
    },
    /// Reads typed lines into drafts; nothing is saved.
    ParseEntry {
        text: String,
        tour_id: String,
        today: String,
    },
    /// Saves several expenses and company payments together, all or none.
    SaveMany {
        items: Vec<Item>,
    },
    ClaimReport {
        tour_id: String,
        today: String,
        #[serde(default)]
        detail: bool,
    },
    ExportClaim {
        tour_id: String,
        format: ClaimFormat,
        today: String,
        #[serde(default)]
        detail: bool,
    },
    /// A JPEG, base64, for one expense.
    AddAttachment {
        expense_id: String,
        data: String,
    },
    GetAttachment {
        id: String,
    },
    /// The report laid out page by page, as the PDF will be.
    ReportPreview {
        tour_id: String,
        today: String,
        #[serde(default)]
        detail: bool,
    },
    /// Which category a typed note belongs to.
    SuggestCategory {
        text: String,
    },
    LoanMark {
        loan_id: String,
        number: u32,
        paid: bool,
        today: String,
    },
    /// Marks every instalment already due as paid.
    LoanMarkDue {
        loan_id: String,
        today: String,
    },
    LoanClose {
        loan_id: String,
        date: String,
        /// Paid to settle it; 0 if nothing more was due.
        amount: i64,
    },
    LoanReopen {
        loan_id: String,
    },
    ExportExpenses {
        today: String,
    },
    ExportBundle {
        include_local: bool,
        today: String,
    },
    ImportBundle {
        content: String,
    },
    ImportCsv {
        content: String,
    },
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClaimFormat {
    Csv,
    Pdf,
}

#[derive(Debug, Deserialize)]
pub struct Item {
    pub kind: String,
    pub body: Value,
}

fn today(text: &str) -> Result<NaiveDate> {
    parse_date("today", text)
}

fn file(name: String, mime: &str, content: String) -> Value {
    json!({ "name": name, "mime": mime, "content": content, "base64": false })
}

fn binary_file(name: String, mime: &str, content: &[u8]) -> Value {
    let content = data_encoding::BASE64.encode(content);
    json!({ "name": name, "mime": mime, "content": content, "base64": true })
}

fn slug(text: &str) -> String {
    let joined: String = text
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let slug = joined
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "tour".into()
    } else {
        slug.chars().take(40).collect()
    }
}

/// A new entry must not point at something that is not there.
fn check_references(store: &Store, kind: &str, body: &Value) -> Result<()> {
    let text = |name: &str| body.get(name).and_then(Value::as_str).unwrap_or("");
    let exists = |field: &'static str, target: &str, id: &str| {
        if id.is_empty() || store.get(target, id).is_some() {
            Ok(())
        } else {
            Err(Error::invalid(
                field,
                "That item was deleted. Pick another.",
            ))
        }
    };
    match kind {
        EXPENSE => {
            exists("tour_id", TOUR, text("tour_id"))?;
            let payer = text("paid_by");
            if payer != ME && payer != COMPANY_PAYER {
                exists("paid_by", PERSON, payer)?;
            }
            Ok(())
        }
        TRANSFER => {
            exists("tour_id", TOUR, text("tour_id"))?;
            exists("person_id", PERSON, text("person_id"))
        }
        PAYMENT => exists("loan_id", LOAN, text("loan_id")),
        ATTACHMENT => {
            exists("expense_id", EXPENSE, text("expense_id"))?;
            if store.blobs().has(text("hash")) {
                Ok(())
            } else {
                Err(Error::invalid(
                    "bill",
                    "That picture is not on this device.",
                ))
            }
        }
        TOUR | PERSON => exists("company_id", COMPANY, text("company_id")),
        _ => Ok(()),
    }
}

fn prepare(store: &Store, kind: &str, id: Option<String>, body: Value) -> Result<Change> {
    let id = match kind {
        PROFILE => ME.to_owned(),
        BUDGET => id.ok_or_else(|| Error::invalid("category", "Pick a category."))?,
        // One id per instalment, so two devices marking it paid make one entry.
        PAYMENT if body.get("kind").and_then(Value::as_str) == Some("instalment") => {
            let loan = body.get("loan_id").and_then(Value::as_str).unwrap_or("");
            let number = body.get("number").and_then(Value::as_u64).unwrap_or(0);
            if number == 0 {
                return Err(Error::invalid("number", "Pick the instalment."));
            }
            format!("{loan}:{number}")
        }
        _ => id
            .filter(|i| !i.is_empty())
            .unwrap_or_else(|| uuid::Uuid::now_v7().to_string()),
    };
    let mut body = validate(kind, &id, &body)?;
    check_references(store, kind, &body)?;
    if kind == EXPENSE && body["tour_id"] == "" {
        body["claim"] = json!("personal");
        body["rejected"] = json!(false);
    }
    Ok(Change {
        kind: kind.into(),
        id,
        deleted: false,
        body,
    })
}

fn save(store: &mut Store, kind: &str, id: Option<String>, body: Value) -> Result<Value> {
    let change = prepare(store, kind, id, body)?;
    let id = change.id.clone();
    store.commit(vec![change])?;
    Ok(json!({ "id": id }))
}

fn save_many(store: &mut Store, items: Vec<Item>) -> Result<Value> {
    if items.is_empty() || items.len() > 100 {
        return Err(Error::msg("Save between 1 and 100 entries at a time."));
    }
    let mut changes = Vec::new();
    for (index, item) in items.into_iter().enumerate() {
        if item.kind != EXPENSE && item.kind != TRANSFER {
            return Err(Error::msg("Only expenses and company payments."));
        }
        let change = prepare(store, &item.kind, None, item.body)
            .map_err(|e| Error::msg(format!("Entry {}: {e}", index + 1)))?;
        changes.push(change);
    }
    let saved = changes.len();
    store.commit(changes)?;
    Ok(json!({ "saved": saved }))
}

fn parse_entry(store: &Store, text: &str, tour_id: &str, now: NaiveDate) -> Result<Value> {
    let tour: Option<Tour> = store
        .get(TOUR, tour_id)
        .and_then(|r| serde_json::from_value(r.body.clone()).ok());
    let dates = tour.as_ref().and_then(|t| {
        Some((
            parse_date("start_date", &t.start_date).ok()?,
            parse_date("end_date", &t.end_date).ok()?,
        ))
    });
    let people: Vec<(String, String)> = views::load::<Person>(store, PERSON)
        .into_iter()
        .map(|p| (p.id, p.data.name))
        .collect();
    // Refused rather than cut short, so no typed line can vanish.
    if entry::lines(text).count() > entry::MAX_LINES {
        return Err(Error::invalid("text", "Type at most 100 lines at a time."));
    }
    let drafts = entry::parse(
        text,
        &entry::Context {
            today: now,
            tour_dates: dates,
            // Only on a work trip is spending claimed unless said otherwise.
            company_tour: tour.is_some_and(|t| t.is_work()),
            people: &people,
        },
    );
    if drafts.is_empty() {
        return Err(Error::invalid("text", "Type at least one line."));
    }
    Ok(json!({ "drafts": drafts }))
}

fn loan_of(store: &Store, id: &str) -> Result<Loan> {
    let record = store.get(LOAN, id).ok_or(Error::NotFound)?;
    Ok(serde_json::from_value(record.body.clone())?)
}

fn payments_of(store: &Store, loan: &str) -> Vec<Payment> {
    views::load::<Payment>(store, PAYMENT)
        .into_iter()
        .map(|p| p.data)
        .filter(|p| p.loan_id == loan)
        .collect()
}

fn loan_change(id: &str, loan: &Loan) -> Result<Change> {
    Ok(Change {
        kind: LOAN.into(),
        id: id.into(),
        deleted: false,
        body: serde_json::to_value(loan)?,
    })
}

/// A payment for one instalment, recorded on its due date, or today if early.
fn instalment_change(loan_id: &str, row: &loans::Instalment, now: NaiveDate) -> Result<Change> {
    let payment = Payment {
        loan_id: loan_id.into(),
        date: row.due.min(now).format("%Y-%m-%d").to_string(),
        kind: PaymentKind::Instalment,
        number: row.number,
        amount: row.amount,
        note: String::new(),
    };
    Ok(Change {
        kind: PAYMENT.into(),
        id: format!("{loan_id}:{}", row.number),
        deleted: false,
        body: serde_json::to_value(payment)?,
    })
}

fn loan_mark(
    store: &mut Store,
    id: &str,
    number: u32,
    paid: bool,
    now: NaiveDate,
) -> Result<Value> {
    let mut loan = loan_of(store, id)?;
    let status = loans::status(&loan, &payments_of(store, id), now);
    let row = status
        .schedule
        .iter()
        .find(|r| r.number == number)
        .ok_or_else(|| Error::msg("That instalment is not in the schedule."))?;
    let mut changes = Vec::new();
    if paid {
        if !row.paid {
            changes.push(instalment_change(id, row, now)?);
        }
    } else {
        if number <= loan.paid_before {
            // Turn the later "paid before tracking" ones into real payments.
            for later in status
                .schedule
                .iter()
                .filter(|r| r.number > number && r.number <= loan.paid_before)
            {
                changes.push(instalment_change(id, later, now)?);
            }
            loan.paid_before = number - 1;
            changes.push(loan_change(id, &loan)?);
        }
        let ids: Vec<String> = store
            .live(PAYMENT)
            .filter(|r| {
                r.str_field("loan_id") == Some(id)
                    && r.str_field("kind") == Some("instalment")
                    && r.body["number"] == number
            })
            .map(|r| r.id.clone())
            .collect();
        for payment in ids {
            if let Some(record) = store.get(PAYMENT, &payment) {
                changes.push(tombstone(record));
            }
        }
    }
    if !changes.is_empty() {
        store.commit(changes)?;
    }
    Ok(json!({}))
}

fn loan_mark_due(store: &mut Store, id: &str, now: NaiveDate) -> Result<Value> {
    let loan = loan_of(store, id)?;
    if loan.closed {
        return Err(Error::msg("This loan is closed."));
    }
    let changes: Result<Vec<Change>> = loans::status(&loan, &payments_of(store, id), now)
        .schedule
        .iter()
        .filter(|r| !r.paid && r.due <= now)
        .map(|r| instalment_change(id, r, now))
        .collect();
    let changes = changes?;
    let marked = changes.len();
    if marked > 0 {
        store.commit(changes)?;
    }
    Ok(json!({ "marked": marked }))
}

fn loan_close(store: &mut Store, id: &str, date: &str, amount: i64) -> Result<Value> {
    let mut loan = loan_of(store, id)?;
    parse_date("date", date)?;
    if !(0..=crate::money::MAX_AMOUNT).contains(&amount) {
        return Err(Error::invalid("amount", "Enter an amount of zero or more."));
    }
    loan.closed = true;
    loan.closed_on = date.into();
    let mut changes = vec![loan_change(id, &loan)?];
    if amount > 0 {
        let payment = Payment {
            loan_id: id.into(),
            date: date.into(),
            kind: PaymentKind::Prepayment,
            number: 0,
            amount,
            note: "Closing payment".into(),
        };
        changes.push(Change {
            kind: PAYMENT.into(),
            id: format!("{id}:close"),
            deleted: false,
            body: serde_json::to_value(payment)?,
        });
    }
    store.commit(changes)?;
    Ok(json!({}))
}

fn loan_reopen(store: &mut Store, id: &str) -> Result<Value> {
    let mut loan = loan_of(store, id)?;
    loan.closed = false;
    loan.closed_on.clear();
    let mut changes = vec![loan_change(id, &loan)?];
    if let Some(record) = store.get(PAYMENT, &format!("{id}:close")) {
        changes.push(tombstone(record));
    }
    store.commit(changes)?;
    Ok(json!({}))
}

/// Keeps only what sync needs to decide whether the deletion may be shared.
fn tombstone(record: &Record) -> Change {
    let mut keep = serde_json::Map::new();
    // `hash` lets a deleted picture's file be cleared; the rest decide sharing.
    for field in ["local_only", "tour_id", "loan_id", "expense_id", "hash"] {
        if let Some(value) = record.body.get(field) {
            keep.insert(field.into(), value.clone());
        }
    }
    Change {
        kind: record.kind.clone(),
        id: record.id.clone(),
        deleted: true,
        body: Value::Object(keep),
    }
}

fn uses(store: &Store, kind: &str, id: &str) -> usize {
    store
        .all()
        .filter(|r| !r.deleted)
        .filter(|r| {
            let is = |field: &str| r.str_field(field) == Some(id);
            let listed = |field: &str| {
                r.body
                    .get(field)
                    .and_then(Value::as_array)
                    .is_some_and(|list| list.iter().any(|v| v.as_str() == Some(id)))
            };
            match kind {
                PERSON => {
                    is("paid_by")
                        || is("person_id")
                        || listed("split_with")
                        || listed("participants")
                }
                COMPANY => r.kind == TOUR && is("company_id"),
                _ => false,
            }
        })
        .count()
}

fn delete(store: &mut Store, kind: &str, id: &str) -> Result<Value> {
    let record = store.get(kind, id).ok_or(Error::NotFound)?;
    let used = uses(store, kind, id);
    if used > 0 {
        return Err(Error::msg(format!(
            "Used in {used} {}. Change or delete those first.",
            if used == 1 { "entry" } else { "entries" }
        )));
    }
    let mut changes = vec![tombstone(record)];
    let owned: &[(&str, &str)] = match kind {
        TOUR => &[(EXPENSE, "tour_id"), (TRANSFER, "tour_id")],
        LOAN => &[(PAYMENT, "loan_id")],
        EXPENSE => &[(ATTACHMENT, "expense_id")],
        _ => &[],
    };
    for (child, field) in owned {
        changes.extend(
            store
                .live(child)
                .filter(|r| r.str_field(field) == Some(id))
                .map(tombstone),
        );
    }
    // The bills of a deleted tour's expenses go with them.
    if kind == TOUR {
        let expenses: Vec<&str> = changes
            .iter()
            .filter(|c| c.kind == EXPENSE)
            .map(|c| c.id.as_str())
            .collect();
        let bills: Vec<Change> = store
            .live(ATTACHMENT)
            .filter(|r| {
                r.str_field("expense_id")
                    .is_some_and(|e| expenses.contains(&e))
            })
            .map(tombstone)
            .collect();
        changes.extend(bills);
    }
    let pictures: Vec<String> = changes
        .iter()
        .filter(|c| c.kind == ATTACHMENT)
        .filter_map(|c| {
            c.body
                .get("hash")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .collect();
    let removed = changes.len();
    store.commit(changes)?;
    // Clear the files nothing else still shows.
    let in_use = share::picture_hashes(store.all());
    let blobs = store.blobs();
    pictures
        .iter()
        .filter(|hash| !in_use.contains(*hash))
        .for_each(|hash| blobs.remove(hash));
    Ok(json!({ "removed": removed }))
}

fn add_attachment(store: &mut Store, expense_id: &str, data: &str) -> Result<Value> {
    if store.get(EXPENSE, expense_id).is_none() {
        return Err(Error::NotFound);
    }
    let held = store
        .live(ATTACHMENT)
        .filter(|r| r.str_field("expense_id") == Some(expense_id))
        .count();
    if held >= MAX_BILLS_PER_EXPENSE {
        return Err(Error::invalid(
            "bill",
            "Ten pictures for one expense is the most.",
        ));
    }
    let bytes = data_encoding::BASE64
        .decode(data.as_bytes())
        .map_err(|_| Error::invalid("bill", "That picture could not be read."))?;
    let (width, height) = blobs::jpeg_size(&bytes)?;
    let attachment = Attachment {
        expense_id: expense_id.into(),
        hash: store.blobs().put(&bytes)?,
        width,
        height,
        bytes: bytes.len() as u64,
        full_page: false,
    };
    save(store, ATTACHMENT, None, serde_json::to_value(attachment)?)
}

fn picture(store: &Store, id: &str) -> Result<Vec<u8>> {
    let record = store.get(ATTACHMENT, id).ok_or(Error::NotFound)?;
    store
        .blobs()
        .get(record.str_field("hash").unwrap_or(""))
        .ok_or_else(|| Error::msg("This picture has not reached this device yet. Sync again."))
}

/// The pictures a report needs, for those that are on this device.
fn report_pictures(store: &Store, report: &report::Report) -> pdf::Pictures {
    let blobs = store.blobs();
    report
        .bills
        .iter()
        .filter_map(|b| Some((b.hash.clone(), blobs.get(&b.hash)?)))
        .collect()
}

/// The same note filed before is the person's own answer; otherwise the words.
fn suggest_category(store: &Store, text: &str) -> Value {
    let same = |other: &str| other.trim().eq_ignore_ascii_case(text.trim());
    let filed = (!text.trim().is_empty())
        .then(|| {
            views::load::<Expense>(store, EXPENSE)
                .into_iter()
                .filter(|e| same(&e.data.description))
                .max_by(|a, b| (&a.data.date, &a.id).cmp(&(&b.data.date, &b.id)))
        })
        .flatten();
    match filed {
        Some(e) => json!({ "category": e.data.category, "from": "history" }),
        None => match entry::categorise(text) {
            Some(category) => json!({ "category": category, "from": "words" }),
            None => json!({ "category": null, "from": "" }),
        },
    }
}

fn claim_report(
    store: &Store,
    tour_id: &str,
    now: NaiveDate,
    detail: bool,
) -> Result<report::Report> {
    let data = views::data(store, now);
    let tour = data
        .tours
        .iter()
        .find(|x| x.id == tour_id)
        .ok_or(Error::NotFound)?;
    Ok(report::build(&data, tour, detail))
}

pub fn handle(store: &mut Store, request: Request) -> Result<Value> {
    match request {
        Request::Snapshot { today: t } => {
            Ok(serde_json::to_value(views::snapshot(store, today(&t)?))?)
        }
        Request::Save { kind, id, body } => save(store, &kind, id, body),
        Request::Delete { kind, id } => delete(store, &kind, &id),
        Request::LoanPreview {
            principal,
            rate_bps,
            tenure_months,
        } => {
            let valid = principal > 0
                && principal <= crate::money::MAX_AMOUNT
                && rate_bps <= 10_000
                && (1..=600).contains(&tenure_months);
            let emi = if valid {
                loans::emi_for(principal, rate_bps, tenure_months)
            } else {
                0
            };
            Ok(json!({ "emi": emi }))
        }
        Request::ParseMessage { text, today: t } => {
            let draft = message::parse(&text, today(&t)?)
                .ok_or_else(|| Error::invalid("text", "No rupee amount found in that message."))?;
            Ok(serde_json::to_value(draft)?)
        }
        Request::ParseEntry {
            text,
            tour_id,
            today: t,
        } => parse_entry(store, &text, &tour_id, today(&t)?),
        Request::SaveMany { items } => save_many(store, items),
        Request::ClaimReport {
            tour_id,
            today: t,
            detail,
        } => Ok(serde_json::to_value(claim_report(
            store,
            &tour_id,
            today(&t)?,
            detail,
        )?)?),
        Request::ExportClaim {
            tour_id,
            format,
            today: t,
            detail,
        } => {
            let report = claim_report(store, &tour_id, today(&t)?, detail)?;
            let kind = if report.is_claim { "claim" } else { "expenses" };
            let name = format!("{kind}-{}", slug(&report.tour));
            let pictures = report_pictures(store, &report);
            Ok(match format {
                ClaimFormat::Csv => file(
                    format!("{name}.csv"),
                    "text/csv",
                    export::report_csv(&report)?,
                ),
                ClaimFormat::Pdf => binary_file(
                    format!("{name}.pdf"),
                    "application/pdf",
                    &pdf::render(&report, &pictures)?,
                ),
            })
        }
        Request::AddAttachment { expense_id, data } => add_attachment(store, &expense_id, &data),
        Request::GetAttachment { id } => {
            let bytes = picture(store, &id)?;
            Ok(json!({ "data": data_encoding::BASE64.encode(&bytes) }))
        }
        Request::ReportPreview {
            tour_id,
            today: t,
            detail,
        } => {
            let report = claim_report(store, &tour_id, today(&t)?, detail)?;
            let pictures = report_pictures(store, &report);
            let pages = pdf::preview(&report, &pictures)?;
            let images: serde_json::Map<String, Value> = pictures
                .iter()
                .map(|(hash, bytes)| (hash.clone(), json!(data_encoding::BASE64.encode(bytes))))
                .collect();
            Ok(json!({
                "width": pdf::WIDTH,
                "height": pdf::HEIGHT,
                "pages": pages,
                "images": images,
            }))
        }
        Request::SuggestCategory { text } => Ok(suggest_category(store, &text)),
        Request::LoanMark {
            loan_id,
            number,
            paid,
            today: t,
        } => loan_mark(store, &loan_id, number, paid, today(&t)?),
        Request::LoanMarkDue { loan_id, today: t } => loan_mark_due(store, &loan_id, today(&t)?),
        Request::LoanClose {
            loan_id,
            date,
            amount,
        } => loan_close(store, &loan_id, &date, amount),
        Request::LoanReopen { loan_id } => loan_reopen(store, &loan_id),
        Request::ExportExpenses { today: t } => {
            let data = views::data(store, today(&t)?);
            Ok(file(
                format!("expenses-{t}.csv"),
                "text/csv",
                export::all_expenses_csv(&data)?,
            ))
        }
        Request::ExportBundle {
            include_local,
            today: t,
        } => {
            today(&t)?;
            let what = if include_local { "backup" } else { "sync" };
            Ok(file(
                format!("tourledger-{what}-{t}.json"),
                "application/json",
                share::export_bundle(store, include_local)?,
            ))
        }
        Request::ImportBundle { content } => {
            Ok(json!({ "imported": share::import_bundle(store, &content)? }))
        }
        Request::ImportCsv { content } => {
            Ok(json!({ "imported": export::import_csv(store, &content)? }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(store: &mut Store, request: Value) -> Result<Value> {
        handle(store, serde_json::from_value(request).unwrap())
    }

    fn save_one(store: &mut Store, kind: &str, body: Value) -> String {
        call(store, json!({"op": "save", "kind": kind, "body": body})).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    #[test]
    fn a_category_from_a_newer_version_still_arrives_and_is_claimed() {
        let (a_dir, b_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let a = &mut Store::open(a_dir.path(), "a").unwrap();
        let tour = save_one(
            a,
            TOUR,
            json!({"name": "Fair", "start_date": "2026-11-14", "end_date": "2026-11-16"}),
        );
        save_one(
            a,
            EXPENSE,
            json!({"tour_id": tour, "date": "2026-11-14", "category": "metro",
                   "amount": 4_000, "paid_by": "me", "claim": "reimbursable"}),
        );
        // As if sent by a version that knows a category this one does not.
        let bundle = call(
            a,
            json!({"op": "export_bundle", "include_local": true, "today": "2026-11-20"}),
        )
        .unwrap()["content"]
            .as_str()
            .unwrap()
            .replace("\"metro\"", "\"cable_car\"");
        let b = &mut Store::open(b_dir.path(), "b").unwrap();
        call(b, json!({"op": "import_bundle", "content": bundle})).unwrap();
        let report = call(
            b,
            json!({"op": "claim_report", "tour_id": tour, "today": "2026-11-20"}),
        )
        .unwrap();
        assert_eq!(report["total"], 4_000);
        assert_eq!(report["groups"][0]["name"], "Miscellaneous");
        // Still not a way to store anything at all as a category.
        let bad = json!({"op": "save", "kind": EXPENSE, "body": {"date": "2026-11-14",
            "category": "Not A Category!", "amount": 1, "paid_by": "me"}});
        assert!(call(b, bad).is_err());
    }

    #[test]
    fn a_trade_fair_from_entry_to_claim() {
        let dir = tempfile::tempdir().unwrap();
        let s = &mut Store::open(dir.path(), "a").unwrap();
        save_one(
            s,
            PROFILE,
            json!({"name": "Asha <Rao>", "monthly_income": 5_000_000}),
        );
        let tour = save_one(
            s,
            TOUR,
            json!({"name": "IITF 2026", "organiser": "ITPO", "venue": "Bharat Mandapam",
                   "start_date": "2026-11-14", "end_date": "2026-11-16", "status": "completed"}),
        );
        for (paid_by, amount) in [("me", 25_000), ("company", 184_000), ("me", 120_000)] {
            save_one(
                s,
                EXPENSE,
                json!({"tour_id": tour, "date": "2026-11-14", "category": "local",
                       "amount": amount, "paid_by": paid_by, "claim": "reimbursable"}),
            );
        }
        save_one(
            s,
            TRANSFER,
            json!({"tour_id": tour, "date": "2026-11-10", "kind": "advance", "amount": 100_000}),
        );

        let snap = call(s, json!({"op": "snapshot", "today": "2026-12-05"})).unwrap();
        assert_eq!(snap["tours"][0]["summary"]["net"], 45_000);
        assert_eq!(snap["insights"]["awaiting_reimbursement"], 45_000);
        assert_eq!(snap["insights"]["alerts"][0]["kind"], "claim_not_sent");

        let report = call(
            s,
            json!({"op": "claim_report", "tour_id": tour, "today": "2026-12-05"}),
        )
        .unwrap();
        assert_eq!(report["total"], 145_000);
        assert_eq!(report["company_total"], 184_000);
        assert_eq!(report["net"], 45_000);
        assert_eq!(report["groups"][0]["name"], "Travelling expenses");

        let pdf = call(
            s,
            json!({"op": "export_claim", "tour_id": tour, "format": "pdf", "today": "2026-12-05"}),
        )
        .unwrap();
        let bytes = data_encoding::BASE64
            .decode(pdf["content"].as_str().unwrap().as_bytes())
            .unwrap();
        assert!(bytes.starts_with(b"%PDF-") && bytes.len() > 2_000);
        if let Some(path) = std::env::var_os("TOURLEDGER_SAMPLE_PDF") {
            std::fs::write(path, &bytes).unwrap();
        }

        let removed = call(s, json!({"op": "delete", "kind": "tour", "id": tour})).unwrap();
        assert_eq!(removed["removed"], 5);
    }

    #[test]
    fn typed_lines_are_saved_together_or_not_at_all() {
        let dir = tempfile::tempdir().unwrap();
        let s = &mut Store::open(dir.path(), "a").unwrap();
        let tour = save_one(
            s,
            TOUR,
            json!({"name": "Fair", "start_date": "2026-11-14", "end_date": "2026-11-23"}),
        );
        let read = call(
            s,
            json!({"op": "parse_entry", "tour_id": tour, "today": "2026-11-27",
                   "text": "cab home to station 180\ncompany gave 2000"}),
        )
        .unwrap();
        assert_eq!(read["drafts"][0]["claim"], "reimbursable");
        assert_eq!(read["drafts"][1]["kind"], "advance");
        assert_eq!(s.live(EXPENSE).count(), 0);

        let good = json!({"kind": "expense", "body": {"tour_id": tour, "date": "2026-11-14",
                          "category": "local", "amount": 18_000, "paid_by": "me"}});
        let bad = json!({"kind": "expense", "body": {"date": "2026-11-14", "category": "local",
                         "amount": 0, "paid_by": "me"}});
        assert!(call(s, json!({"op": "save_many", "items": [good, bad]})).is_err());
        assert_eq!(s.live(EXPENSE).count(), 0);
        call(s, json!({"op": "save_many", "items": [good]})).unwrap();
        assert_eq!(s.live(EXPENSE).count(), 1);
    }

    #[test]
    fn instalments_can_be_ticked_unticked_closed_and_reopened() {
        let dir = tempfile::tempdir().unwrap();
        let s = &mut Store::open(dir.path(), "a").unwrap();
        let loan = save_one(
            s,
            LOAN,
            json!({"name": "Bike", "category": "vehicle", "principal": 1_000_000, "rate_bps": 1000,
                   "tenure_months": 10, "start_date": "2026-01-01",
                   "first_due_date": "2026-02-01", "paid_before": 3}),
        );
        let paid = |s: &mut Store| -> Vec<bool> {
            let snap = call(s, json!({"op": "snapshot", "today": "2026-07-15"})).unwrap();
            snap["loans"][0]["status"]["schedule"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["paid"].as_bool().unwrap())
                .collect()
        };
        let mark = |s: &mut Store, number: u32, on: bool| {
            call(
                s,
                json!({"op": "loan_mark", "loan_id": loan, "number": number, "paid": on,
                       "today": "2026-07-15"}),
            )
            .unwrap();
        };
        // Unticking one from before tracking leaves its neighbours ticked.
        mark(s, 2, false);
        assert_eq!(&paid(s)[..4], [true, false, true, false]);
        mark(s, 2, true);
        mark(s, 5, true);
        assert_eq!(&paid(s)[..6], [true, true, true, false, true, false]);

        let marked = call(
            s,
            json!({"op": "loan_mark_due", "loan_id": loan, "today": "2026-07-15"}),
        )
        .unwrap();
        assert_eq!(marked["marked"], 2); // numbers 4 and 6

        call(
            s,
            json!({"op": "loan_close", "loan_id": loan, "date": "2026-07-15", "amount": 410_000}),
        )
        .unwrap();
        let snap = call(s, json!({"op": "snapshot", "today": "2026-07-15"})).unwrap();
        assert_eq!(snap["loans"][0]["status"]["outstanding"], 0);
        assert_eq!(snap["portfolio"]["closed"], 1);
        assert_eq!(snap["portfolio"]["outstanding"], 0);
        // Loan money never reaches the tour figures.
        assert!(snap["insights"]["alerts"].as_array().unwrap().is_empty());

        call(s, json!({"op": "loan_reopen", "loan_id": loan})).unwrap();
        let snap = call(s, json!({"op": "snapshot", "today": "2026-07-15"})).unwrap();
        assert_eq!(snap["portfolio"]["running"], 1);
        assert!(snap["portfolio"]["outstanding"].as_i64().unwrap() > 0);
    }

    /// A small real JPEG (8x8 grey).
    fn jpeg() -> Vec<u8> {
        data_encoding::BASE64.decode(
            b"/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAP//////////////////////////////////////////////////////////////////////////////////////wAALCAAIAAgBAREA/8QAFAABAAAAAAAAAAAAAAAAAAAAB//EABQQAQAAAAAAAAAAAAAAAAAAAAD/2gAIAQEAAD8AN//Z",
        )
        .unwrap()
    }

    #[test]
    fn bills_are_stored_shown_in_the_report_backed_up_and_cleared() {
        let dir = tempfile::tempdir().unwrap();
        let s = &mut Store::open(dir.path(), "a").unwrap();
        let tour = save_one(
            s,
            TOUR,
            json!({"name": "Fair", "start_date": "2026-11-14", "end_date": "2026-11-16"}),
        );
        let claimed = save_one(
            s,
            EXPENSE,
            json!({"tour_id": tour, "date": "2026-11-14", "category": "food", "amount": 64_000,
                   "paid_by": "me", "claim": "reimbursable", "description": "Lunch"}),
        );
        let private = save_one(
            s,
            EXPENSE,
            json!({"tour_id": tour, "date": "2026-11-14", "category": "shopping", "amount": 900,
                   "paid_by": "me", "claim": "personal"}),
        );
        let picture = data_encoding::BASE64.encode(&jpeg());
        for expense in [&claimed, &private] {
            call(
                s,
                json!({"op": "add_attachment", "expense_id": expense, "data": picture}),
            )
            .unwrap();
        }
        assert!(
            call(
                s,
                json!({"op": "add_attachment", "expense_id": claimed, "data": "bm90IGEganBlZw=="})
            )
            .is_err()
        );

        // Only the claimed entry's bill goes into the claim.
        let report = call(
            s,
            json!({"op": "claim_report", "tour_id": tour, "today": "2026-11-27"}),
        )
        .unwrap();
        assert_eq!(report["bills"].as_array().unwrap().len(), 1);
        assert_eq!(report["bills"][0]["number"], 1);
        assert!(
            report["bills"][0]["label"]
                .as_str()
                .unwrap()
                .contains("640.00")
        );
        let preview = call(
            s,
            json!({"op": "report_preview", "tour_id": tour, "today": "2026-11-27"}),
        )
        .unwrap();
        assert_eq!(preview["pages"].as_array().unwrap().len(), 2);

        // A backup carries the picture to another device.
        let backup = call(
            s,
            json!({"op": "export_bundle", "include_local": true, "today": "2026-11-27"}),
        )
        .unwrap();
        let other = tempfile::tempdir().unwrap();
        let o = &mut Store::open(other.path(), "b").unwrap();
        call(
            o,
            json!({"op": "import_bundle", "content": backup["content"]}),
        )
        .unwrap();
        let hash = crate::blobs::hash_of(&jpeg());
        assert!(o.blobs().has(&hash));

        // Deleting one expense keeps the file the other still shows.
        call(s, json!({"op": "delete", "kind": "expense", "id": claimed})).unwrap();
        assert!(s.blobs().has(&hash));
        call(s, json!({"op": "delete", "kind": "tour", "id": tour})).unwrap();
        assert!(!s.blobs().has(&hash));
        assert_eq!(s.live(ATTACHMENT).count(), 0);
    }

    #[test]
    fn journey_days_are_separated_and_doubtful_categories_pointed_out() {
        let dir = tempfile::tempdir().unwrap();
        let s = &mut Store::open(dir.path(), "a").unwrap();
        let tour = save_one(
            s,
            TOUR,
            json!({"name": "Fair", "start_date": "2026-11-14", "end_date": "2026-11-24"}),
        );
        for (date, category, note, amount) in [
            ("2026-11-12", "local", "powder tea", 2_000),
            ("2026-11-15", "food", "lunch", 25_000),
            ("2026-11-25", "local", "auto home", 15_000),
        ] {
            save_one(
                s,
                EXPENSE,
                json!({"tour_id": tour, "date": date, "category": category, "amount": amount,
                       "description": note, "paid_by": "me", "claim": "reimbursable"}),
            );
        }
        let report = call(
            s,
            json!({"op": "claim_report", "tour_id": tour, "today": "2026-11-27"}),
        )
        .unwrap();
        let stages = report["stages"].as_array().unwrap();
        assert_eq!(stages.len(), 3);
        assert_eq!(stages[0]["total"], 2_000);
        assert!(stages[0]["name"].as_str().unwrap().starts_with("Journey"));
        assert_eq!(stages[1]["total"], 25_000);
        assert_eq!(report["check"].as_array().unwrap().len(), 1);
        assert_eq!(report["check"][0]["suggested"], "food");

        // What was filed before is remembered for the same note.
        save_one(
            s,
            EXPENSE,
            json!({"date": "2026-11-26", "category": "shopping", "amount": 100,
                   "description": "Paan", "paid_by": "me"}),
        );
        let hint = call(s, json!({"op": "suggest_category", "text": "paan"})).unwrap();
        assert_eq!(
            (hint["category"].as_str(), hint["from"].as_str()),
            (Some("shopping"), Some("history"))
        );
    }

    #[test]
    fn the_claim_gives_totals_unless_detail_is_asked_for() {
        let dir = tempfile::tempdir().unwrap();
        let s = &mut Store::open(dir.path(), "a").unwrap();
        save_one(s, PROFILE, json!({"name": "Asha Rao"}));
        let ravi = save_one(s, PERSON, json!({"name": "Ravi"}));
        let tour = save_one(
            s,
            TOUR,
            json!({"name": "Fair", "start_date": "2026-11-14", "end_date": "2026-11-24",
                   "participants": [ravi]}),
        );
        let add = |s: &mut Store, note: &str, paid_by: &str, split: Value| {
            save_one(
                s,
                EXPENSE,
                json!({"tour_id": tour, "date": "2026-11-15", "category": "food",
                       "amount": 2_000, "description": note, "paid_by": paid_by,
                       "claim": "reimbursable", "split_with": split}),
            )
        };
        add(s, "Poha", "me", json!(["me", ravi]));
        add(s, "Powder tea", "me", json!(["me"]));
        add(s, "Samosa", "company", json!([]));

        let export = |s: &mut Store, detail: bool| {
            let file = call(
                s,
                json!({"op": "export_claim", "tour_id": tour, "format": "csv",
                       "today": "2026-11-27", "detail": detail}),
            )
            .unwrap();
            file["content"].as_str().unwrap().to_owned()
        };
        let summary = export(s, false);
        assert!(summary.contains("Food and refreshments,40.00"), "{summary}");
        for item in ["Poha", "Powder tea", "Samosa"] {
            assert!(!summary.contains(item), "{item} in the summary:\n{summary}");
        }
        // My name from the profile, not "Self", where spending is by person.
        assert!(summary.contains("Asha Rao,30.00"), "{summary}");
        let full = export(s, true);
        assert!(full.contains("Poha,Both,Self"), "{full}");
        assert!(full.contains("Powder tea,Asha Rao,Self"), "{full}");
    }

    #[test]
    fn a_person_in_use_cannot_be_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let s = &mut Store::open(dir.path(), "a").unwrap();
        let ravi = save_one(s, PERSON, json!({"name": "Ravi"}));
        save_one(
            s,
            EXPENSE,
            json!({"date": "2026-11-14", "category": "food", "amount": 100,
                   "paid_by": "me", "split_with": ["me", ravi]}),
        );
        assert!(call(s, json!({"op": "delete", "kind": "person", "id": ravi})).is_err());
    }

    #[test]
    fn entries_of_a_tour_deleted_elsewhere_stay_visible() {
        let dir = tempfile::tempdir().unwrap();
        let s = &mut Store::open(dir.path(), "a").unwrap();
        let tour = save_one(
            s,
            TOUR,
            json!({"name": "Trip", "start_date": "2026-11-01", "end_date": "2026-11-02"}),
        );
        save_one(
            s,
            EXPENSE,
            json!({"tour_id": tour, "date": "2026-11-01", "category": "food",
                   "amount": 500, "paid_by": "me", "claim": "reimbursable"}),
        );
        // As a peer's deletion arrives: the tour alone, not its new expense.
        s.commit(vec![Change {
            kind: TOUR.into(),
            id: tour,
            deleted: true,
            body: json!({}),
        }])
        .unwrap();
        let snap = call(s, json!({"op": "snapshot", "today": "2026-11-27"})).unwrap();
        assert_eq!(snap["expenses"][0]["tour_id"], "");
        assert_eq!(snap["insights"]["months"][12]["spend"], 500);
        assert_eq!(snap["insights"]["alerts"][0]["kind"], "orphans");
    }

    #[test]
    fn marking_an_instalment_twice_keeps_one_payment() {
        let dir = tempfile::tempdir().unwrap();
        let s = &mut Store::open(dir.path(), "a").unwrap();
        let loan = save_one(
            s,
            LOAN,
            json!({"name": "Bike", "principal": 1_000_000, "rate_bps": 1000, "tenure_months": 10,
                   "start_date": "2026-01-01", "first_due_date": "2026-02-01"}),
        );
        let body = json!({"loan_id": loan, "date": "2026-02-01", "kind": "instalment",
                          "number": 1, "amount": 104_000});
        assert_eq!(
            save_one(s, PAYMENT, body.clone()),
            save_one(s, PAYMENT, body)
        );
        assert_eq!(s.live(PAYMENT).count(), 1);
    }
}
