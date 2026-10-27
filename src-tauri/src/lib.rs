mod iom;
mod layout;
mod show;

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde::Serialize;
use show::{PatchOp, Show, ShowView};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

/// An opened showfile, shown as a tab in the window with label `window`.
struct Session {
    show: Show,
    window: String,
}

#[derive(Default)]
struct AppState {
    sessions: Mutex<HashMap<u64, Session>>,
    next_id: AtomicU64,
    next_window: AtomicU64,
}

type CmdResult<T> = Result<T, String>;

fn with_show<T>(state: &State<AppState>, id: u64, f: impl FnOnce(&mut Show) -> CmdResult<T>) -> CmdResult<T> {
    let mut sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    let session = sessions.get_mut(&id).ok_or("Showfile is not open")?;
    f(&mut session.show)
}

/// Serialized show state; returned by every command so the UI simply re-renders.
fn view(show: &Show) -> CmdResult<serde_json::Value> {
    serde_json::to_value::<ShowView>(show.view()).map_err(|e| e.to_string())
}

#[derive(Serialize, Clone)]
struct SessionView {
    id: u64,
    show: serde_json::Value,
}

#[derive(Serialize)]
struct SaveResult {
    show: serde_json::Value,
    backup: Option<String>,
}

fn same_file(a: &PathBuf, b: &PathBuf) -> bool {
    a == b || matches!((fs::canonicalize(a), fs::canonicalize(b)), (Ok(a), Ok(b)) if a == b)
}

/// Open a showfile as a new session of the calling window. A file that is already open is
/// not opened twice (saves would overwrite each other): its tab is activated instead,
/// possibly in another window, and `null` is returned.
#[tauri::command]
fn open_show(app: AppHandle, window: WebviewWindow, state: State<AppState>, path: String) -> CmdResult<Option<SessionView>> {
    let path = PathBuf::from(path);
    let mut sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    if let Some((&id, s)) = sessions.iter().find(|(_, s)| same_file(&s.show.path, &path)) {
        if let Some(w) = app.get_webview_window(&s.window) {
            let _ = w.set_focus();
        }
        app.emit_to(s.window.as_str(), "activate-session", id).map_err(|e| e.to_string())?;
        return Ok(None);
    }
    let show = Show::open(&path)?;
    let v = view(&show)?;
    let id = state.next_id.fetch_add(1, Ordering::Relaxed) + 1;
    sessions.insert(id, Session { show, window: window.label().to_string() });
    Ok(Some(SessionView { id, show: v }))
}

/// Sessions owned by the calling window, e.g. the tabs a new window was created with.
#[tauri::command]
fn window_sessions(window: WebviewWindow, state: State<AppState>) -> CmdResult<Vec<SessionView>> {
    let sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    let mut list: Vec<_> = sessions.iter().filter(|(_, s)| s.window == window.label()).collect();
    list.sort_by_key(|(id, _)| **id);
    list.into_iter().map(|(&id, s)| Ok(SessionView { id, show: view(&s.show)? })).collect()
}

#[tauri::command]
fn close_session(state: State<AppState>, id: u64) -> CmdResult<()> {
    state.sessions.lock().map_err(|e| e.to_string())?.remove(&id);
    Ok(())
}

fn create_window(app: &AppHandle, state: &AppState) -> CmdResult<WebviewWindow> {
    let label = format!("w{}", state.next_window.fetch_add(1, Ordering::Relaxed) + 1);
    WebviewWindowBuilder::new(app, label, WebviewUrl::default())
        .title("LV1 Offline Patcher")
        .inner_size(1500.0, 900.0)
        .min_inner_size(900.0, 500.0)
        .build()
        .map_err(|e| e.to_string())
}

/// Open an empty window. Async: creating a window in a sync command deadlocks on Windows.
#[tauri::command]
async fn new_window(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    create_window(&app, &state)?;
    Ok(())
}

