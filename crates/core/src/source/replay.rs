//! Replay source: plays a format-1 recording written by `tools/recorder.py` (contract section 6).
//!
//! The source applies one fs-event per [`Source::poll`] call and serves the file view as it was
//! right after that event, so the engine reads exactly the bytes the recording held at that point.

use super::{ChangeOp, DirEntry, FileChange, FileView, Source};
use crate::contract::{CommandError, CommandErrorCode, EpochMs, Liveness, RecordingInfo, SourceMode};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Manifest {
    pub format: u32,
    pub claude_dir: Option<String>,
    pub session_id: Option<String>,
    pub project_slug: Option<String>,
    pub started_at: EpochMs,
    pub poll_ms: i64,
    pub stopped_at: Option<EpochMs>,
    pub event_count: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct FsEvent {
    pub seq: u64,
    pub t: EpochMs,
    pub op: ChangeOp,
    pub path: String,
    pub size: u64,
    pub mtime_ms: Option<EpochMs>,
    pub snapshot: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct PathState {
    exists: bool,
    size: u64,
    mtime: Option<EpochMs>,
    /// Index into `events` of the last event applied for this path.
    last_event: usize,
    /// The newest snapshot file seen for this path.
    snapshot: Option<String>,
}

fn invalid(msg: impl Into<String>) -> CommandError {
    CommandError::new(CommandErrorCode::InvalidRecording, msg)
}

pub fn read_manifest(dir: &Path) -> Result<Manifest, CommandError> {
    let p = dir.join("manifest.json");
    let bytes = fs::read(&p).map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            CommandError::new(CommandErrorCode::NotFound, format!("no manifest.json in {}", dir.display()))
        } else {
            CommandError::new(CommandErrorCode::Io, e.to_string())
        }
    })?;
    let v: Value = serde_json::from_slice(&bytes).map_err(|e| invalid(format!("manifest.json: {e}")))?;
    let format = v.get("format").and_then(Value::as_u64).unwrap_or(0) as u32;
    if format != 1 {
        return Err(invalid(format!("unsupported recording format {format}")));
    }
    let started_at = v.get("startedAt").and_then(Value::as_i64).ok_or_else(|| invalid("manifest has no startedAt"))?;
    Ok(Manifest {
        format,
        claude_dir: v.get("claudeDir").and_then(Value::as_str).map(str::to_string),
        session_id: v.get("sessionId").and_then(Value::as_str).map(str::to_string),
        project_slug: v.get("projectSlug").and_then(Value::as_str).map(str::to_string),
        started_at,
        poll_ms: v.get("pollMs").and_then(Value::as_i64).unwrap_or(500).max(1),
        stopped_at: v.get("stoppedAt").and_then(Value::as_i64),
        event_count: v.get("eventCount").and_then(Value::as_u64),
    })
}

fn parse_fs_event(line: &[u8]) -> Option<FsEvent> {
    let v: Value = serde_json::from_slice(line).ok()?;
    let op = match v.get("op")?.as_str()? {
        "create" => ChangeOp::Create,
        "modify" => ChangeOp::Modify,
        "delete" => ChangeOp::Delete,
        _ => return None,
    };
    Some(FsEvent {
        seq: v.get("seq").and_then(Value::as_u64).unwrap_or(0),
        t: v.get("t")?.as_i64()?,
        op,
        path: v.get("path")?.as_str()?.replace('\\', "/"),
        size: v.get("size").and_then(Value::as_u64).unwrap_or(0),
        mtime_ms: v.get("mtimeMs").and_then(Value::as_i64),
        snapshot: v.get("snapshot").and_then(Value::as_str).map(str::to_string),
    })
}

