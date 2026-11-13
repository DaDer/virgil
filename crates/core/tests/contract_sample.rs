//! `fixtures/contract-sample.json` must match what the core serialises today.
//! Regenerate with `npm run contract:sample`.

#[allow(dead_code)]
#[path = "../examples/contract_sample.rs"]
mod sample;

#[test]
fn contract_sample_is_up_to_date() {
    let fresh = sample::build();
    // A Windows checkout may have converted the file's line endings.
    let on_disk = std::fs::read_to_string(sample::sample_path()).unwrap_or_default().replace("\r\n", "\n");
    let first_difference = fresh.lines().zip(on_disk.lines()).enumerate().find(|(_, (a, b))| a != b).map(|(i, (a, b))| {
        let cut = |s: &str| s.chars().take(200).collect::<String>();
        format!("line {}:\n  core: {}\n  file: {}", i + 1, cut(a), cut(b))
    });
    assert!(
        fresh == on_disk,
        "fixtures/contract-sample.json is stale or missing: run `npm run contract:sample`\n{}",
        first_difference.unwrap_or_else(|| format!("lengths differ: core {} bytes, file {} bytes", fresh.len(), on_disk.len()))
    );
}

#[test]
fn contract_sample_covers_every_event_type() {
    let v: serde_json::Value = serde_json::from_str(&sample::build()).unwrap();
    let types: std::collections::BTreeSet<&str> =
        v["events"].as_array().unwrap().iter().map(|e| e["type"].as_str().unwrap()).collect();
    for t in [
        "snapshotReset", "sessionUpserted", "sessionRemoved", "agentSpawned", "agentUpdated", "toolCallStarted",
        "toolCallFinished", "fileTouched", "runStarted", "runUpdated", "runFinalized",
    ] {
        assert!(types.contains(t), "sample lacks {t}");
    }
}
