#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Input the user can fix. `field` names the form field to mark.
    #[error("{message}")]
    Invalid {
        field: Option<&'static str>,
        message: String,
    },
    #[error("That item no longer exists.")]
    NotFound,
    #[error("TourLedger is already open. Close the other window first.")]
    Locked,
    #[error("The data file is damaged at byte {offset}. It was left untouched: {path}")]
    Corrupt { path: String, offset: u64 },
    #[error("Storage error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not read data: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Storage stopped after a failed write. Restart the app.")]
    Poisoned,
}

impl Error {
    pub fn invalid(field: &'static str, message: impl Into<String>) -> Self {
        Self::Invalid {
            field: Some(field),
            message: message.into(),
        }
    }

    pub fn msg(message: impl Into<String>) -> Self {
        Self::Invalid {
            field: None,
            message: message.into(),
        }
    }

    pub fn field(&self) -> Option<&'static str> {
        match self {
            Self::Invalid { field, .. } => *field,
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
