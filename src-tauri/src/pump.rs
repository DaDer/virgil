//! The background loop that drives the core [`Hub`] and forwards what it produces to the UI
//! (data-contract 5.2). It knows nothing about Tauri: it talks to a [`Sink`], so tests can drive it.
//!
//! - One thread owns the loop. It ticks the hub every [`TICK`] and sends **at most one event batch
//!   per tick**, so bursts are coalesced and the UI never gets more than 10 batches a second.
//! - Status goes out whenever it changes, except that a replay clock that only moved is sent at
//!   most every [`CLOCK_EVERY`] (about four times a second while playing).
//! - Commands run on other threads through [`Shared::with_hub`]. They never send anything
//!   themselves; the next tick sends the resulting events and status, which keeps both channels in
//!   order.
//! - The hub owns the only source (live watcher or replay). Mode switches replace it, which drops
//!   the old watcher, so switching never accumulates watchers or threads.

use virgil_core::{AttentionItem, EventBatch, Hub, SourceStatus};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub const TICK: Duration = Duration::from_millis(100);
pub const CLOCK_EVERY: Duration = Duration::from_millis(250);
/// Breathing room between ticks so commands can take the lock even when a tick runs long.
/// The attention list is derived once a second, not on every tick.
pub const ATTENTION_EVERY: u32 = 10;
const MIN_GAP: Duration = Duration::from_millis(10);

pub trait Sink: Send + 'static {
    fn events(&self, batch: EventBatch);
    fn status(&self, status: &SourceStatus);
    /// The attention list, sent when it changed (checked every [`ATTENTION_EVERY`] ticks).
    fn attention(&self, _items: Vec<AttentionItem>) {}
}

/// The hub, shared between the pump thread and command handlers.
#[derive(Default)]
pub struct Shared {
    hub: Mutex<Option<Hub>>,
    ready: Condvar,
    stop: AtomicBool,
}

impl Shared {
    pub fn new() -> Arc<Shared> {
        Arc::new(Shared::default())
    }

    fn lock(&self) -> MutexGuard<'_, Option<Hub>> {
        // A panic inside the core must not take the whole app down with a poisoned lock.
        self.hub.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Installs the hub and wakes commands that arrived before it existed.
    pub fn install(&self, hub: Hub) {
        *self.lock() = Some(hub);
        self.ready.notify_all();
    }

