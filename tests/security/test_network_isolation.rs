#[test]
fn test_recovery_crates_network_isolation() {
    // Audit the Cargo.lock file to ensure no telemetry, advertising, or unexpected network crates are present in recovery core
    let cargo_lock = std::fs::read_to_string("Cargo.lock").expect("Cargo.lock must exist");

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
