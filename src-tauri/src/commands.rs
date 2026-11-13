//! The Tauri commands of data-contract 5.1. Each runs on a blocking
//! worker thread so neither the webview nor the async runtime waits on the core's lock.

use crate::pump::Shared;
use virgil_core::{
    default_recording_dirs, list_recordings as core_list, source::read_recording_info, CommandError, CommandErrorCode, EpochMs,
    Hub, RecordingInfo, Snapshot, SourceStatus, ToolHistory,
};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

type Res<T> = Result<T, CommandError>;

fn internal(e: impl std::fmt::Display) -> CommandError {
    CommandError::new(CommandErrorCode::Internal, e.to_string())
}

async fn blocking<R: Send + 'static>(f: impl FnOnce() -> Res<R> + Send + 'static) -> Res<R> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(internal)?
}

async fn on_hub<R: Send + 'static>(state: State<'_, Arc<Shared>>, f: impl FnOnce(&mut Hub) -> Res<R> + Send + 'static) -> Res<R> {
    let shared = state.inner().clone();
    blocking(move || shared.with_hub(f)).await
}

#[tauri::command]
pub async fn get_snapshot(state: State<'_, Arc<Shared>>) -> Res<Snapshot> {
    on_hub(state, |h| Ok(h.snapshot().clone())).await
}

#[tauri::command]
pub async fn get_tool_history(state: State<'_, Arc<Shared>>) -> Res<ToolHistory> {
    on_hub(state, |h| Ok(h.tool_history())).await
}

#[tauri::command]
pub async fn set_skip_gaps(state: State<'_, Arc<Shared>>, on: bool) -> Res<SourceStatus> {
    on_hub(state, move |h| Ok(h.set_skip_gaps(on))).await
}