/// Move a session into the window `target`, or into a new window if `None`. The caller
/// removes the tab afterwards.
#[tauri::command]
async fn move_session(app: AppHandle, state: State<'_, AppState>, id: u64, target: Option<String>) -> CmdResult<()> {
    let target = match target {
        Some(label) => app.get_webview_window(&label).ok_or("Window not found")?,
        None => create_window(&app, &state)?,
    };
    let moved = {
        let mut sessions = state.sessions.lock().map_err(|e| e.to_string())?;
        let s = sessions.get_mut(&id).ok_or("Showfile is not open")?;
        s.window = target.label().to_string();
        SessionView { id, show: view(&s.show)? }
    };
    // a new window picks its sessions up itself once loaded; an existing one is told
    target.emit_to(target.label(), "session-added", moved).map_err(|e| e.to_string())?;
    target.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn get_show(state: State<AppState>, id: u64) -> CmdResult<serde_json::Value> {
    with_show(&state, id, |s| view(s))
}

#[tauri::command]
fn patch(state: State<AppState>, id: u64, ops: Vec<PatchOp>) -> CmdResult<serde_json::Value> {
    with_show(&state, id, |s| {
        s.apply(&ops);
        view(s)
    })
}

#[tauri::command]
fn undo(state: State<AppState>, id: u64) -> CmdResult<serde_json::Value> {
    with_show(&state, id, |s| {
        s.undo();
        view(s)
    })
}

#[tauri::command]
fn redo(state: State<AppState>, id: u64) -> CmdResult<serde_json::Value> {
    with_show(&state, id, |s| {
        s.redo();
        view(s)
    })
}

/// Set the output source (tap point) of a direct out or bus: 0 INP, 1 PRE, 2 PST, 3 PSP.
#[tauri::command]
fn set_tap(state: State<AppState>, id: u64, cluster: i64, index: i64, tap: i64) -> CmdResult<serde_json::Value> {
    with_show(&state, id, |s| {
        s.set_tap(cluster, index, tap)?;
        view(s)
    })
}

/// Set the number of driver channels (inputs and outputs) of a DAW device.
#[tauri::command]
fn set_driver_channels(state: State<AppState>, id: u64, idx: i64, count: i64) -> CmdResult<serde_json::Value> {
    with_show(&state, id, |s| {
        s.set_driver_channels(idx, count)?;
        view(s)
    })
}

/// Set gain (dB) and/or 48V of an input of an I/O device; `None` leaves a value unchanged.
#[tauri::command]
fn set_preamp(
    state: State<AppState>,
    id: u64,
    idx: i64,
    channel: i64,
    gain: Option<f64>,
    phantom: Option<bool>,
) -> CmdResult<serde_json::Value> {
    with_show(&state, id, |s| {
        s.set_preamp(idx, channel, gain, phantom)?;
        view(s)
    })
}

/// Write an exported text file (e.g. the patch list of a device), chosen in a save dialog.
#[tauri::command]
fn export_text(path: String, contents: String) -> CmdResult<()> {
    fs::write(path, contents).map_err(|e| e.to_string())
}

/// Save into the opened file (with backup), or into `path` as a patched copy.
#[tauri::command]
fn save(state: State<AppState>, id: u64, path: Option<String>, force: bool) -> CmdResult<SaveResult> {
    with_show(&state, id, |s| {
        let target = path.map(PathBuf::from).unwrap_or_else(|| s.path.clone());
        let backup = s.save(&target, force)?;
        Ok(SaveResult { show: view(s)?, backup: backup.map(|b| b.display().to_string()) })
    })
}

/// macOS menu. The default one closes the whole window on ⌘W; here ⌘W closes a tab.
/// Custom items are forwarded to the focused window as a "menu" event (the UI handles the
/// same shortcuts itself on the other platforms, which have no menu bar).
#[cfg(target_os = "macos")]
fn build_menu(app: &AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{AboutMetadata, MenuBuilder, MenuItem, PredefinedMenuItem, SubmenuBuilder};
    let item = |id: &str, text: &str, accel: &str| MenuItem::with_id(app, id, text, true, Some(accel));
    let info = app.package_info();
    let about = AboutMetadata {
        name: Some(info.name.clone()),
        version: Some(info.version.to_string()),
        copyright: app.config().bundle.copyright.clone(),
        ..Default::default()
    };
    let app_menu = SubmenuBuilder::new(app, info.name.clone())
        .item(&PredefinedMenuItem::about(app, None, Some(about))?)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .separator()
        .quit()
        .build()?;
    let file = SubmenuBuilder::new(app, "File")
        .item(&item("new-tab", "New Tab", "CmdOrCtrl+T")?)
        .item(&item("new-window", "New Window", "CmdOrCtrl+N")?)
        .item(&item("open", "Open…", "CmdOrCtrl+O")?)
        .separator()
        .item(&item("close-tab", "Close Tab", "CmdOrCtrl+W")?)
        .item(&item("close-window", "Close Window", "CmdOrCtrl+Shift+W")?)
        .separator()
        .item(&item("save", "Save", "CmdOrCtrl+S")?)
        .item(&item("save-as", "Save Patched Copy…", "CmdOrCtrl+Shift+S")?)
        .build()?;
    let edit = SubmenuBuilder::new(app, "Edit").undo().redo().separator().cut().copy().paste().select_all().build()?;
    let window = SubmenuBuilder::new(app, "Window")
        .minimize()
        .maximize()
        .separator()
        .item(&item("next-tab", "Show Next Tab", "CmdOrCtrl+Shift+]")?)
        .item(&item("prev-tab", "Show Previous Tab", "CmdOrCtrl+Shift+[")?)
        .build()?;
    MenuBuilder::new(app).items(&[&app_menu, &file, &edit, &window]).build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .setup(|_app| {
            #[cfg(target_os = "macos")]
            {
                let menu = build_menu(_app.handle())?;
                _app.set_menu(menu)?;
                _app.on_menu_event(|app, event| {
                    let id = event.id().as_ref().to_string();
                    let focused = app.webview_windows().into_values().find(|w| w.is_focused().unwrap_or(false));
                    match focused {
                        Some(w) => {
                            let _ = w.emit_to(w.label(), "menu", id);
                        }
                        // no window left (macOS keeps the app running): start a new one
                        None if id == "new-window" || id == "new-tab" || id == "open" => {
                            let _ = create_window(app, &app.state::<AppState>());
                        }
                        None => {}
                    }
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // a closed window takes its tabs with it
            if let WindowEvent::Destroyed = event {
                let state = window.state::<AppState>();
                if let Ok(mut sessions) = state.sessions.lock() {
                    sessions.retain(|_, s| s.window != window.label());
                };
            }
        })
        .invoke_handler(tauri::generate_handler![
            open_show,
            window_sessions,
            close_session,
            new_window,
            move_session,
            get_show,
            patch,
            undo,
            redo,
            set_tap,
            set_driver_channels,
            set_preamp,
            save,
            export_text
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
