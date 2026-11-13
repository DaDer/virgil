//! [`Hub`]: the one object a shell needs. It owns the current source (live or replay) and its
//! engine, keeps `seq` increasing across mode switches and seeks, runs the replay clock, and hands
//! out event batches. It does no threading: call [`Hub::tick`] every ~100 ms from one thread.

use crate::contract::*;
use crate::engine::{Engine, EngineConfig, EngineStats};
use crate::source::{LiveConfig, LiveSource, ReplaySource, Source};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const MIN_SPEED: f64 = 0.1;
pub const MAX_SPEED: f64 = 600.0;
const LIVENESS_EVERY: Duration = Duration::from_secs(5);
const RESCAN_EVERY: Duration = Duration::from_secs(10);
const REFRESH_RECORDING_EVERY: Duration = Duration::from_secs(1);
/// Time spent reading files per live tick, so the initial scan of a large history does not block.
const LIVE_TICK_BUDGET: Duration = Duration::from_millis(50);
/// Skip quiet gaps (contract 6.2): a stretch longer than this with no fs-event...
pub const QUIET_GAP_MS: EpochMs = 30_000;
/// ...plays in this much wall time.
pub const QUIET_GAP_PLAY_MS: f64 = 1000.0;

enum Mode {
    Live { src: Box<LiveSource>, last_liveness: Instant, last_rescan: Instant },
    Replay {
        src: Box<ReplaySource>,
        playing: bool,
        speed: f64,
        skip_gaps: bool,
        last_tick: Instant,
        last_refresh: Instant,
        carry_ms: f64,
    },
}

pub struct Hub {
    mode: Mode,
    engine: Engine,
    /// Built events not yet taken by the shell.
    pending: Vec<Event>,
    live_factory: Box<dyn Fn() -> LiveConfig + Send>,
    /// Set by `set_claude_dir`; replaces the factory's `claude_dir`.
    claude_dir: Option<PathBuf>,
    /// Kept across recordings, like the UI's toggle (default on, ux-spec 2.7).
    skip_gaps: bool,
}

impl Hub {
    /// Starts in live mode. `live_factory` builds the live config again on every `switch_to_live`.
    pub fn new_live(live_factory: impl Fn() -> LiveConfig + Send + 'static) -> Hub {
        let cfg = live_factory();
        let (mode, engine) = Self::live_mode(cfg, 1);
        Hub { mode, engine, pending: Vec::new(), live_factory: Box::new(live_factory), claude_dir: None, skip_gaps: true }
    }

    fn live_config(&self) -> LiveConfig {
        let mut cfg = (self.live_factory)();
        if let Some(d) = &self.claude_dir {
            cfg.claude_dir = Some(d.clone());
        }
        cfg
    }

    /// Starts in live mode with the default config (Claude dir resolved, watcher on, 24 h window).
    pub fn new_default() -> Hub {
        Self::new_live(LiveConfig::default)
    }

    fn live_mode(cfg: LiveConfig, next_seq: u64) -> (Mode, Engine) {
        let (recent_window_ms, quiet_after_ms) = (cfg.recent_window_ms, cfg.quiet_after_ms);
        let src = LiveSource::new(cfg);
        let mut engine = Engine::new(EngineConfig { mode: SourceMode::Live, recent_window_ms, quiet_after_ms });
        engine.set_next_seq(next_seq);
        engine.initial_scan(&src);
        let now = Instant::now();
        (Mode::Live { src: Box::new(src), last_liveness: now, last_rescan: now }, engine)
    }

    pub fn mode(&self) -> SourceMode {
        match self.mode {
            Mode::Live { .. } => SourceMode::Live,
            Mode::Replay { .. } => SourceMode::Replay,
        }
    }

    pub fn snapshot(&self) -> &Snapshot {
        self.engine.snapshot()
    }

    pub fn session_extras(&self) -> std::collections::BTreeMap<String, crate::engine::SessionExtras> {
        self.engine.session_extras()
    }

    /// The attention list (see [`crate::attention`]). `None` while files are still being read, so
    /// nothing is concluded from half-read transcripts; empty in replay mode.
    pub fn attention(&self) -> Option<Vec<crate::attention::AttentionItem>> {
        match &self.mode {
            Mode::Live { .. } if self.engine.has_pending() => None,
            Mode::Live { src, .. } => Some(crate::attention::derive(self.engine.snapshot(), &self.engine.session_extras(), src.now())),
            Mode::Replay { .. } => Some(Vec::new()),
        }
    }

    pub fn stats(&self) -> &EngineStats {
        self.engine.stats()
    }

    /// Whether file reading is still pending (live initial scan in progress).
    pub fn is_busy(&self) -> bool {
        self.engine.has_pending()
    }

