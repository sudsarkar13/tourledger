//! The storage engine: one append-only file of checksummed transactions, replayed
//! into memory on open. Format and failure behaviour: docs/ARCHITECTURE.md.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::blobs::Blobs;
use crate::error::{Error, Result};
use crate::hlc::{Clock, Stamp, now_ms};

const HEADER: &[u8; 8] = b"TLEDGER1";
const MAGIC: &[u8; 4] = b"TLF1";
const FRAME_HEADER: usize = 12;
/// A frame larger than this is treated as damage rather than allocated.
const MAX_PAYLOAD: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub kind: String,
    pub id: String,
    pub stamp: Stamp,
    #[serde(default)]
    pub deleted: bool,
    pub body: Value,
}

pub type Key = (String, String);

impl Record {
    pub fn key(&self) -> Key {
        (self.kind.clone(), self.id.clone())
    }

    pub fn str_field(&self, name: &str) -> Option<&str> {
        self.body
            .get(name)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
    }
}

/// One change inside a transaction; the store assigns the stamp.
pub struct Change {
    pub kind: String,
    pub id: String,
    pub deleted: bool,
    pub body: Value,
}

pub struct Store {
    path: PathBuf,
    file: File,
    _lock: File,
    len: u64,
    records: BTreeMap<Key, Record>,
    clock: Clock,
    device: String,
    /// Versions on disk that a newer version has replaced.
    stale: usize,
    revision: u64,
    poisoned: bool,
}

impl Store {
    /// Opens (or creates) the ledger inside `dir`. `device` identifies this
    /// installation in stamps and must differ between devices.
    pub fn open(dir: &Path, device: &str) -> Result<Self> {
        fs::create_dir_all(dir)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(dir.join("ledger.lock"))?;
        lock.try_lock().map_err(|_| Error::Locked)?;

        let path = dir.join("ledger.tlg");
        let mut file = private_options()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)?;

        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;

        if bytes.len() < HEADER.len() && HEADER.starts_with(&bytes) {
            // New file, or creation was interrupted.
            file.set_len(0)?;
            file.seek(SeekFrom::Start(0))?;
            file.write_all(HEADER)?;
            file.sync_all()?;
            sync_dir(dir)?;
            bytes = HEADER.to_vec();
        }
        if !bytes.starts_with(HEADER) {
            return Err(corrupt(&path, 0));
        }

        let mut store = Self {
            path: path.clone(),
            file,
            _lock: lock,
            len: HEADER.len() as u64,
            records: BTreeMap::new(),
            clock: Clock::default(),
            device: device.to_owned(),
            stale: 0,
            revision: 0,
            poisoned: false,
        };