/// Summary of a recording directory, or an error when it is not a format-1 recording.
pub fn read_recording_info(dir: &Path) -> Result<RecordingInfo, CommandError> {
    let m = read_manifest(dir)?;
    let (events, _) = read_events(dir, 0);
    let last_t = events.last().map(|e| e.t);
    Ok(RecordingInfo {
        path: fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf()).to_string_lossy().into_owned(),
        name: dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        format: m.format,
        session_id: m.session_id.clone(),
        project_slug: m.project_slug.clone(),
        started_at: m.started_at,
        ended_at: m.stopped_at.or(last_t).unwrap_or(m.started_at).max(m.started_at),
        event_count: m.event_count.unwrap_or(events.len() as u64),
        complete: m.stopped_at.is_some() && dir.join("final").is_dir(),
    })
}

/// Recordings in the given directories (each a parent of recording directories), newest first.
pub fn list_recordings(dirs: &[PathBuf]) -> Vec<RecordingInfo> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for d in dirs {
        let canon = fs::canonicalize(d).unwrap_or_else(|_| d.clone());
        if !seen.insert(canon) {
            continue;
        }
        let Ok(rd) = fs::read_dir(d) else { continue };
        for e in rd.filter_map(Result::ok) {
            let p = e.path();
            if p.join("manifest.json").is_file() {
                if let Ok(info) = read_recording_info(&p) {
                    out.push(info);
                }
            }
        }
    }
    out.sort_by(|a, b| b.started_at.cmp(&a.started_at).then(a.path.cmp(&b.path)));
    out.dedup_by(|a, b| a.path == b.path);
    out
}

/// Reads complete fs-event lines from byte `from`; returns them and the new offset.
fn read_events(dir: &Path, from: u64) -> (Vec<FsEvent>, u64) {
    let Ok(mut f) = fs::File::open(dir.join("fs-events.jsonl")) else { return (Vec::new(), from) };
    if f.seek(SeekFrom::Start(from)).is_err() {
        return (Vec::new(), from);
    }
    let mut buf = Vec::new();
    if f.read_to_end(&mut buf).is_err() {
        return (Vec::new(), from);
    }
    let Some(last_nl) = buf.iter().rposition(|&c| c == b'\n') else { return (Vec::new(), from) };
    let events = buf[..last_nl].split(|&c| c == b'\n').filter_map(parse_fs_event).collect();
    (events, from + last_nl as u64 + 1)
}

pub struct ReplaySource {
    dir: PathBuf,
    manifest: Manifest,
    events: Vec<FsEvent>,
    events_offset: u64,
    /// Number of events applied.
    cursor: usize,
    position: EpochMs,
    paths: BTreeMap<String, PathState>,
    /// Last event index per path over the whole recording.
    last_index: HashMap<String, usize>,
    has_final: bool,
}

impl ReplaySource {
    pub fn open(dir: impl AsRef<Path>) -> Result<ReplaySource, CommandError> {
        let dir = dir.as_ref().to_path_buf();
        let manifest = read_manifest(&dir)?;
        let (mut events, events_offset) = read_events(&dir, 0);
        events.sort_by_key(|e| e.seq);
        let mut s = ReplaySource {
            has_final: dir.join("final").is_dir(),
            position: manifest.started_at,
            dir,
            manifest,
            events: Vec::new(),
            events_offset,
            cursor: 0,
            paths: BTreeMap::new(),
            last_index: HashMap::new(),
        };
        s.push_events(events);
        Ok(s)
    }

    fn push_events(&mut self, events: Vec<FsEvent>) {
        for e in events {
            self.last_index.insert(e.path.clone(), self.events.len());
            self.events.push(e);
        }
    }