/// `dir: null` goes back to the default Claude dir.
#[tauri::command]
pub async fn set_claude_dir(state: State<'_, Arc<Shared>>, dir: Option<String>) -> Res<SourceStatus> {
    on_hub(state, move |h| h.set_claude_dir(dir.map(PathBuf::from))).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PickedPath {
    /// The folder the person chose, canonicalised; `null` when they cancelled.
    path: Option<String>,
}

/// Shows the native folder dialog for choosing the Claude folder. Changes nothing by itself: the UI
/// passes the result to `set_claude_dir`.
#[tauri::command]
pub async fn pick_claude_dir(app: AppHandle) -> Res<PickedPath> {
    blocking(move || {
        let picked = app.dialog().file().set_title("Choose the Claude folder (usually ~/.claude)").blocking_pick_folder();
        let path = picked.and_then(|p| p.into_path().ok()).map(|p| std::fs::canonicalize(&p).unwrap_or(p));
        Ok(PickedPath { path: path.map(|p| p.to_string_lossy().into_owned()) })
    })
    .await
}

#[tauri::command]
pub async fn get_status(state: State<'_, Arc<Shared>>) -> Res<SourceStatus> {
    on_hub(state, |h| Ok(h.status())).await
}

#[tauri::command]
pub async fn open_recording(state: State<'_, Arc<Shared>>, path: String) -> Res<SourceStatus> {
    on_hub(state, move |h| h.open_recording(Path::new(&path))).await
}

#[tauri::command]
pub async fn switch_to_live(state: State<'_, Arc<Shared>>) -> Res<SourceStatus> {
    on_hub(state, |h| Ok(h.switch_to_live())).await
}

#[tauri::command]
pub async fn replay_play(state: State<'_, Arc<Shared>>) -> Res<SourceStatus> {
    on_hub(state, |h| h.replay_play()).await
}

#[tauri::command]
pub async fn replay_pause(state: State<'_, Arc<Shared>>) -> Res<SourceStatus> {
    on_hub(state, |h| h.replay_pause()).await
}

#[tauri::command]
pub async fn replay_seek(state: State<'_, Arc<Shared>>, t: EpochMs) -> Res<SourceStatus> {
    on_hub(state, move |h| h.replay_seek(t)).await
}

#[tauri::command]
pub async fn replay_set_speed(state: State<'_, Arc<Shared>>, speed: f64) -> Res<SourceStatus> {
    on_hub(state, move |h| h.replay_set_speed(speed)).await
}

#[tauri::command]
pub async fn list_recordings(app: AppHandle, dir: Option<String>) -> Res<Vec<RecordingInfo>> {
    let app_data = app.path().app_data_dir().ok();
    blocking(move || {
        Ok(match dir {
            Some(d) => recordings_in(Path::new(&d)),
            None => core_list(&recording_dirs(app_data.as_deref())),
        })
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PickedFolder {
    /// The folder the person chose, canonicalised; `null` when they cancelled.
    path: Option<String>,
    /// The recordings in it, or the folder itself when it is one.
    recordings: Vec<RecordingInfo>,
}

/// Shows the native folder dialog (from Rust, so the webview needs no dialog permission).
#[tauri::command]
pub async fn pick_recording_folder(app: AppHandle) -> Res<PickedFolder> {
    blocking(move || {
        let picked = app.dialog().file().set_title("Choose a recording, or a folder of recordings").blocking_pick_folder();
        let Some(path) = picked.and_then(|p| p.into_path().ok()) else {
            return Ok(PickedFolder { path: None, recordings: Vec::new() });
        };
        // Canonical, like `RecordingInfo.path`, so the UI can tell "this folder is a recording".
        let path = std::fs::canonicalize(&path).unwrap_or(path);
        Ok(PickedFolder { recordings: recordings_in(&path), path: Some(path.to_string_lossy().into_owned()) })
    })
    .await
}

/// The default places of contract 5.1; debug builds add the repository's `recordings/`, because
/// `tauri dev` runs the app with `src-tauri/` as its working directory.
pub fn recording_dirs(app_data: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = default_recording_dirs(app_data);
    if cfg!(debug_assertions) {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("recordings");
        let canon = std::fs::canonicalize(&repo).unwrap_or(repo);
        if !dirs.iter().any(|d| std::fs::canonicalize(d).ok().as_ref() == Some(&canon)) {
            dirs.push(canon);
        }
    }
    dirs
}

/// A folder of recordings, or a single recording.
pub fn recordings_in(dir: &Path) -> Vec<RecordingInfo> {
    if dir.join("manifest.json").is_file() {
        return read_recording_info(dir).map(|r| vec![r]).unwrap_or_default();
    }
    core_list(&[dir.to_path_buf()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recording_or_a_folder_of_them() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures");
        let one = recordings_in(&fixtures.join("recording-small"));
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].name, "recording-small");
        let found = recordings_in(&fixtures);
        assert!(found.iter().any(|r| r.name == "recording-small"));
        assert!(recordings_in(&fixtures.join("nope")).is_empty());
    }

    #[test]
    fn debug_builds_list_the_repo_recordings() {
        let dirs = recording_dirs(None);
        assert!(dirs.iter().any(|d| d.ends_with("recordings")));
        let mut uniq = dirs.clone();
        uniq.dedup();
        assert_eq!(uniq.len(), dirs.len());
    }
}

/// Diagnostics from the webview, printed to stderr so a terminal run shows what the UI receives.
#[tauri::command]
pub fn ui_report(line: String) {
    eprintln!("ui: {line}");
}

/// The attention list with the menu-bar preferences (see `tower.rs`).
#[tauri::command]
pub fn get_attention(tower: State<'_, crate::tower::Tower>) -> crate::tower::AttentionState {
    tower.state()
}

#[tauri::command]
pub fn set_attention_prefs(app: AppHandle, tower: State<'_, crate::tower::Tower>, prefs: crate::tower::Prefs) -> crate::tower::AttentionState {
    tower.set_prefs(&app, prefs);
    tower.state()
}

/// `sessionId: null` marks every session as looked at.
#[tauri::command]
pub fn mark_attention_seen(app: AppHandle, tower: State<'_, crate::tower::Tower>, session_id: Option<String>) -> crate::tower::AttentionState {
    tower.mark_seen(&app, session_id.as_deref());
    tower.state()
}
