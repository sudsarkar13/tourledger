//! Messages between two linked devices: length-prefixed JSON on one stream.
//!
//! Hello → Welcome → Digest → Reply (records + digest) → Records →
//! pictures each way (Want → Blob… → Sent) → Done
//!
//! or, to end a link: Unlink → Done

use iroh::EndpointAddr;
use iroh::endpoint::{RecvStream, SendStream};
use serde::{Deserialize, Serialize};
use tourledger_core::hlc::Stamp;
use tourledger_core::{Error, Record, Result};

pub const ALPN: &[u8] = b"tourledger/sync/1";

/// Before a device has proved who it is, accept only a small message.
pub const MAX_HELLO: usize = 8 * 1024;
pub const MAX_MESSAGE: usize = 64 * 1024 * 1024;

pub type Digest = Vec<(String, String, Stamp)>;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    Hello {
        version: u32,
        name: String,
        secret: Option<String>,
        addr: EndpointAddr,
    },
    Welcome {
        name: String,
    },
    Refused {
        reason: String,
        /// The caller was unlinked here and should drop the link as well.
        #[serde(default)]
        unlinked: bool,
    },
    /// Sent instead of `Hello` by a device that has just unlinked this one.
    /// Answered with `Done`.
    Unlink,
    Digest {
        entries: Digest,
    },
    Reply {
        records: Vec<Record>,
        entries: Digest,
    },
    Records {
        records: Vec<Record>,
    },
    /// Bill pictures the sender has records for but no file.
    Want {
        hashes: Vec<String>,
    },
    Blob {
        hash: String,
        /// The JPEG, base64.
        data: String,
    },
    /// No more pictures for the last `Want`.
    Sent,
    Done,
}

pub fn net(error: impl std::fmt::Display) -> Error {
    Error::msg(format!("Connection problem: {error}"))
}

pub fn unexpected() -> Error {
    Error::msg("The other device sent something unexpected.")
}

pub async fn send(stream: &mut SendStream, message: &Message) -> Result<()> {
    let bytes = serde_json::to_vec(message)?;
    if bytes.len() > MAX_MESSAGE {
        return Err(Error::msg("Too much to sync at once."));
    }
    stream
        .write_all(&(bytes.len() as u32).to_le_bytes())
        .await
        .map_err(net)?;
    stream.write_all(&bytes).await.map_err(net)
}

pub async fn receive(stream: &mut RecvStream, limit: usize) -> Result<Message> {
    let mut len = [0u8; 4];
    stream.read_exact(&mut len).await.map_err(net)?;
    let len = u32::from_le_bytes(len) as usize;
    if len > limit {
        return Err(unexpected());
    }
    let mut bytes = vec![0; len];
    stream.read_exact(&mut bytes).await.map_err(net)?;
    serde_json::from_slice(&bytes).map_err(|_| unexpected())
}