        let mut at = HEADER.len();
        while at < bytes.len() {
            match read_frame(&bytes[at..]) {
                Some((payload, size)) => {
                    let batch: Vec<Record> =
                        serde_json::from_slice(payload).map_err(|_| corrupt(&path, at as u64))?;
                    for record in batch {
                        store.absorb(record);
                    }
                    at += size;
                }
                None => {
                    // A later intact frame means damage in the middle: stop and
                    // keep the file as it is. Otherwise it is an unfinished write.
                    let later = (at + 1..bytes.len()).any(|i| read_frame(&bytes[i..]).is_some());
                    if later {
                        return Err(corrupt(&path, at as u64));
                    }
                    let aside = path.with_extension(format!("tlg.torn.{}", now_ms()));
                    private_options()
                        .create(true)
                        .truncate(true)
                        .write(true)
                        .open(aside)?
                        .write_all(&bytes[at..])?;
                    store.file.set_len(at as u64)?;
                    store.file.sync_all()?;
                    break;
                }
            }
        }
        store.len = at as u64;
        store.file.seek(SeekFrom::Start(store.len))?;
        Ok(store)
    }

    /// The pictures that belong to this ledger.
    pub fn blobs(&self) -> Blobs {
        Blobs::new(self.path.parent().unwrap_or(Path::new(".")))
    }

    pub fn device(&self) -> &str {
        &self.device
    }

    /// Changes whenever stored data changes, so a UI knows to reload.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// For changes outside the records (a picture arriving), so a UI reloads.
    pub fn touch(&mut self) {
        self.revision += 1;
    }

    pub fn stale(&self) -> usize {
        self.stale
    }

    pub fn get(&self, kind: &str, id: &str) -> Option<&Record> {
        self.records
            .get(&(kind.to_owned(), id.to_owned()))
            .filter(|r| !r.deleted)
    }

    /// Includes tombstones.
    pub fn get_any(&self, kind: &str, id: &str) -> Option<&Record> {
        self.records.get(&(kind.to_owned(), id.to_owned()))
    }

    pub fn live<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a Record> {
        self.records
            .values()
            .filter(move |r| r.kind == kind && !r.deleted)
    }

    /// Every record including tombstones.
    pub fn all(&self) -> impl Iterator<Item = &Record> {
        self.records.values()
    }

    /// Writes the changes as one transaction.
    pub fn commit(&mut self, changes: Vec<Change>) -> Result<Vec<Record>> {
        let now = now_ms();
        let batch: Vec<Record> = changes
            .into_iter()
            .map(|c| Record {
                stamp: self.clock.next(now, &self.device),
                kind: c.kind,
                id: c.id,
                deleted: c.deleted,
                body: c.body,
            })
            .collect();
        self.append(&batch)?;
        for record in &batch {
            self.absorb(record.clone());
        }
        self.maybe_compact();
        Ok(batch)
    }

    /// Stores records that came from elsewhere, keeping their stamps. Only those
    /// newer than what is held are written. Returns how many were.
    pub fn merge(&mut self, incoming: Vec<Record>) -> Result<usize> {
        let mut newest: BTreeMap<Key, Record> = BTreeMap::new();
        for record in incoming {
            let key = record.key();
            let beats_held = self
                .records
                .get(&key)
                .is_none_or(|h| record.stamp > h.stamp);
            let beats_batch = newest.get(&key).is_none_or(|h| record.stamp > h.stamp);
            if beats_held && beats_batch {
                newest.insert(key, record);
            }
        }
        let batch: Vec<Record> = newest.into_values().collect();
        if batch.is_empty() {
            return Ok(0);
        }
        self.append(&batch)?;
        let count = batch.len();
        for record in batch {
            self.absorb(record);
        }
        self.maybe_compact();
        Ok(count)
    }

    fn absorb(&mut self, record: Record) {
        self.clock.observe(&record.stamp);
        let key = record.key();
        match self.records.get(&key) {
            Some(held) if held.stamp >= record.stamp => self.stale += 1,
            held => {
                if held.is_some() {
                    self.stale += 1;
                }
                self.records.insert(key, record);
                self.revision += 1;
            }
        }
    }

    fn append(&mut self, batch: &[Record]) -> Result<()> {
        if self.poisoned {
            return Err(Error::Poisoned);
        }
        let frame = encode_frame(&serde_json::to_vec(batch)?)?;
        let written = self
            .file
            .write_all(&frame)
            .and_then(|()| self.file.sync_data());
        if let Err(error) = written {
            // Remove the partial frame so later writes do not follow garbage.
            let undone = self
                .file
                .set_len(self.len)
                .and_then(|()| self.file.seek(SeekFrom::Start(self.len)));
            self.poisoned = undone.is_err();
            return Err(error.into());
        }
        self.len += frame.len() as u64;
        Ok(())
    }

    /// Housekeeping after a write that is already safe on disk, so a failure
    /// here must not be reported as a failed save. It is tried again later.
    fn maybe_compact(&mut self) {
        if self.stale > 2_000 && self.stale > self.records.len() {
            let _ = self.compact();
        }
    }

    /// Rewrites the file holding only the newest version of each record.
    pub fn compact(&mut self) -> Result<()> {
        let temp = self.path.with_extension("tlg.new");
        let mut out = private_options()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(&temp)?;
        out.write_all(HEADER)?;
        let mut len = HEADER.len() as u64;
        let all: Vec<&Record> = self.records.values().collect();
        for chunk in all.chunks(500) {
            let frame = encode_frame(&serde_json::to_vec(chunk)?)?;
            out.write_all(&frame)?;
            len += frame.len() as u64;
        }
        out.sync_all()?;
        fs::rename(&temp, &self.path)?;
        // `out` now is the ledger; keep writing through it.
        self.file = out;
        self.len = len;
        self.stale = 0;
        if let Some(dir) = self.path.parent() {
            sync_dir(dir)?;
        }
        Ok(())
    }
}

fn corrupt(path: &Path, offset: u64) -> Error {
    Error::Corrupt {
        path: path.display().to_string(),
        offset,
    }
}

fn encode_frame(payload: &[u8]) -> Result<Vec<u8>> {
    if payload.len() > MAX_PAYLOAD {
        return Err(Error::msg(
            "Too much data in one step. Import a smaller file.",
        ));
    }
    let mut frame = Vec::with_capacity(FRAME_HEADER + payload.len());
    frame.extend_from_slice(MAGIC);
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(&crc32fast::hash(payload).to_le_bytes());
    frame.extend_from_slice(payload);
    Ok(frame)
}

/// Returns the payload and the whole frame size, or `None` if the bytes do not
/// start with a complete, intact frame.
fn read_frame(bytes: &[u8]) -> Option<(&[u8], usize)> {
    if bytes.len() < FRAME_HEADER || &bytes[..4] != MAGIC {
        return None;
    }
    let len = u32::from_le_bytes(bytes[4..8].try_into().ok()?) as usize;
    let crc = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
    if len > MAX_PAYLOAD {
        return None;
    }
    let payload = bytes.get(FRAME_HEADER..FRAME_HEADER + len)?;
    (crc32fast::hash(payload) == crc).then_some((payload, FRAME_HEADER + len))
}

