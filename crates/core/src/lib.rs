//! All TourLedger rules: storage, records, money, settlement, loans, insights.
//! No network and no UI code lives here.

pub mod api;
pub mod blobs;
pub mod entry;
pub mod error;
pub mod export;
pub mod hlc;
pub mod insights;
pub mod loans;
pub mod message;
pub mod model;
pub mod money;
pub mod pdf;
pub mod portfolio;
pub mod proof;
pub mod reminders;
pub mod report;
pub mod settlement;
pub mod share;
pub mod store;
pub mod views;

pub use error::{Error, Result};
pub use store::{Record, Store};
