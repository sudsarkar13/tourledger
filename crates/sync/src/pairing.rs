//! The code one device shows and the other reads: who to dial and a one-time
//! secret that proves the dialler saw the code.

use data_encoding::BASE64URL_NOPAD;
use iroh::EndpointAddr;
use serde::{Deserialize, Serialize};
use tourledger_core::{Error, Result};

const PREFIX: &str = "TL1.";
pub const LIFETIME_MS: u64 = 10 * 60 * 1000;
pub const MAX_ATTEMPTS: u32 = 5;

#[derive(Serialize, Deserialize)]
pub struct Code {
    pub addr: EndpointAddr,
    pub secret: String,
}

impl Code {
    pub fn encode(&self) -> Result<String> {
        Ok(format!(
            "{PREFIX}{}",
            BASE64URL_NOPAD.encode(&serde_json::to_vec(self)?)
        ))
    }

    pub fn decode(text: &str) -> Result<Self> {
        let invalid = || Error::invalid("code", "That is not a TourLedger pairing code.");
        // Codes pasted through chat apps pick up line breaks and spaces.
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        let body = compact.strip_prefix(PREFIX).ok_or_else(invalid)?;
        let bytes = BASE64URL_NOPAD
            .decode(body.as_bytes())
            .map_err(|_| invalid())?;
        serde_json::from_slice(&bytes).map_err(|_| invalid())
    }
}

pub struct Offer {
    pub secret: String,
    pub code: String,
    pub expires_ms: u64,
    pub failures: u32,
}

pub fn new_secret() -> String {
    BASE64URL_NOPAD.encode(&rand::random::<[u8; 16]>())
}
