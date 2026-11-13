//! Live source: the Claude dir on this machine, watched with `notify`.

use super::pid::{PidProbe, SystemPidProbe};
use super::{wall_clock_ms, ChangeOp, DirEntry, FileChange, FileView, Source};
use crate::contract::{EpochMs, Liveness, SourceMode};
use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

/// `$CLAUDE_CONFIG_DIR` if set, else `~/.claude` (`%USERPROFILE%\.claude` on Windows).
pub fn resolve_claude_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d));
    }
    let home = if cfg!(windows) {
        std::env::var_os("USERPROFILE").filter(|h| !h.is_empty()).map(PathBuf::from).or_else(|| {
            let drive = std::env::var_os("HOMEDRIVE")?;
            let path = std::env::var_os("HOMEPATH")?;
            let mut p = PathBuf::from(drive);
            p.push(path);
            Some(p)
        })
    } else {
        std::env::var_os("HOME").filter(|h| !h.is_empty()).map(PathBuf::from)
    };
    home.map(|h| h.join(".claude"))
}

pub struct LiveConfig {
    /// `None` resolves with [`resolve_claude_dir`].
    pub claude_dir: Option<PathBuf>,
    /// Stale sessions older than this are not shown. Default 24 hours.
    pub recent_window_ms: Option<i64>,
    /// See `EngineConfig::quiet_after_ms`. Default 30 minutes.
    pub quiet_after_ms: Option<i64>,
    /// Use the file-system watcher. Off for tests and one-shot CLI runs.
    pub watch: bool,
    pub probe: Box<dyn PidProbe>,
}

impl Default for LiveConfig {
    fn default() -> Self {
        LiveConfig {
            claude_dir: None,
            recent_window_ms: Some(24 * 3600 * 1000),
            quiet_after_ms: Some(crate::engine::QUIET_AFTER_MS),
            watch: true,
            probe: Box::new(SystemPidProbe),
        }
    }
}

/// `std::fs` view of a directory, paths relative to it.
pub struct DirView {
    root: PathBuf,
}

impl DirView {
    pub fn new(root: impl Into<PathBuf>) -> DirView {
        DirView { root: root.into() }
    }
    fn full(&self, rel: &str) -> PathBuf {
        let mut p = self.root.clone();
        for part in rel.split('/').filter(|p| !p.is_empty() && *p != "..") {
            p.push(part);
        }
        p
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
}

impl FileView for DirView {
    fn list_dir(&self, dir: &str) -> Vec<DirEntry> {
        let Ok(rd) = fs::read_dir(self.full(dir)) else { return Vec::new() };
        rd.filter_map(Result::ok)
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                let is_dir = e.file_type().ok()?.is_dir();
                Some(DirEntry { name, is_dir })
            })
            .collect()
    }
    fn size(&self, path: &str) -> Option<u64> {
        let m = fs::metadata(self.full(path)).ok()?;
        m.is_file().then_some(m.len())
    }
    fn mtime(&self, path: &str) -> Option<EpochMs> {
        let m = fs::metadata(self.full(path)).ok()?;
        let d = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?;
        Some(d.as_millis() as EpochMs)
    }
    fn read_range(&self, path: &str, from: u64, to: u64) -> io::Result<Vec<u8>> {
        let mut f = fs::File::open(self.full(path))?;
        f.seek(SeekFrom::Start(from))?;
        let mut buf = Vec::with_capacity(to.saturating_sub(from) as usize);
        f.take(to.saturating_sub(from)).read_to_end(&mut buf)?;
        Ok(buf)
    }
    fn read_all(&self, path: &str) -> io::Result<Vec<u8>> {
        fs::read(self.full(path))
    }
}

pub struct LiveSource {
    view: DirView,
    /// Canonical form of the root, for stripping watcher paths (macOS reports /private/var/...).
    canonical_root: Option<PathBuf>,
    probe: Box<dyn PidProbe>,
    watcher: Option<notify::RecommendedWatcher>,
    rx: Option<Receiver<notify::Result<notify::Event>>>,
    watch: bool,
    error: Option<String>,
    pub recent_window_ms: Option<i64>,
}