    /// Picks up fs-events appended since opening (a recording that is still running). Returns
    /// whether the timeline grew.
    pub fn refresh(&mut self) -> bool {
        let (events, off) = read_events(&self.dir, self.events_offset);
        self.events_offset = off;
        if events.is_empty() {
            return false;
        }
        self.push_events(events);
        if let Ok(m) = read_manifest(&self.dir) {
            self.manifest.stopped_at = m.stopped_at;
            self.manifest.event_count = m.event_count;
        }
        self.has_final = self.dir.join("final").is_dir();
        true
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn info(&self) -> RecordingInfo {
        RecordingInfo {
            path: fs::canonicalize(&self.dir).unwrap_or_else(|_| self.dir.clone()).to_string_lossy().into_owned(),
            name: self.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            format: self.manifest.format,
            session_id: self.manifest.session_id.clone(),
            project_slug: self.manifest.project_slug.clone(),
            started_at: self.start(),
            ended_at: self.end(),
            event_count: self.manifest.event_count.unwrap_or(self.events.len() as u64),
            complete: self.manifest.stopped_at.is_some() && self.has_final,
        }
    }
    pub fn start(&self) -> EpochMs {
        self.manifest.started_at
    }
    pub fn end(&self) -> EpochMs {
        self.manifest.stopped_at.or(self.events.last().map(|e| e.t)).unwrap_or(self.start()).max(self.start())
    }
    pub fn event_count(&self) -> usize {
        self.events.len()
    }
    /// Events with `t` below this belong to the initial scan.
    pub fn initial_scan_end(&self) -> EpochMs {
        self.start() + self.manifest.poll_ms
    }
    /// Number of events in the initial scan.
    pub fn initial_scan_len(&self) -> usize {
        self.events.iter().take_while(|e| e.t < self.initial_scan_end()).count()
    }
    pub fn position(&self) -> EpochMs {
        self.position
    }
    /// Moves the clock; events up to it become available through `poll`. Never goes back: use
    /// [`ReplaySource::rewind`] first.
    pub fn set_position(&mut self, t: EpochMs) {
        self.position = t.clamp(self.start(), self.end());
    }
    /// Forgets all applied events (the view becomes empty) and puts the clock at the start.
    pub fn rewind(&mut self) {
        self.cursor = 0;
        self.paths.clear();
        self.position = self.start();
    }
    /// Whether every event up to the position has been applied.
    pub fn caught_up(&self) -> bool {
        self.events.get(self.cursor).is_none_or(|e| e.t > self.effective_position())
    }
    /// The time of every fs-event of the recording, ascending.
    pub fn event_times(&self) -> impl Iterator<Item = EpochMs> + '_ {
        self.events.iter().map(|e| e.t)
    }
    /// The quiet stretch the clock is in: `(time of the last applied fs-event, else start; time of
    /// the next one, else end)`. Used to skip quiet gaps.
    pub fn gap_around(&self) -> (EpochMs, EpochMs) {
        let prev = self.cursor.checked_sub(1).and_then(|i| self.events.get(i)).map_or(self.start(), |e| e.t);
        let next = self.events.get(self.cursor).map_or(self.end(), |e| e.t);
        (prev.max(self.start()), next.min(self.end()).max(prev))
    }

    fn effective_position(&self) -> EpochMs {
        self.position.max(self.initial_scan_end() - 1)
    }

    fn final_path(&self, rel: &str) -> PathBuf {
        let mut p = self.dir.join("final");
        p.extend(rel.split('/'));
        p
    }

    fn live_path(&self, rel: &str) -> Option<PathBuf> {
        let mut p = PathBuf::from(self.manifest.claude_dir.as_ref()?);
        p.extend(rel.split('/'));
        Some(p)
    }

    /// The on-disk file that holds `rel`'s bytes: `final/<rel>`, else the recorded machine's live
    /// file when it exists here (a recording without `final/`).
    fn content_file(&self, rel: &str) -> Option<PathBuf> {
        let f = self.final_path(rel);
        if f.is_file() {
            return Some(f);
        }
        self.live_path(rel).filter(|p| p.is_file())
    }

