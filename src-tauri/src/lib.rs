//! The native shell: opens the ledger, passes the UI's requests to Rust and
//! saves exported files where the person chooses. No rules live here.

mod updates;

use std::io::Write;
use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_fs::{FsExt, OpenOptions};
use tourledger_sync::{Network, Service};

/// Why opening the ledger failed is kept, so the window can show it.
struct Ledger(Result<Arc<Service>, String>);

#[derive(Serialize)]
struct Failure {
    message: String,
    field: Option<&'static str>,
}

impl Failure {
    fn new(message: impl ToString) -> Self {
        Failure {
            message: message.to_string(),
            field: None,
        }
    }
}

#[tauri::command]
async fn api(ledger: State<'_, Ledger>, request: Value) -> Result<Value, Failure> {
    let service = ledger.0.as_ref().map_err(Failure::new)?;
    service.handle(request).await.map_err(|e| Failure {
        message: e.to_string(),
        field: e.field(),
    })
}

/// The page cannot touch files itself: it hands over the text and the person
/// picks the destination in the system dialog.
#[tauri::command]
async fn save_file(
    app: AppHandle,
    name: String,
    content: String,
    base64: bool,
) -> Result<bool, Failure> {
    // PDFs arrive as base64 text.
    let bytes = if base64 {
        data_encoding::BASE64
            .decode(content.as_bytes())
            .map_err(Failure::new)?
    } else {
        content.into_bytes()
    };
    let Some(path) = app
        .dialog()
        .file()
        .set_file_name(&name)
        .blocking_save_file()
    else {
        return Ok(false);
    };
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    let mut file = app.fs().open(path, options).map_err(Failure::new)?;
    file.write_all(&bytes).map_err(Failure::new)?;
    Ok(true)
}

fn device_name() -> String {
    if cfg!(target_os = "android") {
        return "Android phone".into();
    }
    if cfg!(target_os = "ios") {
        return "iPhone or iPad".into();
    }
    let host = gethostname::gethostname().to_string_lossy().into_owned();
    let host = host.trim_end_matches(".local").trim();
    if host.is_empty() {
        "My computer".into()
    } else {
        host.chars().take(40).collect()
    }
}

fn open_ledger(app: &AppHandle) -> Result<Arc<Service>, String> {
    // Lets a test or a second profile use another folder.
    let dir = match std::env::var_os("TOURLEDGER_DATA_DIR") {
        Some(dir) => dir.into(),
        None => app.path().app_data_dir().map_err(|e| e.to_string())?,
    };
    Service::open(&dir, &device_name(), Network::Public).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    // A second launch brings the first window forward; the ledger allows one
    // process only. Must be the first plugin. The plugin has no FreeBSD version.
    #[cfg(all(desktop, not(target_os = "freebsd")))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _, _| {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
    }));

    let builder = builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init());

    #[cfg(mobile)]
    let builder = builder.plugin(tauri_plugin_barcode_scanner::init());

    #[cfg(any(target_os = "macos", windows, target_os = "linux"))]
    let builder = builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updates::Updates::default());

    builder
        .setup(|app| {
            let ledger = open_ledger(app.handle());
            if let Ok(service) = &ledger {
                let service = service.clone();
                tauri::async_runtime::spawn(async move { service.start().await });
            }
            app.manage(Ledger(ledger));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            api,
            save_file,
            updates::update_status,
            updates::update_check,
            updates::update_download,
            updates::update_install
        ])
        .run(tauri::generate_context!())
        .expect("TourLedger could not start");
}
