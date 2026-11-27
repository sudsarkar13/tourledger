//! Bill pictures, kept as files beside the ledger and named by their SHA-256, so
//! the ledger stays small and a received file can be checked.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use data_encoding::HEXLOWER;
use krilla::image::Image;
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::store::private_options;

/// The app shrinks pictures well below this before saving them.
pub const MAX_BYTES: usize = 3 * 1024 * 1024;

pub struct Blobs {
    dir: PathBuf,
}

pub fn hash_of(bytes: &[u8]) -> String {
    HEXLOWER.encode(&Sha256::digest(bytes))
}

/// Also what keeps a name from reaching outside the folder.
pub fn is_hash(name: &str) -> bool {
    name.len() == 64 && name.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Width and height of a JPEG that can be drawn into a report.
pub fn jpeg_size(bytes: &[u8]) -> Result<(u32, u32)> {
    let bad = || Error::invalid("bill", "That picture could not be read.");
    if bytes.len() > MAX_BYTES {
        return Err(Error::invalid("bill", "That picture is too large."));
    }
    if !bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Err(bad());
    }
    let image = Image::from_jpeg(bytes.to_vec().into(), true).map_err(|_| bad())?;
    let (width, height) = image.size();
    if width == 0 || height == 0 {
        return Err(bad());
    }
    Ok((width, height))
}

impl Blobs {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            dir: data_dir.join("bills"),
        }
    }

    fn path(&self, hash: &str) -> Option<PathBuf> {
        is_hash(hash).then(|| self.dir.join(format!("{hash}.jpg")))
    }

    pub fn has(&self, hash: &str) -> bool {
        self.path(hash).is_some_and(|p| p.is_file())
    }

    /// `None` if the file is not here or no longer matches its name.
    pub fn get(&self, hash: &str) -> Option<Vec<u8>> {
        let bytes = fs::read(self.path(hash)?).ok()?;
        (hash_of(&bytes) == hash).then_some(bytes)
    }

    /// Written to a temporary file and renamed, so a crash leaves no half file.
    pub fn put(&self, bytes: &[u8]) -> Result<String> {
        jpeg_size(bytes)?;
        let hash = hash_of(bytes);
        if self.get(&hash).is_some() {
            return Ok(hash);
        }
        fs::create_dir_all(&self.dir)?;
        let temp = self.dir.join(format!("{hash}.new"));
        let mut file = private_options()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        let target = self.path(&hash).ok_or(Error::NotFound)?;
        fs::rename(temp, target)?;
        Ok(hash)
    }

    /// Only for a picture the person has deleted.
    pub fn remove(&self, hash: &str) {
        if let Some(path) = self.path(hash) {
            // Already gone is fine; a file left behind costs only space.
            let _ = fs::remove_file(path);
        }
    }
}