    /// Runs `f` on the hub, waiting for it to be installed first.
    pub fn with_hub<R>(&self, f: impl FnOnce(&mut Hub) -> R) -> R {
        let mut g = self.lock();
        while g.is_none() {
            g = self.ready.wait(g).unwrap_or_else(|e| e.into_inner());
        }
        f(g.as_mut().expect("hub installed"))
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    pub fn stopped(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

}

pub struct Pump<S: Sink> {
    shared: Arc<Shared>,
    sink: S,
    last_status: Option<SourceStatus>,
    last_status_at: Instant,
    ticks: u32,
    last_attention: Option<Vec<AttentionItem>>,
}

impl<S: Sink> Pump<S> {
    pub fn new(shared: Arc<Shared>, sink: S) -> Pump<S> {
        Pump { shared, sink, last_status: None, last_status_at: Instant::now(), ticks: 0, last_attention: None }
    }

    /// One tick: advance the hub, send the events it produced as one batch, maybe send status.
    pub fn step(&mut self) {
        let shared = self.shared.clone();
        let result = catch_unwind(AssertUnwindSafe(|| shared.with_hub(|h| (h.tick(), h.status()))));
        let (events, status) = match result {
            Ok(r) => r,
            Err(_) => {
                eprintln!("virgil: the core panicked during a tick; continuing");
                return;
            }
        };
        if !events.is_empty() {
            self.sink.events(EventBatch { events });
        }
        if self.should_send(&status) {
            self.sink.status(&status);
            self.last_status = Some(status);
            self.last_status_at = Instant::now();
        }
        self.ticks = self.ticks.wrapping_add(1);
        if self.ticks % ATTENTION_EVERY == 0 {
            let shared = self.shared.clone();
            if let Ok(Some(items)) = catch_unwind(AssertUnwindSafe(|| shared.with_hub(|h| h.attention()))) {
                if self.last_attention.as_ref() != Some(&items) {
                    self.last_attention = Some(items.clone());
                    self.sink.attention(items);
                }
            }
        }
    }

    fn should_send(&self, s: &SourceStatus) -> bool {
        let Some(prev) = &self.last_status else { return true };
        if prev == s {
            return false;
        }
        let playing = s.replay.as_ref().is_some_and(|r| r.playing);
        if playing && only_clock_moved(prev, s) {
            return self.last_status_at.elapsed() >= CLOCK_EVERY;
        }
        true
    }
}

fn only_clock_moved(a: &SourceStatus, b: &SourceStatus) -> bool {
    let strip = |s: &SourceStatus| {
        let mut s = s.clone();
        if let Some(r) = s.replay.as_mut() {
            r.position = 0;
        }
        s
    };
    strip(a) == strip(b)
}

/// Starts the pump thread. `make_hub` runs on that thread, so a slow first scan never blocks the
/// window from opening; commands wait until it is done.
pub fn spawn<S: Sink>(shared: Arc<Shared>, sink: S, make_hub: impl FnOnce() -> Hub + Send + 'static) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("virgil-pump".into())
        .spawn(move || {
            shared.install(make_hub());
            let mut pump = Pump::new(shared.clone(), sink);
            while !shared.stopped() {
                let started = Instant::now();
                pump.step();
                std::thread::sleep(TICK.saturating_sub(started.elapsed()).max(MIN_GAP));
            }
        })
        .expect("spawn pump thread")
}

#[cfg(test)]
mod tests {
    use super::*;
    use virgil_core::{LiveConfig, SourceMode};
    use std::path::PathBuf;
    use std::sync::mpsc::{channel, Receiver, Sender};

    #[derive(Debug)]
    enum Out {
        Batch(EventBatch),
        Status(Box<SourceStatus>),
    }

    struct TestSink(Sender<Out>);
    impl Sink for TestSink {
        fn events(&self, batch: EventBatch) {
            let _ = self.0.send(Out::Batch(batch));
        }
        fn status(&self, status: &SourceStatus) {
            let _ = self.0.send(Out::Status(Box::new(status.clone())));
        }
    }

    fn repo(p: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(p)
    }

    fn fixture_hub() -> Hub {
        let dir = repo("fixtures/claude-small");
        Hub::new_live(move || LiveConfig { claude_dir: Some(dir.clone()), recent_window_ms: None, ..LiveConfig::default() })
    }

    fn pump() -> (Arc<Shared>, Pump<TestSink>, Receiver<Out>) {
        let shared = Shared::new();
        shared.install(fixture_hub());
        let (tx, rx) = channel();
        (shared.clone(), Pump::new(shared, TestSink(tx)), rx)
    }

    fn drain(rx: &Receiver<Out>) -> Vec<Out> {
        rx.try_iter().collect()
    }

    #[test]
    fn one_batch_per_tick_in_seq_order_and_status_once() {
        let (_shared, mut p, rx) = pump();
        let mut seqs = Vec::new();
        let mut statuses = 0;
        for _ in 0..20 {
            p.step();
            let out = drain(&rx);
            assert!(out.iter().filter(|o| matches!(o, Out::Batch(_))).count() <= 1);
            for o in out {
                match o {
                    Out::Batch(b) => {
                        assert!(!b.events.is_empty());
                        seqs.extend(b.events.iter().map(|e| e.seq));
                    }
                    Out::Status(s) => {
                        statuses += 1;
                        assert_eq!(s.mode, SourceMode::Live);
                    }
                }
            }
        }
        assert!(!seqs.is_empty(), "the fixture Claude dir produces events");
        assert!(seqs.windows(2).all(|w| w[1] == w[0] + 1), "seq has no gaps across batches");
        assert_eq!(statuses, 1, "an unchanged status is sent once");
    }

    #[test]
    fn replay_commands_reach_the_ui_through_the_next_tick() {
        let (shared, mut p, rx) = pump();
        p.step();
        drain(&rx);
        let st = shared.with_hub(|h| h.open_recording(&repo("fixtures/recording-small"))).unwrap();
        assert_eq!(st.mode, SourceMode::Replay);
        p.step();
        let out = drain(&rx);
        let first = out.iter().find_map(|o| match o {
            Out::Batch(b) => b.events.first().cloned(),
            _ => None,
        });
        assert_eq!(first.map(|e| e.kind.type_name()), Some("snapshotReset"));
        assert!(out.iter().any(|o| matches!(o, Out::Status(s) if s.mode == SourceMode::Replay)));

        // Playing: the clock-only status changes are throttled.
        shared.with_hub(|h| {
            h.replay_set_speed(2.0).unwrap();
            h.replay_play().unwrap();
        });
        let mut statuses = 0;
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_millis(1000) {
            p.step();
            std::thread::sleep(Duration::from_millis(20));
            statuses += drain(&rx).iter().filter(|o| matches!(o, Out::Status(_))).count();
        }
        assert!((2..=7).contains(&statuses), "about four clock updates a second, got {statuses}");
    }

    #[test]
    fn mode_switches_do_not_leak_threads() {
        let (shared, mut p, rx) = pump();
        let rec = repo("fixtures/recording-small");
        let cycle = |n: usize, p: &mut Pump<TestSink>| {
            for _ in 0..n {
                shared.with_hub(|h| h.open_recording(&rec)).unwrap();
                p.step();
                shared.with_hub(|h| h.switch_to_live());
                p.step();
            }
            drain(&rx);
        };
        cycle(3, &mut p);
        let before = thread_count();
        cycle(25, &mut p);
        let after = thread_count();
        if let (Some(b), Some(a)) = (before, after) {
            assert!(a <= b + 1, "threads grew from {b} to {a} over 25 live/replay switches");
        }
    }

    /// Threads of this process, where the platform makes it cheap to ask.
    fn thread_count() -> Option<usize> {
        if cfg!(target_os = "linux") {
            return std::fs::read_dir("/proc/self/task").ok().map(|d| d.count());
        }
        if cfg!(target_os = "macos") {
            let out = std::process::Command::new("ps").args(["-M", "-p", &std::process::id().to_string()]).output().ok()?;
            return Some(String::from_utf8_lossy(&out.stdout).lines().count().saturating_sub(1));
        }
        None
    }

    /// Opt-in: plays a real recording through the shell path at high speed.
    /// `VIRGIL_SMOKE_RECORDING=recordings/first-build cargo test -p virgil -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn smoke_replay_of_a_real_recording() {
        let Some(dir) = std::env::var_os("VIRGIL_SMOKE_RECORDING") else { return };
        let dir = repo(&PathBuf::from(dir).to_string_lossy());
        let (shared, mut p, rx) = pump();
        let st = shared.with_hub(|h| h.open_recording(&dir)).expect("open");
        let r = st.replay.unwrap();
        shared.with_hub(|h| {
            h.replay_set_speed(600.0).unwrap();
            h.replay_play().unwrap();
        });
        let (mut batches, mut events, mut statuses, mut max_batch) = (0, 0, 0, 0);
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(3) {
            let s = Instant::now();
            p.step();
            for o in drain(&rx) {
                match o {
                    Out::Batch(b) => {
                        batches += 1;
                        events += b.events.len();
                        max_batch = max_batch.max(b.events.len());
                    }
                    Out::Status(_) => statuses += 1,
                }
            }
            std::thread::sleep(TICK.saturating_sub(s.elapsed()));
        }
        let pos = shared.with_hub(|h| h.status().replay.unwrap().position);
        println!(
            "timeline {} s, played to +{} s in 3 s: {batches} batches, {events} events (largest {max_batch}), {statuses} status updates",
            (r.end - r.start) / 1000,
            (pos - r.start) / 1000
        );
        assert!(batches <= 31 && events > 0);
    }

    #[test]
    fn commands_wait_for_the_hub() {
        let shared = Shared::new();
        let s2 = shared.clone();
        let waiter = std::thread::spawn(move || s2.with_hub(|h| h.mode()));
        std::thread::sleep(Duration::from_millis(50));
        shared.install(fixture_hub());
        assert_eq!(waiter.join().unwrap(), SourceMode::Live);
    }
}
