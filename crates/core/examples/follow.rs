//! Follow the live source and print every event as it arrives.
//! `cargo run -p virgil-core --example follow -- [seconds]`
use std::time::{Duration, Instant};

fn main() {
    let secs: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(30);
    let mut hub = virgil_core::Hub::new_default();
    let start = Instant::now();
    let mut settled = false;
    let mut total = 0usize;
    while start.elapsed() < Duration::from_secs(secs) {
        let events = hub.tick();
        if !settled && !hub.is_busy() {
            settled = true;
            println!("[{:>6.1}s] initial scan done, {} events so far; now following", start.elapsed().as_secs_f32(), total + events.len());
        } else if settled {
            for e in &events {
                let s = serde_json::to_string(e).unwrap_or_default();
                let cut = s.char_indices().nth(220).map(|(i, _)| i).unwrap_or(s.len());
                println!("[{:>6.1}s] {}", start.elapsed().as_secs_f32(), &s[..cut]);
            }
        }
        total += events.len();
        std::thread::sleep(Duration::from_millis(100));
    }
    println!("total events {total}; status {}", serde_json::to_string(&hub.status()).unwrap_or_default());
}
