use brave_isole::{
    ipc::BridgeConfig,
    mcp::{BrowserServer, parse_command, tools},
    protocol::{BridgeError, Role},
};
use serde_json::json;
#[test]
fn tools_list_is_closed() {
    let names: Vec<_> = tools().into_iter().map(|t| t.name.to_string()).collect();
    assert_eq!(
        names,
        vec![
            "browser_open",
            "browser_tabs",
            "browser_close",
            "browser_navigate",
            "browser_snapshot",
            "browser_screenshot",
            "browser_click",
            "browser_fill",
            "browser_scroll",
            "browser_wait",
            "browser_evaluate"
        ]
    );
}
#[test]
fn tool_arguments_cannot_override_role() {
    for args in [
        json!({"role":"claude"}),
        json!({"shell":"echo"}),
        json!({"path":"/tmp"}),
        json!({"cdp_method":"Browser.close"}),
        json!({"op":"open","url":"https://example.org"}),
    ] {
        assert_eq!(
            parse_command("browser_tabs", args.as_object().expect("object").clone()),
            Err(BridgeError::InvalidRequest)
        );
    }
}
#[tokio::test]
async fn disconnected_is_structured_error() {
    let root = std::env::temp_dir().join(format!("bi-mcp-{}", std::process::id()));
    BridgeConfig::initialize(&root, &"a".repeat(32)).expect("config");
    let server = BrowserServer::new(Role::Codex, root.clone());
    let result = server.invoke("browser_tabs", serde_json::Map::new()).await;
    assert_eq!(result.is_error, Some(true));
    let encoded = serde_json::to_string(&result).expect("JSON");
    assert!(encoded.contains("disconnected"));
    std::fs::remove_dir_all(root).expect("own fixture cleanup");
}
#[test]
fn oversized_result_is_explicit() {
    let args = json!({"tab":"handle","expression":"😀".repeat(262_144)});
    assert_eq!(
        parse_command(
            "browser_evaluate",
            args.as_object().expect("object").clone()
        ),
        Err(BridgeError::TooLarge)
    );
}
#[test]
fn stdout_contains_only_mcp_and_stderr_contains_no_content() {
    use std::{
        io::{BufRead, BufReader, Write},
        process::{Command, Stdio},
    };
    let home = std::env::temp_dir().join(format!("bi-mcp-home-{}", std::process::id()));
    let mut child = Command::new(env!("CARGO_BIN_EXE_brave-isole"))
        .arg("--mcp-codex")
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("MCP server");
    let mut input = child.stdin.take().expect("stdin");
    let mut output = BufReader::new(child.stdout.take().expect("stdout"));
    writeln!(input,"{}",json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"synthetic-test","version":"1"}}})).expect("initialize");
    let mut line = String::new();
    output.read_line(&mut line).expect("response");
    let first: serde_json::Value = serde_json::from_str(&line).expect("only JSON RPC");
    assert_eq!(first["id"], 1);
    assert!(first.get("result").is_some());
    writeln!(
        input,
        "{}",
        json!({"jsonrpc":"2.0","method":"notifications/initialized"})
    )
    .expect("ready");
    writeln!(
        input,
        "{}",
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}})
    )
    .expect("tools");
    line.clear();
    output.read_line(&mut line).expect("response");
    let second: serde_json::Value = serde_json::from_str(&line).expect("only JSON RPC");
    assert_eq!(
        second["result"]["tools"].as_array().expect("tools").len(),
        11
    );
    drop(input);
    let result = child.wait_with_output().expect("wait");
    assert!(result.status.success());
    assert!(result.stderr.is_empty());
}
#[tokio::test(start_paused = true)]
async fn deadline_is_30_seconds() {
    let start = tokio::time::Instant::now();
    let result = brave_isole::mcp::bounded(async {
        tokio::time::sleep(std::time::Duration::from_secs(31)).await;
        Ok::<_, BridgeError>(())
    })
    .await;
    assert_eq!(result, Err(BridgeError::Timeout));
    assert_eq!(
        tokio::time::Instant::now() - start,
        std::time::Duration::from_secs(30)
    );
}
