//! `fixtures/contract-sample.json` must match what the core serialises today.
//! Regenerate with `npm run contract:sample`.

#[allow(dead_code)]
#[path = "../examples/contract_sample.rs"]
mod sample;

#[test]
fn contract_sample_is_up_to_date() {
    let fresh = sample::build();
    let on_disk = std::fs::read_to_string(sample::sample_path()).unwrap_or_default();
    assert!(
        fresh == on_disk,
        "fixtures/contract-sample.json is stale or missing: run `npm run contract:sample`"
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
