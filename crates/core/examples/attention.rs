//! Print the attention list for the live Claude dir.
//! `cargo run -p virgil-core --example attention`
fn main() {
    let mut hub = virgil_core::Hub::new_default();
    hub.settle();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    if std::env::args().any(|a| a == "--background") {
        for (id, x) in hub.session_extras() {
            for b in &x.background {
                println!("{} {:>5} min ago  by {:<18} {}", &id[..8], (now - b.started_at) / 60_000, b.agent_id, b.summary);
            }
        }
        return;
    }
    for i in hub.attention().unwrap_or_default() {
        let mins = (now - i.since) / 60_000;
        println!("{:<9} {:<14} {:>5} min  agents {} bg {}  {}", format!("{:?}", i.kind), i.name, mins, i.running_agents, i.background_tasks, i.line);
    }
}
