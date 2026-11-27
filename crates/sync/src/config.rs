//! This device's identity and links. Never synced.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use iroh::{EndpointAddr, EndpointId, SecretKey};
use serde::{Deserialize, Serialize};
use tourledger_core::store::private_options;
use tourledger_core::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peer {
    pub id: EndpointId,
    pub name: String,
    /// Where it was last reached, for networks without discovery.
    #[serde(default)]
    pub addr: Option<EndpointAddr>,
    #[serde(default)]
    pub last_sync_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    pub device_id: String,
    secret: [u8; 32],
    pub name: String,
    #[serde(default)]
    pub sync_enabled: bool,
    #[serde(default)]
    pub peers: Vec<Peer>,
    /// Devices unlinked here. One that was not reachable at the time is told
    /// when it next calls, so that it drops the link too.
    #[serde(default)]
    pub unlinked: Vec<EndpointId>,
}

/// More unlinked devices than anyone owns; the oldest are forgotten.
const MAX_UNLINKED: usize = 50;

impl DeviceConfig {
    fn path(dir: &Path) -> PathBuf {
        dir.join("device.json")
    }

    pub fn load_or_create(dir: &Path, default_name: &str) -> Result<Self> {
        fs::create_dir_all(dir)?;
        match fs::read(Self::path(dir)) {
            // Never replace an unreadable identity: that would silently unlink
            // every device.
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let config = Self {
                    device_id: uuid_like(),
                    secret: SecretKey::generate().to_bytes(),
                    name: default_name.to_owned(),
                    sync_enabled: false,
                    peers: Vec::new(),
                    unlinked: Vec::new(),
                };
                config.save(dir)?;
                Ok(config)
            }
            Err(e) => Err(Error::Io(e)),
        }
    }

    pub fn secret_key(&self) -> SecretKey {
        SecretKey::from_bytes(&self.secret)
    }

    /// Written to a temporary file first so a crash cannot leave half a file.
    pub fn save(&self, dir: &Path) -> Result<()> {
        let temp = dir.join("device.json.new");
        let mut file = private_options()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temp)?;
        file.write_all(&serde_json::to_vec_pretty(self)?)?;
        file.sync_all()?;
        fs::rename(temp, Self::path(dir))?;
        Ok(())
    }

    pub fn peer(&self, id: &EndpointId) -> Option<&Peer> {
        self.peers.iter().find(|p| p.id == *id)
    }

    pub fn mark_unlinked(&mut self, id: EndpointId) {
        self.unlinked.retain(|other| *other != id);
        self.unlinked.push(id);
        let extra = self.unlinked.len().saturating_sub(MAX_UNLINKED);
        self.unlinked.drain(..extra);
    }

    pub fn upsert_peer(&mut self, id: EndpointId, name: String, addr: Option<EndpointAddr>) {
        self.unlinked.retain(|other| *other != id);
        match self.peers.iter_mut().find(|p| p.id == id) {
            Some(peer) => {
                peer.name = name;
                if addr.is_some() {
                    peer.addr = addr;
                }
            }
            None => self.peers.push(Peer {
                id,
                name,
                addr,
                last_sync_ms: 0,
            }),
        }
    }
}

fn uuid_like() -> String {
    data_encoding::HEXLOWER.encode(&rand::random::<[u8; 8]>())
}
