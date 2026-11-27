//! Updating the desktop app from its GitHub releases. Android does this itself
//! (src-tauri/android/Updater.kt); both report the same shape to the page.
//!
//! Only the program is replaced: the ledger lives in the app data folder, which
//! installers do not touch. A download is used only if it is signed with the key
//! whose public half is in tauri.conf.json.

use serde::Serialize;

#[derive(Clone, Copy, Default, Serialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available,
    Downloading,
    Ready,
    Installing,
    Failed,
}

#[derive(Clone, Default, Serialize)]
pub struct Status {
    pub supported: bool,
    pub current: String,
    pub phase: Phase,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub received: u64,
    pub total: Option<u64>,
    pub error: Option<String>,
    pub checked_ms: Option<u64>,
    /// Android only: the connection is metered, so nothing downloads by itself.
    pub metered: bool,
}

#[cfg(any(target_os = "macos", windows, target_os = "linux"))]
mod desktop {
    use std::sync::{Arc, Mutex, MutexGuard};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use tauri::{AppHandle, State};
    use tauri_plugin_updater::{Update, UpdaterExt};

    use super::{Phase, Status};

    #[derive(Default)]
    struct Inner {
        status: Status,
        found: Option<Update>,
        bytes: Option<Vec<u8>>,
    }

    #[derive(Default, Clone)]
    pub struct Updates(Arc<Mutex<Inner>>);

    impl Updates {
        fn lock(&self) -> MutexGuard<'_, Inner> {
            self.0
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
        }

        fn snapshot(&self, app: &AppHandle) -> Status {
            let mut status = self.lock().status.clone();
            status.supported = true;
            status.current = app.package_info().version.to_string();
            status
        }