/// Options that create files readable by the owner only, where the OS allows it.
pub fn private_options() -> OpenOptions {
    #[allow(unused_mut)]
    let mut options = OpenOptions::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
}

/// Makes a rename or file creation durable. Not possible on Windows.
fn sync_dir(dir: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(dir)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn put(store: &mut Store, id: &str, n: i64) {
        store
            .commit(vec![Change {
                kind: "t".into(),
                id: id.into(),
                deleted: false,
                body: json!({ "n": n }),
            }])
            .unwrap();
    }

    fn n(store: &Store, id: &str) -> Option<i64> {
        store.get("t", id).map(|r| r.body["n"].as_i64().unwrap())
    }

    #[test]
    fn survives_reopen() {
        let dir = tempfile::tempdir().unwrap();
        {
            let mut s = Store::open(dir.path(), "a").unwrap();
            put(&mut s, "x", 1);
            put(&mut s, "x", 2);
        }
        let s = Store::open(dir.path(), "a").unwrap();
        assert_eq!(n(&s, "x"), Some(2));
    }

    #[test]
    fn second_instance_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let _first = Store::open(dir.path(), "a").unwrap();
        assert!(matches!(Store::open(dir.path(), "a"), Err(Error::Locked)));
    }

    #[test]
    fn cuts_off_an_unfinished_write_at_every_length() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.tlg");
        let first_len;
        {
            let mut s = Store::open(dir.path(), "a").unwrap();
            put(&mut s, "x", 1);
            first_len = fs::metadata(&path).unwrap().len() as usize;
            put(&mut s, "y", 2);
        }
        let full = fs::read(&path).unwrap();
        for cut in first_len + 1..full.len() {
            fs::write(&path, &full[..cut]).unwrap();
            let mut s = Store::open(dir.path(), "a").unwrap();
            assert_eq!((n(&s, "x"), n(&s, "y")), (Some(1), None), "cut at {cut}");
            put(&mut s, "z", 3);
            drop(s);
            let s = Store::open(dir.path(), "a").unwrap();
            assert_eq!(n(&s, "z"), Some(3), "append after cut at {cut}");
        }
    }

    #[test]
    fn refuses_damage_in_the_middle_and_leaves_the_file_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.tlg");
        {
            let mut s = Store::open(dir.path(), "a").unwrap();
            put(&mut s, "x", 1);
            put(&mut s, "y", 2);
        }
        let original = fs::read(&path).unwrap();
        // One flipped byte in the first payload, and in its length field.
        for at in [HEADER.len() + FRAME_HEADER + 3, HEADER.len() + 5] {
            let mut damaged = original.clone();
            damaged[at] ^= 0x7f;
            fs::write(&path, &damaged).unwrap();
            assert!(matches!(
                Store::open(dir.path(), "a"),
                Err(Error::Corrupt { .. })
            ));
            assert_eq!(fs::read(&path).unwrap(), damaged);
        }
    }

    #[test]
    fn compaction_keeps_the_newest_versions_and_tombstones() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.tlg");
        let mut s = Store::open(dir.path(), "a").unwrap();
        for i in 0..50 {
            put(&mut s, "x", i);
        }
        s.commit(vec![Change {
            kind: "t".into(),
            id: "gone".into(),
            deleted: true,
            body: json!({}),
        }])
        .unwrap();
        let before = fs::metadata(&path).unwrap().len();
        s.compact().unwrap();
        assert!(fs::metadata(&path).unwrap().len() < before);
        put(&mut s, "y", 7);
        drop(s);
        let s = Store::open(dir.path(), "a").unwrap();
        assert_eq!((n(&s, "x"), n(&s, "y")), (Some(49), Some(7)));
        assert!(s.get_any("t", "gone").unwrap().deleted);
    }

    #[test]
    fn merge_converges_in_any_order() {
        let make = |ms, device: &str, v| Record {
            kind: "t".into(),
            id: "x".into(),
            stamp: Stamp {
                ms,
                counter: 0,
                device: device.into(),
            },
            deleted: false,
            body: json!({ "n": v }),
        };
        let records = [
            make(1, "a", 1),
            make(3, "a", 3),
            make(3, "b", 4),
            make(2, "c", 2),
        ];
        for order in [[0, 1, 2, 3], [3, 2, 1, 0], [2, 0, 3, 1], [1, 3, 0, 2]] {
            let dir = tempfile::tempdir().unwrap();
            let mut s = Store::open(dir.path(), "z").unwrap();
            for i in order {
                s.merge(vec![records[i].clone()]).unwrap();
            }
            assert_eq!(n(&s, "x"), Some(4), "order {order:?}");
            assert_eq!(s.merge(records.to_vec()).unwrap(), 0);
        }
    }
}
