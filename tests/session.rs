use brave_isole::{
    ipc::{validate_origin, validate_peer_uid, validate_private_dir, validate_secret},
    protocol::{BridgeError, Command, Envelope, Payload, Role},
    session::{SessionRegistry, token},
};
use std::{fs, os::unix::fs::PermissionsExt};
fn envelope(epoch: &str, role: Role) -> Envelope {
    Envelope {
        version: 1,
        expires_at_ms: None,
        epoch: epoch.into(),
        id: "test".into(),
        payload: Payload::Command {
            role,
            command: Command::List {},
        },
    }
}
#[test]
fn codex_cannot_claim_claude_role() {
    let mut registry = SessionRegistry::new().expect("random");
    let session = registry.open(Role::Codex).expect("session");
    assert_eq!(
        registry.authorize(&session, &envelope(&session.epoch, Role::Claude)),
        Err(BridgeError::Unauthorized)
    );
}
#[test]
fn revoked_epoch_rejects_late_reply() {
    let mut registry = SessionRegistry::new().expect("random");
    let session = registry.open(Role::Codex).expect("session");
    registry.revoke_all();
    assert_eq!(
        registry.authorize(&session, &envelope(&session.epoch, Role::Codex)),
        Err(BridgeError::StaleSession)
    );
}
#[test]
fn reconnect_gets_new_epoch() {
    assert_ne!(
        SessionRegistry::new().expect("random").epoch(),
        SessionRegistry::new().expect("random").epoch()
    );
}
#[test]
fn wrong_secret_rejected() {
    assert_eq!(
        validate_secret(&"a".repeat(64), &"b".repeat(64)),
        Err(BridgeError::Unauthorized)
    );
    assert!(validate_secret(&"a".repeat(64), &"a".repeat(64)).is_ok());
}
#[test]
fn wrong_peer_rejected() {
    assert_eq!(validate_peer_uid(42, 43), Err(BridgeError::Unauthorized));
}
#[test]
fn wrong_extension_origin_rejected() {
    let id = "a".repeat(32);
    assert!(validate_origin(&format!("chrome-extension://{id}/"), &id).is_ok());
    for origin in [
        "https://evil.test/",
        "chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.evil/",
    ] {
        assert_eq!(validate_origin(origin, &id), Err(BridgeError::Unauthorized));
    }
}
#[test]
fn public_directory_and_symlink_are_refused() {
    let dir = std::env::temp_dir().join(format!("bi-session-{}", std::process::id()));
    fs::create_dir(&dir).expect("fixture");
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).expect("mode");
    assert!(validate_private_dir(&dir).is_err());
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).expect("mode");
    assert!(validate_private_dir(&dir).is_ok());
    let link = dir.with_extension("link");
    std::os::unix::fs::symlink(&dir, &link).expect("link");
    assert!(validate_private_dir(&link).is_err());
    fs::remove_file(link).expect("cleanup");
    fs::remove_dir(dir).expect("cleanup");
}
#[test]
fn role_cannot_reuse_another_sessions_capability() {
    let mut registry = SessionRegistry::new().expect("random");
    let one = registry.open(Role::Codex).expect("session");
    let two = registry.open(Role::Claude).expect("session");
    assert_ne!(one.id(), two.id());
    assert!(
        registry
            .authorize(&one, &envelope(&one.epoch, Role::Codex))
            .is_ok()
    );
}
#[test]
fn random_tokens_have_enough_entropy_and_no_repetition() {
    let one = token().expect("random");
    assert_eq!(one.len(), 64);
    assert_ne!(one, token().expect("random"));
}

#[test]
fn response_is_routed_only_to_requester() {
    use brave_isole::{
        framing::{read_native, write_native},
        ipc::{BridgeConfig, NativeBridge},
        native_host::run_native,
        protocol::Reply,
    };
    use std::os::unix::net::UnixStream;
    let root = std::env::temp_dir().join(format!("bi-route-{}", std::process::id()));
    let config = BridgeConfig::initialize(&root, &"a".repeat(32)).expect("config");
    let (input, mut browser_writer) = UnixStream::pair().expect("input");
    let (output, mut browser_reader) = UnixStream::pair().expect("output");
    let host_config = config.clone();
    let host = std::thread::spawn(move || {
        run_native(
            input,
            output,
            host_config,
            &format!("chrome-extension://{}/", "a".repeat(32)),
        )
    });
    browser_reader
        .set_read_timeout(Some(std::time::Duration::from_secs(2)))
        .expect("diagnostic timeout");
    let hello = read_native(&mut browser_reader)
        .expect("hello")
        .expect("frame");
    assert!(matches!(hello.payload, Payload::Hello {}));
    let one = NativeBridge::connect(&config, Role::Codex).expect("codex");
    let two = NativeBridge::connect(&config, Role::Claude).expect("claude");
    let a = std::thread::spawn(move || one.dispatch(one.session(), Command::List {}));
    let b = std::thread::spawn(move || two.dispatch(two.session(), Command::List {}));
    let first = read_native(&mut browser_reader)
        .expect("request")
        .expect("frame");
    let second = read_native(&mut browser_reader)
        .expect("request")
        .expect("frame");
    for request in [second, first] {
        let role = match request.payload {
            Payload::Command { role, .. } => role,
            _ => panic!("command"),
        };
        let reply = Envelope {
            version: 1,
            expires_at_ms: None,
            epoch: request.epoch,
            id: request.id,
            payload: Payload::Reply {
                reply: Reply::success(serde_json::json!(role.label())),
            },
        };
        write_native(&mut browser_writer, &reply).expect("reply");
    }
    assert_eq!(
        a.join().expect("thread").expect("dispatch").result,
        Some(serde_json::json!("codex"))
    );
    assert_eq!(
        b.join().expect("thread").expect("dispatch").result,
        Some(serde_json::json!("claude"))
    );
    drop(browser_writer);
    assert!(host.join().expect("host").is_ok());
    fs::remove_dir_all(root).expect("own fixture cleanup");
}

