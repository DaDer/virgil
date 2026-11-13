//! Sources of file observations. Live watches the Claude dir; Replay plays a recording. The engine
//! only sees the [`FileView`] interface, so adapter and model code are identical for both.

use crate::contract::{EpochMs, Liveness, SourceMode};
use std::io;

pub mod live;
pub mod pid;
pub mod replay;

pub use live::{resolve_claude_dir, LiveConfig, LiveSource};
pub use pid::{PidProbe, SystemPidProbe};
pub use replay::{list_recordings, read_recording_info, ReplaySource};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeOp {
    Create,
    Modify,
    Delete,
}

/// A path relative to the Claude dir (forward slashes) that changed at `t`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub path: String,
    pub op: ChangeOp,
    pub t: EpochMs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
}

/// Read-only view of the Claude dir "as it is now" for a source. Paths are relative, forward slashes.
pub trait FileView {
    fn list_dir(&self, dir: &str) -> Vec<DirEntry>;
    fn size(&self, path: &str) -> Option<u64>;
    fn mtime(&self, path: &str) -> Option<EpochMs>;
    /// Bytes `[from, to)`; shorter when the file holds fewer bytes.
    fn read_range(&self, path: &str, from: u64, to: u64) -> io::Result<Vec<u8>>;
    fn read_all(&self, path: &str) -> io::Result<Vec<u8>>;
}

pub trait Source: FileView + Send {
    fn mode(&self) -> SourceMode;
    /// Changes since the last call, in order. Replay returns at most one fs-event per call and
    /// advances its file view to that event, so the engine reads exactly what the recording held.
    fn poll(&mut self) -> Vec<FileChange>;
    fn now(&self) -> EpochMs;
    fn liveness(&self, pid: u32) -> Liveness;
}

pub fn wall_clock_ms() -> EpochMs {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as EpochMs)
        .unwrap_or(0)
}