        fn fail(&self, error: impl ToString) {
            let mut inner = self.lock();
            inner.status.phase = Phase::Failed;
            inner.status.error = Some(error.to_string());
        }
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or_default()
    }

    #[tauri::command]
    pub fn update_status(app: AppHandle, updates: State<'_, Updates>) -> Status {
        updates.snapshot(&app)
    }

    /// Asks GitHub whether there is a newer release. Does nothing while a
    /// download or install is under way.
    #[tauri::command]
    pub async fn update_check(app: AppHandle, updates: State<'_, Updates>) -> Result<Status, ()> {
        let updates = updates.inner().clone();
        {
            let mut inner = updates.lock();
            if matches!(
                inner.status.phase,
                Phase::Checking | Phase::Downloading | Phase::Installing
            ) {
                drop(inner);
                return Ok(updates.snapshot(&app));
            }
            inner.status.phase = Phase::Checking;
            inner.status.error = None;
        }
        // A connection that goes quiet (sleep, a sign-in page) must end in an error,
        // or the app would sit at "checking" until restarted. These apply to the
        // download too.
        let updater = app
            .updater_builder()
            .configure_client(|client| {
                client
                    .connect_timeout(Duration::from_secs(15))
                    .read_timeout(Duration::from_secs(30))
            })
            .build();
        let result = match updater {
            Ok(updater) => updater.check().await,
            Err(error) => Err(error),
        };
        match result {
            Ok(found) => {
                let mut inner = updates.lock();
                inner.status.checked_ms = Some(now_ms());
                match found {
                    Some(update) => {
                        // A download already held for this version stays usable.
                        let held = inner.bytes.is_some()
                            && inner.status.version.as_deref() == Some(&update.version);
                        if !held {
                            inner.bytes = None;
                            inner.status.received = 0;
                            inner.status.total = None;
                        }
                        inner.status.phase = if held { Phase::Ready } else { Phase::Available };
                        inner.status.version = Some(update.version.clone());
                        inner.status.notes = update.body.clone();
                        inner.found = Some(update);
                    }
                    None => {
                        inner.status.phase = Phase::UpToDate;
                        inner.status.version = None;
                        inner.status.notes = None;
                        inner.found = None;
                        inner.bytes = None;
                    }
                }
            }
            Err(error) => {
                // An update already downloaded is still good when GitHub cannot be
                // reached: keep offering it.
                let held = updates.lock().bytes.is_some();
                if held {
                    updates.lock().status.phase = Phase::Ready;
                } else {
                    updates.fail(error);
                }
            }
        }
        Ok(updates.snapshot(&app))
    }

    /// Starts downloading what the last check found and returns at once.
    #[tauri::command]
    pub fn update_download(app: AppHandle, updates: State<'_, Updates>) -> Status {
        let updates = updates.inner().clone();
        let update = {
            let mut inner = updates.lock();
            let wanted = matches!(inner.status.phase, Phase::Available | Phase::Failed)
                && inner.bytes.is_none();
            match inner.found.clone() {
                Some(update) if wanted => {
                    inner.status.phase = Phase::Downloading;
                    inner.status.error = None;
                    inner.status.received = 0;
                    inner.status.total = None;
                    Some(update)
                }
                _ => None,
            }
        };
        if let Some(update) = update {
            let background = updates.clone();
            tauri::async_runtime::spawn(async move {
                let progress = background.clone();
                let result = update
                    .download(
                        move |chunk, total| {
                            let mut inner = progress.lock();
                            inner.status.received += chunk as u64;
                            inner.status.total = total;
                        },
                        || {},
                    )
                    .await;
                match result {
                    // The signature was checked before the bytes were handed over.
                    Ok(bytes) => {
                        let mut inner = background.lock();
                        inner.bytes = Some(bytes);
                        inner.status.phase = Phase::Ready;
                    }
                    Err(error) => background.fail(error),
                }
            });
        }
        updates.snapshot(&app)
    }

    /// Replaces the program and starts it again. On Windows the app closes while
    /// the installer runs, and the installer opens it afterwards.
    #[tauri::command]
    pub async fn update_install(app: AppHandle, updates: State<'_, Updates>) -> Result<Status, ()> {
        let updates = updates.inner().clone();
        let held = {
            let mut inner = updates.lock();
            match (inner.status.phase, inner.found.clone()) {
                (Phase::Ready, Some(update)) => inner.bytes.take().map(|bytes| {
                    inner.status.phase = Phase::Installing;
                    inner.status.error = None;
                    (update, bytes)
                }),
                _ => None,
            }
        };
        if let Some((update, bytes)) = held {
            // Unpacking, and on Linux a password prompt, must not stall the window.
            let installed = tauri::async_runtime::spawn_blocking(move || {
                let result = update.install(&bytes);
                (result, bytes)
            })
            .await;
            match installed {
                Ok((Ok(()), _)) => app.restart(),
                // Kept, so a cancelled password prompt can be tried again, and the
                // reason is shown: otherwise the button would seem to do nothing.
                Ok((Err(error), bytes)) => {
                    let mut inner = updates.lock();
                    inner.bytes = Some(bytes);
                    inner.status.phase = Phase::Ready;
                    inner.status.error = Some(error.to_string());
                }
                Err(error) => updates.fail(error),
            }
        }
        Ok(updates.snapshot(&app))
    }
}

/// Systems that update some other way: Android in Kotlin, FreeBSD by rebuilding.
#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
mod desktop {
    use tauri::AppHandle;

    use super::Status;

    fn none(app: &AppHandle) -> Status {
        Status {
            current: app.package_info().version.to_string(),
            ..Status::default()
        }
    }

    #[tauri::command]
    pub fn update_status(app: AppHandle) -> Status {
        none(&app)
    }

    #[tauri::command]
    pub fn update_check(app: AppHandle) -> Status {
        none(&app)
    }

    #[tauri::command]
    pub fn update_download(app: AppHandle) -> Status {
        none(&app)
    }

    #[tauri::command]
    pub fn update_install(app: AppHandle) -> Status {
        none(&app)
    }
}

pub use desktop::*;