#[test]
fn occupied_socket_is_not_removed() {
    use brave_isole::ipc::BridgeConfig;
    let root = std::env::temp_dir().join(format!("bi-active-{}", std::process::id()));
    let config = BridgeConfig::initialize(&root, &"a".repeat(32)).expect("config");
    let listener = config.bind().expect("bind");
    assert!(config.bind().is_err());
    assert!(config.socket().exists());
    drop(listener);
    fs::remove_dir_all(root).expect("own fixture cleanup");
}
#[test]
fn socket_symlink_is_refused() {
    use brave_isole::ipc::BridgeConfig;
    let root = std::env::temp_dir().join(format!("bi-link-{}", std::process::id()));
    let config = BridgeConfig::initialize(&root, &"a".repeat(32)).expect("config");
    std::os::unix::fs::symlink(root.join("codex"), config.socket()).expect("link");
    assert!(config.bind().is_err());
    fs::remove_dir_all(root).expect("own fixture cleanup");
}

#[test]
fn authenticated_idle_connection_remains_usable() {
    use brave_isole::{framing::write_native, ipc::read_authenticated};
    use std::os::unix::net::UnixStream;
    let (mut sender, mut receiver) = UnixStream::pair().expect("pair");
    receiver
        .set_read_timeout(Some(std::time::Duration::from_millis(5)))
        .expect("timeout");
    let frame = envelope("epoch", Role::Codex);
    let expected = frame.clone();
    let handle = std::thread::spawn(move || read_authenticated(&mut receiver));
    std::thread::sleep(std::time::Duration::from_millis(30));
    write_native(&mut sender, &frame).expect("send after idle");
    assert_eq!(
        handle.join().expect("thread").expect("read"),
        Some(expected)
    );
}

#[test]
fn partial_authenticated_frame_has_an_absolute_deadline() {
    use brave_isole::ipc::read_until;
    use std::{
        io::Write,
        os::unix::net::UnixStream,
        time::{Duration, Instant},
    };
    let (mut sender, mut receiver) = UnixStream::pair().expect("pair");
    sender.write_all(&[10]).expect("partial header");
    let result = read_until(&mut receiver, Instant::now() + Duration::from_millis(10));
    assert_eq!(result, Err(BridgeError::Timeout));
}

#[test]
fn stopped_socket_is_reported_as_disconnected_so_client_can_reconnect() {
    use brave_isole::{
        framing::{read_native, write_native},
        ipc::{BridgeConfig, NativeBridge},
    };
    let root = std::env::temp_dir().join(format!("bi-stopped-{}", std::process::id()));
    let config = BridgeConfig::initialize(&root, &"a".repeat(32)).expect("config");
    let listener = config.bind().expect("listen");
    let (closed, ready) = std::sync::mpsc::channel();
    let (stop, should_stop) = std::sync::mpsc::channel();
    let host = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("client");
        read_native(&mut stream).expect("authentication");
        write_native(
            &mut stream,
            &Envelope {
                version: 1,
                expires_at_ms: None,
                epoch: "epoch".into(),
                id: "auth".into(),
                payload: Payload::Hello {},
            },
        )
        .expect("hello");
        should_stop.recv().expect("handshake consumed");
        stream.shutdown(std::net::Shutdown::Both).expect("stop");
        drop(stream);
        closed.send(()).expect("closed");
    });
    let bridge = NativeBridge::connect(&config, Role::Codex).expect("connect");
    stop.send(()).expect("stop host");
    ready.recv().expect("host stopped");
    assert_eq!(
        bridge.dispatch(bridge.session(), Command::List {}),
        Err(BridgeError::Disconnected)
    );
    host.join().expect("host");
    std::fs::remove_dir_all(root).expect("own cleanup");
}
