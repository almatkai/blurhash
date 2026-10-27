#[path = "support/host_validator.rs"]
mod validator;
use serde_json::{json, Value};
#[test]
fn actual_frozen_host_runtime_validator_positive_and_negative() {
    let manifest: Value = serde_json::from_str(include_str!("../extension.json")).unwrap();
    validator::frozen::validate(&manifest).unwrap(); // artifacts:{} is valid for source; not a trusted release artifact
    for (field, value) in [
        ("schemaVersion", json!(2)),
        ("runtime", json!("javascript")),
        ("protocolVersion", json!(2)),
        ("id", json!("com.tezbar.blurhash")),
        ("capabilities", json!(["process.exec"])),
        ("migrationPaths", json!({"escape":"../elsewhere"})),
    ] {
        let mut bad = manifest.clone();
        bad[field] = value;
        assert!(validator::frozen::validate(&bad).is_err(), "{field}");
    }
    for (field, value) in [
        ("concurrency", json!("anything")),
        ("timeoutMs", json!(300001)),
        ("refreshMs", json!(1)),
    ] {
        let mut bad = manifest.clone();
        bad["commands"][0][field] = value;
        assert!(validator::frozen::validate(&bad).is_err(), "{field}");
    }
}
