use std::{fs, process::Command};

fn probe(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_brave-isole-probe"))
        .args(args)
        .output()
        .expect("launch test probe");
    assert!(output.status.success(), "probe failed: {:?}", output.status);
    String::from_utf8(output.stdout).expect("UTF-8 probe result")
}

#[test]
fn reports_readable_witness_without_returning_its_content() {
    let path = std::env::temp_dir().join(format!("brave-isole-read-{}", std::process::id()));
    fs::write(&path, b"synthetic-private-marker").expect("create own fixture");
    let output = probe(&["read", path.to_str().expect("path")]);
    fs::remove_file(path).expect("remove own fixture");
    assert_eq!(output.trim(), r#"{"ok":true,"errno":0}"#);
    assert!(!output.contains("synthetic-private-marker"));
}

#[test]
fn reports_missing_file_as_a_failure() {
    let output = probe(&["read", "/nonexistent-brave-isole-witness-670c00c4"]);
    assert_eq!(output.trim(), r#"{"ok":false,"errno":2}"#);
}

#[test]
fn rejects_an_unknown_operation() {
    let output = Command::new(env!("CARGO_BIN_EXE_brave-isole-probe"))
        .arg("shell")
        .output()
        .expect("launch test probe");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}

#[test]
fn writes_only_the_synthetic_marker() {
    let path = std::env::temp_dir().join(format!("brave-isole-write-{}", std::process::id()));
    let output = probe(&["write", path.to_str().expect("path")]);
    assert_eq!(output.trim(), r#"{"ok":true,"errno":0}"#);
    assert_eq!(
        fs::read(&path).expect("written fixture"),
        b"brave-isole-synthetic-witness"
    );
    fs::remove_file(path).expect("remove own fixture");
}

#[test]
fn reaches_a_listening_tcp_control() {
    let server = std::net::TcpListener::bind("127.0.0.1:0").expect("control listener");
    let target = server.local_addr().expect("address").to_string();
    assert_eq!(probe(&["tcp", &target]).trim(), r#"{"ok":true,"errno":0}"#);
}

#[test]
fn reaches_a_listening_udp_control() {
    let server = std::net::UdpSocket::bind("127.0.0.1:0").expect("control listener");
    server
        .set_read_timeout(Some(std::time::Duration::from_secs(1)))
        .expect("timeout");
    let target = server.local_addr().expect("address").to_string();
    assert_eq!(probe(&["udp", &target]).trim(), r#"{"ok":true,"errno":0}"#);
    let mut buf = [0; 64];
    let (n, _) = server
        .recv_from(&mut buf)
        .expect("control received witness");
    assert_eq!(&buf[..n], b"brave-isole-probe");
}

#[test]
fn reaches_a_private_unix_control() {
    let path = std::env::temp_dir().join(format!("bi-{}.sock", std::process::id()));
    let server = std::os::unix::net::UnixListener::bind(&path).expect("control listener");
    let output = probe(&["unix-connect", path.to_str().expect("path")]);
    assert_eq!(output.trim(), r#"{"ok":true,"errno":0}"#);
    drop(server);
    fs::remove_file(path).expect("remove own fixture");
}

#[test]
fn child_forwards_a_probe_without_changing_the_result() {
    assert_eq!(
        probe(&["child", "read", "/nonexistent-brave-isole-witness-670c00c4"]).trim(),
        r#"{"ok":false,"errno":2}"#
    );
}

#[test]
fn unix_server_accepts_an_independent_client() {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;
    let path = std::env::temp_dir().join(format!("bi-server-{}.sock", std::process::id()));
    let mut server = Command::new(env!("CARGO_BIN_EXE_brave-isole-probe"))
        .args(["unix-serve", path.to_str().expect("path")])
        .stdout(Stdio::piped())
        .spawn()
        .expect("server");
    let mut reader = BufReader::new(server.stdout.take().expect("stdout"));
    let mut line = String::new();
    reader.read_line(&mut line).expect("ready");
    assert_eq!(line.trim(), r#"{"ready":true}"#);
    assert_eq!(
        probe(&["unix-connect", path.to_str().expect("path")]).trim(),
        r#"{"ok":true,"errno":0}"#
    );
    assert!(server.wait().expect("wait").success());
    assert!(!path.exists());
}

#[test]
fn product_probe_has_same_denial_shape_without_content() {
    let output = Command::new(env!("CARGO_BIN_EXE_brave-isole"))
        .args([
            "--diagnose-isolation",
            "read",
            "/nonexistent-brave-isole-witness-670c00c4",
        ])
        .output()
        .expect("product diagnostic");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("UTF-8").trim(),
        r#"{"ok":false,"errno":2}"#
    );
}