    pub fn status(&self) -> SourceStatus {
        match &self.mode {
            Mode::Live { src, .. } => SourceStatus {
                mode: SourceMode::Live,
                claude_dir: Some(src.claude_dir().to_string_lossy().into_owned()),
                recording: None,
                replay: None,
                error: src.error().map(str::to_string),
                problems: self.engine.problems(),
            },
            Mode::Replay { src, playing, speed, skip_gaps, .. } => SourceStatus {
                mode: SourceMode::Replay,
                claude_dir: None,
                recording: Some(src.info()),
                replay: Some(ReplayState {
                    playing: *playing,
                    speed: *speed,
                    position: src.position(),
                    start: src.start(),
                    end: src.end(),
                    skip_gaps: *skip_gaps,
                }),
                error: None,
                problems: self.engine.problems(),
            },
        }
    }

    /// Advances the source: reads changed files (live) or moves the replay clock, and returns the
    /// events produced since the last call, in `seq` order.
    pub fn tick(&mut self) -> Vec<Event> {
        match &mut self.mode {
            Mode::Live { src, last_liveness, last_rescan } => {
                let changes = src.poll();
                self.engine.enqueue(changes);
                if last_rescan.elapsed() >= RESCAN_EVERY {
                    *last_rescan = Instant::now();
                    src.ensure_watcher();
                    self.engine.rescan(src.as_ref());
                }
                if last_liveness.elapsed() >= LIVENESS_EVERY {
                    *last_liveness = Instant::now();
                    self.engine.refresh_liveness(src.as_ref());
                }
                self.engine.run_queue(src.as_ref(), Some(LIVE_TICK_BUDGET));
            }
            Mode::Replay { src, playing, speed, skip_gaps, last_tick, last_refresh, carry_ms } => {
                if last_refresh.elapsed() >= REFRESH_RECORDING_EVERY {
                    *last_refresh = Instant::now();
                    src.refresh();
                }
                let elapsed = last_tick.elapsed();
                *last_tick = Instant::now();
                if *playing {
                    let mut rate = *speed;
                    if *skip_gaps {
                        let (prev, next) = src.gap_around();
                        if next - prev > QUIET_GAP_MS {
                            rate = rate.max((next - prev) as f64 / QUIET_GAP_PLAY_MS);
                        }
                    }
                    *carry_ms += elapsed.as_secs_f64() * 1000.0 * rate;
                    let step = carry_ms.floor();
                    *carry_ms -= step;
                    let target = src.position() + step as EpochMs;
                    src.set_position(target);
                    if src.position() >= src.end() {
                        *playing = false;
                    }
                }
                pump_replay(&mut self.engine, src.as_mut());
                self.engine.refresh_clock(src.as_ref());
            }
        }
        self.pending.extend(self.engine.take_events());
        std::mem::take(&mut self.pending)
    }

    /// Live: processes everything currently on disk (the whole initial scan). For CLIs and tests.
    pub fn settle(&mut self) {
        loop {
            let events = self.tick();
            self.pending.extend(events);
            if !self.engine.has_pending() {
                break;
            }
        }
    }

    pub fn open_recording(&mut self, path: &Path) -> Result<SourceStatus, CommandError> {
        let mut src = ReplaySource::open(path)?;
        let start = src.start();
        src.set_position(start);
        let now = Instant::now();
        let reset_seq = self.next_seq();
        self.mode = Mode::Replay {
            src: Box::new(src),
            playing: false,
            speed: 1.0,
            skip_gaps: self.skip_gaps,
            last_tick: now,
            last_refresh: now,
            carry_ms: 0.0,
        };
        self.rebuild_replay(start, reset_seq);
        Ok(self.status())
    }

    /// Every tool call the core keeps per agent, plus (replay) the recording's fs-event times.
    pub fn tool_history(&self) -> ToolHistory {
        ToolHistory {
            seq: self.engine.snapshot().seq,
            agents: self.engine.tool_history(),
            timeline_times: match &self.mode {
                Mode::Replay { src, .. } => {
                    let mut v: Vec<EpochMs> = src.event_times().collect();
                    v.sort_unstable();
                    v
                }
                Mode::Live { .. } => Vec::new(),
            },
        }
    }

    /// Chooses the Claude dir for live mode (`None`: the default resolution). In live mode the
    /// source is rebuilt at once and a `snapshotReset` follows; in replay it applies on the next
    /// `switch_to_live`.
    pub fn set_claude_dir(&mut self, dir: Option<PathBuf>) -> Result<SourceStatus, CommandError> {
        if let Some(d) = &dir {
            if !d.is_dir() {
                return Err(CommandError::new(CommandErrorCode::NotFound, format!("not a folder: {}", d.display())));
            }
        }
        self.claude_dir = dir;
        Ok(match self.mode {
            Mode::Live { .. } => self.switch_to_live(),
            Mode::Replay { .. } => self.status(),
        })
    }

