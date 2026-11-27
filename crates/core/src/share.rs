//! What may leave this device, and what may be accepted onto it.

use std::collections::{BTreeMap, BTreeSet};

use data_encoding::BASE64;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::blobs::hash_of;
use crate::error::{Error, Result};
use crate::hlc::now_ms;
use crate::model::{self, ATTACHMENT, EXPENSE, LOAN, TOUR};
use crate::store::{Record, Store};

/// Format of records exchanged between devices and in sync files.
pub const FORMAT_VERSION: u32 = 2;

fn flagged(body: &Value) -> bool {
    body.get("local_only").and_then(Value::as_bool) == Some(true)
}

/// Marked "this device only", directly or through its tour or loan.
pub fn is_local_only(store: &Store, record: &Record) -> bool {
    if flagged(&record.body) {
        return true;
    }
    // A bill follows its expense, wherever that expense belongs.
    if record
        .str_field("expense_id")
        .and_then(|id| store.get_any(EXPENSE, id))
        .is_some_and(|expense| is_local_only(store, expense))
    {
        return true;
    }
    [("tour_id", TOUR), ("loan_id", LOAN)]
        .iter()
        .any(|(field, kind)| {
            record
                .str_field(field)
                .and_then(|id| store.get_any(kind, id))
                .is_some_and(|parent| flagged(&parent.body))
        })
}

pub fn shareable(store: &Store) -> impl Iterator<Item = &Record> {
    store.all().filter(|r| !is_local_only(store, r))
}

/// Allows for a badly set clock, not for stamps no edit could ever beat.
const MAX_CLOCK_SKEW_MS: u64 = 366 * 24 * 60 * 60 * 1000;

fn check(record: &Record) -> Result<()> {
    if !model::KINDS.contains(&record.kind.as_str()) {
        return Err(Error::msg(
            "This data was made by a newer version. Update TourLedger.",
        ));
    }
    if record.stamp.device.is_empty() || record.stamp.device.len() > 64 {
        return Err(Error::msg("Damaged sync data."));
    }
    if record.stamp.ms > now_ms().saturating_add(MAX_CLOCK_SKEW_MS) {
        return Err(Error::msg(
            "Some entries are dated far in the future. Check the clock on the other device.",
        ));
    }
    if record.deleted {
        let small = record.body.to_string().len() <= 512;
        if record.id.is_empty() || record.id.len() > 64 || !record.body.is_object() || !small {
            return Err(Error::msg("Damaged sync data."));
        }
    } else {
        model::validate(&record.kind, &record.id, &record.body)?;
    }
    Ok(())
}

/// Merges records from a peer or a file. All are checked before any is stored.
/// Anything kept to this device here stays untouched.
pub fn accept(store: &mut Store, incoming: Vec<Record>) -> Result<usize> {
    incoming.iter().try_for_each(check)?;
    let wanted: Vec<Record> = incoming
        .into_iter()
        .filter(|r| {
            let held_back = store
                .get_any(&r.kind, &r.id)
                .is_some_and(|held| is_local_only(store, held));
            !held_back && !(is_local_only(store, r) && !flagged(&r.body))
        })
        .collect();
    store.merge(wanted)
}

#[derive(Serialize, Deserialize)]
pub struct Bundle {
    pub format: String,
    pub version: u32,
    /// A full backup, including entries kept to one device.
    #[serde(default)]
    pub backup: bool,
    pub records: Vec<Record>,
    /// Bill pictures, base64, by hash.
    #[serde(default)]
    pub files: BTreeMap<String, String>,
}

/// Pictures that some record in `records` still shows.
pub fn picture_hashes<'a>(records: impl IntoIterator<Item = &'a Record>) -> BTreeSet<String> {
    records
        .into_iter()
        .filter(|r| r.kind == ATTACHMENT && !r.deleted)
        .filter_map(|r| r.str_field("hash").map(str::to_owned))
        .collect()
}

pub const BUNDLE_FORMAT: &str = "tourledger-sync";

pub fn export_bundle(store: &Store, include_local: bool) -> Result<String> {
    let records: Vec<Record> = if include_local {
        store.all().cloned().collect()
    } else {
        shareable(store).cloned().collect()
    };
    let blobs = store.blobs();
    let files = picture_hashes(&records)
        .into_iter()
        .filter_map(|hash| {
            let bytes = blobs.get(&hash)?;
            Some((hash, BASE64.encode(&bytes)))
        })
        .collect();
    Ok(serde_json::to_string(&Bundle {
        format: BUNDLE_FORMAT.into(),
        version: FORMAT_VERSION,
        backup: include_local,
        records,
        files,
    })?)
}

