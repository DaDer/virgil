//! Virgil core: turns the files Claude Code writes under its config dir into a stream of typed
//! events and an up-to-date snapshot (docs/data-contract.md). No Tauri dependency.
//!
//! - [`contract`]: the types that cross the UI boundary, and the reducer.
//! - [`adapter`]: everything that knows the source file formats.
//! - [`engine`]: applies file observations, emits events, keeps the snapshot.
//! - [`source`]: Live (watch the Claude dir) and Replay (play a recording) behind one trait.
//! - [`hub`]: the single object a shell drives (commands of contract section 5.1).

pub mod adapter;
pub mod contract;
pub mod engine;
pub mod attention;
pub mod hub;
pub mod source;

pub use contract::*;
pub use attention::{AttentionItem, AttentionKind};
pub use engine::{BackgroundTask, Engine, EngineConfig, EngineStats, SessionExtras};
pub use hub::{default_recording_dirs, Hub};
pub use source::{list_recordings, resolve_claude_dir, ChangeOp, FileChange, FileView, LiveConfig, LiveSource, ReplaySource, Source};

/// Name of the core crate, used as a smoke test for the workspace wiring.
pub fn core_name() -> &'static str {
    "virgil-core"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_name() {
        assert_eq!(core_name(), "virgil-core");
    }
}
