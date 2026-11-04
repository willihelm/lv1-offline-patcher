//! Updating the app from its GitHub releases.
//!
//! The release workflow (`.github/workflows/release.yml`) attaches a `latest.json` to every
//! release: the version, the release notes and, per platform, the bundle and its signature. The
//! plugin reads it from the endpoint in `tauri.conf.json` and refuses any bundle whose signature
//! doesn't match the public key next to it.

use serde::Serialize;
use tauri::async_runtime::Mutex;
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::{AppState, CmdResult};

/// A newer release than the one running.
#[derive(Serialize, Clone)]
pub struct AvailableUpdate {
    /// without the `v` of the tag
    version: String,
    /// the version running now
    current: String,
    /// release notes (Markdown), None if the release has none
    notes: Option<String>,
}

#[derive(Serialize, Clone)]
struct Progress {
    received: u64,
    /// None if the server sends no length
    total: Option<u64>,
}

/// The last check: `None` until the first one has run. Kept so every window shows the same
/// answer without asking GitHub again, and installing installs what the dialog showed.
#[derive(Default)]
pub struct Pending(Mutex<Option<Option<Update>>>);

fn available(update: &Update) -> AvailableUpdate {
    AvailableUpdate {
        version: update.version.clone(),
        current: update.current_version.clone(),
        notes: update.body.clone().filter(|n| !n.trim().is_empty()),
    }
}

/// Is there a newer release? Every window asks as it opens and gets the result of the first
/// check; `force` (the "Check for Updates…" menu item) asks GitHub again. A development build
/// never has an update: the executable it would replace is the one under `target/`.
#[tauri::command]
pub async fn check_for_update(app: AppHandle, pending: State<'_, Pending>, force: bool) -> CmdResult<Option<AvailableUpdate>> {
    if cfg!(debug_assertions) {
        return if force { Err("This is a development build; it is not updated from GitHub.".into()) } else { Ok(None) };
    }
    // held across the request, so windows opening together wait for one check
    let mut last = pending.0.lock().await;
    if let (Some(update), false) = (&*last, force) {
        return Ok(update.as_ref().map(available));
    }
    let update = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?;
    let result = update.as_ref().map(available);
    *last = Some(update);
    Ok(result)
}

/// Download the update the last check found, install it and restart. Refused while a showfile
/// has unsaved changes, as the restart would drop them. Returns only on failure.
#[tauri::command]
pub async fn install_update(app: AppHandle, window: WebviewWindow, pending: State<'_, Pending>) -> CmdResult<()> {
    let unsaved = unsaved_files(&app)?;
    if !unsaved.is_empty() {
        return Err(format!("Save or close the showfiles with unsaved changes first: {}", unsaved.join(", ")));
    }
    let update = pending.0.lock().await.clone().flatten().ok_or("There is no update to install. Check again.")?;

    let (mut received, mut reported) = (0u64, 0u64);
    let bytes = update
        .download(
            |chunk, total| {
                received += chunk as u64;
                // about a hundred events for the whole download, not one per network chunk
                let step = total.map_or(512 * 1024, |t| (t / 100).max(1));
                if received - reported >= step || total == Some(received) {
                    reported = received;
                    let _ = window.emit_to(window.label(), "update-progress", Progress { received, total });
                }
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;

    // edits made during the download
    let unsaved = unsaved_files(&app)?;
    if !unsaved.is_empty() {
        return Err(format!("Save or close the showfiles with unsaved changes first: {}", unsaved.join(", ")));
    }
    update.install(bytes).map_err(|e| e.to_string())?;
    app.restart()
}

/// file names of the open showfiles with unsaved changes, in all windows
fn unsaved_files(app: &AppHandle) -> CmdResult<Vec<String>> {
    let state = app.state::<AppState>();
    let sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    Ok(sessions
        .values()
        .filter(|s| s.show.dirty())
        .map(|s| s.show.path.file_name().unwrap_or_default().to_string_lossy().into_owned())
        .collect())
}
