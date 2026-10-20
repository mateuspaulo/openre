#[test]
fn test_recovery_crates_network_isolation() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_root = std::path::Path::new(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("Should locate workspace root");

    let cargo_lock_path = workspace_root.join("Cargo.lock");
    let cargo_lock = std::fs::read_to_string(&cargo_lock_path)
        .expect("Cargo.lock must exist at workspace root");

    let forbidden_crates = [
        "reqwest",
        "hyper",
        "curl",
        "ureq",
        "tokio-tungstenite",
        "tungstenite",
        "telemetry",
        "analytics",
        "sentry",
        "datadog",
    ];

    for forbidden in &forbidden_crates {
        assert!(
            !cargo_lock.contains(&format!("name = \"{}\"", forbidden)),
            "Security/Privacy violation: Forbidden network or telemetry dependency '{}' found in Cargo.lock!",
            forbidden
        );
    }
}
