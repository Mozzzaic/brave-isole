use brave_isole::{
    ipc::BridgeConfig,
    verification::{ProofRole, verify_launch},
};
#[test]
fn unconfined_process_cannot_pass_launch_proof() {
    let root = std::env::temp_dir().join(format!("bi-verify-{}", std::process::id()));
    let config = BridgeConfig::initialize(&root, &"a".repeat(32)).expect("config");
    let witness = root.join("outside-test");
    std::fs::write(&witness, b"synthetic-witness").expect("fixture");
    let request = serde_json::json!({"nonce":"a".repeat(64),"witness":witness,"binary_sha256":"b".repeat(64),"extension_id":"a".repeat(32),"tcp4":"127.0.0.1:9","tcp6":"[::1]:9"});
    std::fs::write(root.join("verification.json"), request.to_string()).expect("request");
    let report = verify_launch(&config, ProofRole::Codex).expect("report");
    assert!(!report.passed);
    assert_eq!(report.role, ProofRole::Codex);
    assert!(
        root.join(format!("proof-codex-{}.json", report.pid))
            .is_file()
    );
    let encoded = serde_json::to_string(&report).expect("report JSON");
    assert!(!encoded.contains("synthetic-witness"));
    std::fs::remove_dir_all(root).expect("own cleanup");
}
#[test]
fn verification_server_exposes_no_browser_capability() {
    use rmcp::ServerHandler;
    let server = brave_isole::verification::VerificationServer;
    assert!(server.get_info().capabilities.tools.is_none());
    assert!(server.get_tool("browser_open").is_none());
}