    /// Turns quiet-gap skipping on or off. Works in live mode too: the choice applies to the next
    /// recording.
    pub fn set_skip_gaps(&mut self, on: bool) -> SourceStatus {
        if let Mode::Replay { skip_gaps, .. } = &mut self.mode {
            *skip_gaps = on;
        }
        self.skip_gaps = on;
        self.status()
    }

    pub fn switch_to_live(&mut self) -> SourceStatus {
        let cfg = self.live_config();
        let seq = self.next_seq();
        let (mode, mut engine) = Self::live_mode(cfg, seq);
        let reset = engine.reset_event(seq);
        self.mode = mode;
        self.engine = engine;
        self.pending.clear();
        self.pending.push(reset);
        self.status()
    }

    pub fn replay_play(&mut self) -> Result<SourceStatus, CommandError> {
        let at_end = match &self.mode {
            Mode::Replay { src, .. } => src.position() >= src.end(),
            Mode::Live { .. } => return Err(not_replaying()),
        };
        if at_end {
            let start = self.replay_src().map(|s| s.start()).unwrap_or(0);
            let seq = self.next_seq();
            self.rebuild_replay(start, seq);
        }
        if let Mode::Replay { playing, last_tick, carry_ms, .. } = &mut self.mode {
            *playing = true;
            *last_tick = Instant::now();
            *carry_ms = 0.0;
        }
        Ok(self.status())
    }

    pub fn replay_pause(&mut self) -> Result<SourceStatus, CommandError> {
        match &mut self.mode {
            Mode::Replay { playing, .. } => *playing = false,
            Mode::Live { .. } => return Err(not_replaying()),
        }
        Ok(self.status())
    }

    pub fn replay_seek(&mut self, t: EpochMs) -> Result<SourceStatus, CommandError> {
        if !matches!(self.mode, Mode::Replay { .. }) {
            return Err(not_replaying());
        }
        let seq = self.next_seq();
        self.rebuild_replay(t, seq);
        if let Mode::Replay { last_tick, carry_ms, .. } = &mut self.mode {
            *last_tick = Instant::now();
            *carry_ms = 0.0;
        }
        Ok(self.status())
    }

    pub fn replay_set_speed(&mut self, speed: f64) -> Result<SourceStatus, CommandError> {
        match &mut self.mode {
            Mode::Replay { speed: s, .. } => {
                *s = if speed.is_finite() { speed.clamp(MIN_SPEED, MAX_SPEED) } else { 1.0 };
            }
            Mode::Live { .. } => return Err(not_replaying()),
        }
        Ok(self.status())
    }

    fn replay_src(&self) -> Option<&ReplaySource> {
        match &self.mode {
            Mode::Replay { src, .. } => Some(src),
            Mode::Live { .. } => None,
        }
    }

    fn next_seq(&self) -> u64 {
        self.pending.last().map(|e| e.seq + 1).unwrap_or(0).max(self.engine.next_seq())
    }

    /// Throws away all state, applies every fs-event up to `t` and queues one `snapshotReset`.
    fn rebuild_replay(&mut self, t: EpochMs, reset_seq: u64) {
        let Mode::Replay { src, .. } = &mut self.mode else { return };
        src.rewind();
        src.set_position(t);
        let mut engine = Engine::new(EngineConfig::replay());
        pump_replay(&mut engine, src.as_mut());
        let reset = engine.reset_event(reset_seq);
        self.engine = engine;
        self.pending.clear();
        self.pending.push(reset);
    }
}

fn pump_replay(engine: &mut Engine, src: &mut ReplaySource) {
    loop {
        let changes = src.poll();
        if changes.is_empty() {
            break;
        }
        engine.enqueue(changes);
        engine.run_queue(&*src, None);
    }
}

fn not_replaying() -> CommandError {
    CommandError::new(CommandErrorCode::NotReplaying, "not in replay mode")
}

/// The default recording places (contract 5.1), duplicates removed: `$VIRGIL_RECORDINGS_DIR`,
/// `<app data dir>/recordings`, `./recordings`.
pub fn default_recording_dirs(app_data_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(d) = std::env::var_os("VIRGIL_RECORDINGS_DIR").filter(|d| !d.is_empty()) {
        dirs.push(PathBuf::from(d));
    }
    if let Some(a) = app_data_dir {
        dirs.push(a.join("recordings"));
    }
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd.join("recordings"));
    }
    let mut seen = std::collections::HashSet::new();
    dirs.retain(|d| seen.insert(std::fs::canonicalize(d).unwrap_or_else(|_| d.clone())));
    dirs
}
