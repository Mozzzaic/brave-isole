use crate::{
    ipc::{BridgeConfig, NativeBridge},
    protocol::{BridgeError, Command, DEADLINE_SECS, MAX_FRAME, Reply, Role},
};
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool, ToolAnnotations,
    },
    service::RequestContext,
};
use serde_json::{Map, Value, json};
use std::{future::Future, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::{Mutex, Semaphore};
const OPS: [(&str, &str, &[&str]); 11] = [
    ("browser_open", "open", &["url"]),
    ("browser_tabs", "list", &[]),
    ("browser_close", "close", &["tab"]),
    ("browser_navigate", "navigate", &["tab", "url"]),
    ("browser_snapshot", "snapshot", &["tab"]),
    ("browser_screenshot", "screenshot", &["tab"]),
    ("browser_click", "click", &["tab", "selector"]),
    ("browser_fill", "fill", &["tab", "selector", "value"]),
    ("browser_scroll", "scroll", &["tab", "x", "y"]),
    ("browser_wait", "wait", &["tab", "selector"]),
    ("browser_evaluate", "evaluate", &["tab", "expression"]),
];
pub fn tools() -> Vec<Tool> {
    OPS.iter().map(|(name,op,fields)|{
    let properties:Map<String,Value>=fields.iter().map(|field|((*field).into(),if ["x","y"].contains(field){json!({"type":"integer","minimum":-2147483648_i64,"maximum":2147483647_i64})}else{json!({"type":"string","maxLength":262144})})).collect();
    let mut tool=Tool::new(*name,format!("{op}: only this assistant's background Brave tabs. Use opaque tab handles. HTTP(S) only. JavaScript runs in the page, never on the Mac. No automatic retry of actions."),json!({"type":"object","properties":properties,"required":fields,"additionalProperties":false}).as_object().cloned().unwrap_or_default());
    tool.annotations=Some(ToolAnnotations::new().read_only(["list","snapshot","screenshot"].contains(op)).open_world(true));tool
}).collect()
}
pub fn parse_command(
    name: &str,
    mut arguments: Map<String, Value>,
) -> Result<Command, BridgeError> {
    if arguments.contains_key("op") {
        return Err(BridgeError::InvalidRequest);
    }
    let (_, op, _) = OPS
        .iter()
        .find(|(n, _, _)| *n == name)
        .ok_or(BridgeError::InvalidRequest)?;
    arguments.insert("op".into(), json!(op));
    if serde_json::to_vec(&arguments)
        .map_err(|_| BridgeError::InvalidRequest)?
        .len()
        > MAX_FRAME
    {
        return Err(BridgeError::TooLarge);
    }
    serde_json::from_value(Value::Object(arguments)).map_err(|_| BridgeError::InvalidRequest)
}
pub async fn bounded<F, T>(future: F) -> Result<T, BridgeError>
where
    F: Future<Output = Result<T, BridgeError>>,
{
    tokio::time::timeout(Duration::from_secs(DEADLINE_SECS), future)
        .await
        .map_err(|_| BridgeError::Timeout)?
}
pub struct BrowserServer {
    role: Role,
    root: PathBuf,
    connection: Mutex<Option<Arc<NativeBridge>>>,
    slots: Semaphore,
}
impl BrowserServer {
    pub fn new(role: Role, root: PathBuf) -> Self {
        Self {
            role,
            root,
            connection: Mutex::new(None),
            slots: Semaphore::new(32),
        }
    }
    async fn execute(&self, command: Command) -> Result<Reply, BridgeError> {
        let _permit = self
            .slots
            .try_acquire()
            .map_err(|_| BridgeError::Unavailable)?;
        let deadline = std::time::Instant::now() + Duration::from_secs(DEADLINE_SECS);
        bounded(async {
            let bridge = {
                let mut connection = self.connection.lock().await;
                if connection.is_none() {
                    let root = self.root.clone();
                    let role = self.role;
                    let bridge = tokio::task::spawn_blocking(move || {
                        let config = BridgeConfig::load(&root)?;
                        NativeBridge::connect(&config, role)
                    })
                    .await
                    .map_err(|_| BridgeError::Unavailable)??;
                    *connection = Some(Arc::new(bridge));
                }
                connection
                    .as_ref()
                    .cloned()
                    .ok_or(BridgeError::Unavailable)?
            };
            tokio::task::spawn_blocking(move || {
                bridge.dispatch_until(bridge.session(), command, deadline)
            })
            .await
            .map_err(|_| BridgeError::Unavailable)?
        })
        .await
    }
    pub async fn invoke(&self, name: &str, arguments: Map<String, Value>) -> CallToolResult {
        let result = match parse_command(name, arguments) {
            Ok(command) => self.execute(command).await,
            Err(error) => Err(error),
        };
        match result {
            Ok(reply) if reply.ok => {
                let value = reply.result.unwrap_or(Value::Null);
                if name == "browser_screenshot" {
                    if let (Some(data), Some(mime)) =
                        (value["data"].as_str(), value["mimeType"].as_str())
                    {
                        return CallToolResult::success(vec![ContentBlock::image(data, mime)]);
                    }
                }
                CallToolResult::success(vec![ContentBlock::text(value.to_string())])
            }
            other => {
                let error = match other {
                    Ok(reply) => reply.error.unwrap_or(BridgeError::Unavailable),
                    Err(error) => error,
                };
                if [
                    BridgeError::Disconnected,
                    BridgeError::StaleSession,
                    BridgeError::Timeout,
                ]
                .contains(&error)
                {
                    if let Ok(mut connection) = self.connection.try_lock() {
                        *connection = None;
                    }
                }
                CallToolResult::error(vec![ContentBlock::text(
                    json!({"error":error.to_string()}).to_string(),
                )])
            }
        }
    }
}
impl ServerHandler for BrowserServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build()).with_instructions("Use these tools for Brave in owned background tabs. Never access another assistant's or David's tabs. Respect the normal user approvals. Local bridge confinement does not stop page content being sent to the model. No raw CDP, host shell, cookies or file tools.")
    }
    fn get_tool(&self, name: &str) -> Option<Tool> {
        tools().into_iter().find(|tool| tool.name == name)
    }
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult {
            tools: tools(),
            ..Default::default()
        })
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        Ok(self
            .invoke(&request.name, request.arguments.unwrap_or_default())
            .await
            .into())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test(start_paused = true)]
    async fn timeout_does_not_wait_for_a_busy_connection_cache() {
        let server = BrowserServer::new(Role::Codex, PathBuf::from("/unused-test-path"));
        let _guard = server.connection.lock().await;
        let result = tokio::time::timeout(
            Duration::from_secs(31),
            server.invoke("browser_tabs", Map::new()),
        )
        .await;
        assert!(
            result.is_ok(),
            "timed-out call must not wait another connection lock"
        );
        assert_eq!(result.expect("result").is_error, Some(true));
    }
}