impl LiveSource {
    pub fn new(cfg: LiveConfig) -> LiveSource {
        let root = cfg.claude_dir.or_else(resolve_claude_dir).unwrap_or_else(|| PathBuf::from(".claude"));
        let mut s = LiveSource {
            canonical_root: fs::canonicalize(&root).ok(),
            view: DirView::new(root),
            probe: cfg.probe,
            watcher: None,
            rx: None,
            watch: cfg.watch,
            error: None,
            recent_window_ms: cfg.recent_window_ms,
        };
        s.ensure_watcher();
        s
    }

    pub fn claude_dir(&self) -> &Path {
        self.view.root()
    }

    /// A problem with the source, e.g. the Claude dir is missing.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// (Re)creates the watcher if it is missing, e.g. when the Claude dir appeared later.
    pub fn ensure_watcher(&mut self) {
        let root = self.view.root().to_path_buf();
        if !root.is_dir() {
            self.error = Some(format!("Claude dir not found: {}", root.display()));
            return;
        }
        self.error = None;
        if !self.watch || self.watcher.is_some() {
            return;
        }
        use notify::{RecursiveMode, Watcher};
        let (tx, rx) = channel();
        match notify::recommended_watcher(tx) {
            Ok(mut w) => {
                let mut ok = false;
                for sub in ["sessions", "projects"] {
                    let p = root.join(sub);
                    if p.is_dir() && w.watch(&p, RecursiveMode::Recursive).is_ok() {
                        ok = true;
                    }
                }
                if ok {
                    self.canonical_root = fs::canonicalize(&root).ok();
                    self.watcher = Some(w);
                    self.rx = Some(rx);
                }
            }
            Err(e) => self.error = Some(format!("file watcher unavailable: {e}")),
        }
    }

    fn relative(&self, p: &Path) -> Option<String> {
        let rel = p
            .strip_prefix(self.view.root())
            .ok()
            .or_else(|| self.canonical_root.as_ref().and_then(|c| p.strip_prefix(c).ok()))?;
        let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
        Some(parts.join("/"))
    }
}

impl FileView for LiveSource {
    fn list_dir(&self, dir: &str) -> Vec<DirEntry> {
        self.view.list_dir(dir)
    }
    fn size(&self, path: &str) -> Option<u64> {
        self.view.size(path)
    }
    fn mtime(&self, path: &str) -> Option<EpochMs> {
        self.view.mtime(path)
    }
    fn read_range(&self, path: &str, from: u64, to: u64) -> io::Result<Vec<u8>> {
        self.view.read_range(path, from, to)
    }
    fn read_all(&self, path: &str) -> io::Result<Vec<u8>> {
        self.view.read_all(path)
    }
}

impl Source for LiveSource {
    fn mode(&self) -> SourceMode {
        SourceMode::Live
    }

    fn poll(&mut self) -> Vec<FileChange> {
        let Some(rx) = &self.rx else { return Vec::new() };
        let mut paths: BTreeSet<String> = BTreeSet::new();
        let mut order: Vec<String> = Vec::new();
        while let Ok(res) = rx.try_recv() {
            let Ok(ev) = res else { continue };
            if matches!(ev.kind, notify::EventKind::Access(_)) {
                continue;
            }
            for p in &ev.paths {
                if let Some(rel) = self.relative(p) {
                    if (rel.starts_with("sessions/") || rel.starts_with("projects/")) && paths.insert(rel.clone()) {
                        order.push(rel);
                    }
                }
            }
        }
        let t = wall_clock_ms();
        order
            .into_iter()
            .map(|path| {
                let op = if self.view.size(&path).is_some() { ChangeOp::Modify } else { ChangeOp::Delete };
                FileChange { path, op, t }
            })
            .collect()
    }

    fn now(&self) -> EpochMs {
        wall_clock_ms()
    }

    fn liveness(&self, pid: u32) -> Liveness {
        if self.probe.is_alive(pid) {
            Liveness::Alive
        } else {
            Liveness::Stale
        }
    }
}