    fn json_file(&self, rel: &str, st: &PathState) -> Option<PathBuf> {
        let ev = &self.events[st.last_event];
        if let Some(snap) = &ev.snapshot {
            let p = self.dir.join(snap);
            if p.is_file() {
                return Some(p);
            }
        }
        let is_last = self.last_index.get(rel) == Some(&st.last_event);
        if is_last {
            if let Some(p) = self.content_file(rel) {
                return Some(p);
            }
        }
        st.snapshot.as_ref().map(|s| self.dir.join(s)).filter(|p| p.is_file())
    }
}

impl FileView for ReplaySource {
    fn list_dir(&self, dir: &str) -> Vec<DirEntry> {
        let prefix = if dir.is_empty() { String::new() } else { format!("{}/", dir.trim_end_matches('/')) };
        let mut out: BTreeMap<String, bool> = BTreeMap::new();
        for (p, st) in self.paths.range(prefix.clone()..) {
            if !p.starts_with(&prefix) {
                break;
            }
            if !st.exists {
                continue;
            }
            let rest = &p[prefix.len()..];
            match rest.find('/') {
                Some(i) => {
                    out.insert(rest[..i].to_string(), true);
                }
                None => {
                    out.entry(rest.to_string()).or_insert(false);
                }
            }
        }
        out.into_iter().map(|(name, is_dir)| DirEntry { name, is_dir }).collect()
    }

    fn size(&self, path: &str) -> Option<u64> {
        let st = self.paths.get(path).filter(|s| s.exists)?;
        Some(st.size)
    }

    fn mtime(&self, path: &str) -> Option<EpochMs> {
        self.paths.get(path).filter(|s| s.exists).and_then(|s| s.mtime)
    }

    fn read_range(&self, path: &str, from: u64, to: u64) -> io::Result<Vec<u8>> {
        let st = self.paths.get(path).filter(|s| s.exists).ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        if !path.ends_with(".jsonl") {
            let all = self.read_all(path)?;
            let a = (from as usize).min(all.len());
            let b = (to as usize).min(all.len()).max(a);
            return Ok(all[a..b].to_vec());
        }
        let to = to.min(st.size);
        if from >= to {
            return Ok(Vec::new());
        }
        let file = self.content_file(path).ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        let mut f = fs::File::open(file)?;
        f.seek(SeekFrom::Start(from))?;
        let mut buf = Vec::with_capacity((to - from) as usize);
        f.take(to - from).read_to_end(&mut buf)?;
        Ok(buf)
    }

    fn read_all(&self, path: &str) -> io::Result<Vec<u8>> {
        let st = self.paths.get(path).filter(|s| s.exists).ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        if path.ends_with(".jsonl") {
            return self.read_range(path, 0, st.size);
        }
        let file = if path.ends_with(".json") { self.json_file(path, st) } else { self.content_file(path) };
        let file = file.ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        fs::read(file)
    }
}

impl Source for ReplaySource {
    fn mode(&self) -> SourceMode {
        SourceMode::Replay
    }

    fn poll(&mut self) -> Vec<FileChange> {
        let limit = self.effective_position();
        let Some(ev) = self.events.get(self.cursor).filter(|e| e.t <= limit) else { return Vec::new() };
        let idx = self.cursor;
        self.cursor += 1;
        let st = self.paths.entry(ev.path.clone()).or_default();
        st.exists = ev.op != ChangeOp::Delete;
        st.size = ev.size;
        st.mtime = ev.mtime_ms;
        st.last_event = idx;
        if ev.snapshot.is_some() {
            st.snapshot = ev.snapshot.clone();
        }
        vec![FileChange { path: ev.path.clone(), op: ev.op, t: ev.t }]
    }

    fn now(&self) -> EpochMs {
        // While events are being applied, "now" is the time of the newest applied event.
        match self.cursor.checked_sub(1).and_then(|i| self.events.get(i)) {
            Some(e) if e.t > self.position => e.t,
            _ => self.position,
        }
    }

    fn liveness(&self, _pid: u32) -> Liveness {
        Liveness::Unknown
    }
}