pub fn import_bundle(store: &mut Store, content: &str) -> Result<usize> {
    let bundle: Bundle = serde_json::from_str(content)
        .map_err(|_| Error::msg("This is not a TourLedger sync file."))?;
    if bundle.format != BUNDLE_FORMAT {
        return Err(Error::msg("This is not a TourLedger sync file."));
    }
    if bundle.version != FORMAT_VERSION {
        return Err(Error::msg(
            "This file was made by a different version. Update TourLedger.",
        ));
    }
    bundle.records.iter().try_for_each(check)?;
    let wanted = picture_hashes(&bundle.records);
    let blobs = store.blobs();
    for (hash, data) in &bundle.files {
        let bytes = BASE64
            .decode(data.as_bytes())
            .map_err(|_| Error::msg("A picture in the file is damaged."))?;
        if hash_of(&bytes) != *hash {
            return Err(Error::msg("A picture in the file is damaged."));
        }
        if wanted.contains(hash) {
            blobs.put(&bytes)?;
        }
    }
    if bundle.backup {
        // The person's own restore: the hold-back for peers does not apply, or
        // device-only entries would be lost. Newest still wins.
        return store.merge(bundle.records);
    }
    accept(store, bundle.records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::EXPENSE;
    use crate::store::Change;
    use serde_json::json;

    fn change(kind: &str, id: &str, body: Value) -> Change {
        Change {
            kind: kind.into(),
            id: id.into(),
            deleted: false,
            body,
        }
    }

    fn tour(local_only: bool) -> Value {
        json!({"name": "Trip", "start_date": "2026-11-14", "end_date": "2026-11-16",
               "local_only": local_only})
    }

    fn food(amount: i64) -> Value {
        json!({"tour_id": "t1", "date": "2026-11-14", "category": "food",
               "amount": amount, "paid_by": "me"})
    }

    #[test]
    fn a_device_only_tour_never_leaves_and_is_never_overwritten() {
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let mut a = Store::open(a.path(), "a").unwrap();
        let mut b = Store::open(b.path(), "b").unwrap();
        a.commit(vec![
            change(TOUR, "t1", tour(false)),
            change(EXPENSE, "e1", food(100)),
        ])
        .unwrap();
        accept(&mut b, shareable(&a).cloned().collect()).unwrap();

        // A cuts the tour off, then both keep editing.
        a.commit(vec![change(TOUR, "t1", tour(true))]).unwrap();
        assert_eq!(shareable(&a).count(), 0);
        b.commit(vec![
            change(EXPENSE, "e1", food(999)),
            change(EXPENSE, "e2", food(5)),
        ])
        .unwrap();
        accept(&mut a, shareable(&b).cloned().collect()).unwrap();
        assert_eq!(a.get(EXPENSE, "e1").unwrap().body["amount"], 100);
        assert!(a.get(EXPENSE, "e2").is_none());
    }

    #[test]
    fn a_backup_restores_newer_device_only_entries_but_a_sync_file_does_not() {
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let mut a = Store::open(a.path(), "a").unwrap();
        let mut b = Store::open(b.path(), "b").unwrap();
        a.commit(vec![change(TOUR, "t1", tour(true))]).unwrap();
        import_bundle(&mut b, &export_bundle(&a, true).unwrap()).unwrap();
        a.commit(vec![change(EXPENSE, "e1", food(100))]).unwrap();
        let backup = export_bundle(&a, true).unwrap();
        assert_eq!(import_bundle(&mut b, &backup).unwrap(), 1);
        assert!(b.get(EXPENSE, "e1").is_some());
    }

    #[test]
    fn stamps_from_the_far_future_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Store::open(dir.path(), "a").unwrap();
        let record = Record {
            kind: "person".into(),
            id: "p".into(),
            stamp: crate::hlc::Stamp {
                ms: u64::MAX,
                counter: u32::MAX,
                device: "b".into(),
            },
            deleted: true,
            body: json!({}),
        };
        assert!(accept(&mut s, vec![record]).is_err());
    }

    #[test]
    fn one_bad_record_rejects_the_whole_batch() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Store::open(dir.path(), "a").unwrap();
        let good = r#"{"kind":"person","id":"p","stamp":{"ms":1,"counter":0,"device":"b"},"body":{"name":"Ravi"}}"#;
        let bad = r#"{"kind":"person","id":"q","stamp":{"ms":1,"counter":0,"device":"b"},"body":{"name":""}}"#;
        let file =
            format!(r#"{{"format":"tourledger-sync","version":2,"records":[{good},{bad}]}}"#);
        assert!(import_bundle(&mut s, &file).is_err());
        assert_eq!(s.all().count(), 0);
    }
}
