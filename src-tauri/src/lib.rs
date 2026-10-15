mod layout;
mod show;

use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;
use show::{PatchOp, Show, ShowView};
use tauri::State;

#[derive(Default)]
struct AppState(Mutex<Option<Show>>);

type CmdResult<T> = Result<T, String>;

fn with_show<T>(state: &State<AppState>, f: impl FnOnce(&mut Show) -> CmdResult<T>) -> CmdResult<T> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    let show = guard.as_mut().ok_or("No showfile opened")?;
    f(show)
}

/// Serialized show state; returned by every command so the UI simply re-renders.
fn view(show: &Show) -> CmdResult<serde_json::Value> {
    serde_json::to_value::<ShowView>(show.view()).map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct SaveResult {
    show: serde_json::Value,
    backup: Option<String>,
}

#[tauri::command]
fn open_show(state: State<AppState>, path: String) -> CmdResult<serde_json::Value> {
    let show = Show::open(&PathBuf::from(path))?;
    let v = view(&show)?;
    *state.0.lock().map_err(|e| e.to_string())? = Some(show);
    Ok(v)
}

#[tauri::command]
fn get_show(state: State<AppState>) -> CmdResult<serde_json::Value> {
    with_show(&state, |s| view(s))
}

#[tauri::command]
fn patch(state: State<AppState>, ops: Vec<PatchOp>) -> CmdResult<serde_json::Value> {
    with_show(&state, |s| {
        s.apply(&ops);
        view(s)
    })
}

#[tauri::command]
fn undo(state: State<AppState>) -> CmdResult<serde_json::Value> {
    with_show(&state, |s| {
        s.undo();
        view(s)
    })
}

#[tauri::command]
fn redo(state: State<AppState>) -> CmdResult<serde_json::Value> {
    with_show(&state, |s| {
        s.redo();
        view(s)
    })
}

/// Set the output source (tap point) of a direct out or bus: 0 INP, 1 PRE, 2 PST, 3 PSP.
#[tauri::command]
fn set_tap(state: State<AppState>, cluster: i64, index: i64, tap: i64) -> CmdResult<serde_json::Value> {
    with_show(&state, |s| {
        s.set_tap(cluster, index, tap)?;
        view(s)
    })
}

/// Save into the opened file (with backup), or into `path` as a patched copy.
#[tauri::command]
fn save(state: State<AppState>, path: Option<String>, force: bool) -> CmdResult<SaveResult> {
    with_show(&state, |s| {
        let target = path.map(PathBuf::from).unwrap_or_else(|| s.path.clone());
        let backup = s.save(&target, force)?;
        Ok(SaveResult { show: view(s)?, backup: backup.map(|b| b.display().to_string()) })
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![open_show, get_show, patch, undo, redo, set_tap, save])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
