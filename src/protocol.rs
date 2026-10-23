use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_FRAME: usize = 1_048_576;
pub const DEADLINE_SECS: u64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Codex,
    Claude,
}
impl Role {
    pub fn label(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
        }
    }
}
pub type Epoch = String;
pub type RequestId = String;
pub type TabHandle = String;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Open {
        url: String,
    },
    List {},
    Close {
        tab: TabHandle,
    },
    Navigate {
        tab: TabHandle,
        url: String,
    },
    Snapshot {
        tab: TabHandle,
    },
    Screenshot {
        tab: TabHandle,
    },
    Click {
        tab: TabHandle,
        selector: String,
    },
    Fill {
        tab: TabHandle,
        selector: String,
        value: String,
    },
    Scroll {
        tab: TabHandle,
        x: i32,
        y: i32,
    },
    Wait {
        tab: TabHandle,
        selector: String,
    },
    Evaluate {
        tab: TabHandle,
        expression: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum BridgeError {
    #[error("invalid_request")]
    InvalidRequest,
    #[error("unauthorized")]
    Unauthorized,
    #[error("stale_session")]
    StaleSession,
    #[error("forbidden_url")]
    ForbiddenUrl,
    #[error("too_large")]
    TooLarge,
    #[error("timeout")]
    Timeout,
    #[error("disconnected")]
    Disconnected,
    #[error("unavailable")]
    Unavailable,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub ok: bool,
    pub result: Option<Value>,
    pub error: Option<BridgeError>,
}
impl Reply {
    pub fn success(result: Value) -> Self {
        Self {
            ok: true,
            result: Some(result),
            error: None,
        }
    }
    pub fn failure(error: BridgeError) -> Self {
        Self {
            ok: false,
            result: None,
            error: Some(error),
        }
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Secret(String);
impl From<String> for Secret {
    fn from(value: String) -> Self {
        Self(value)
    }
}
impl Secret {
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("[redacted]")
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Payload {
    Verification { passed: bool },
    Authenticate { role: Role, secret: Secret },
    Hello {},
    Command { role: Role, command: Command },
    Reply { reply: Reply },
    Stop {},
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at_ms: Option<u64>,
    pub version: u8,
    pub epoch: Epoch,
    pub id: RequestId,
    pub payload: Payload,
}
impl Envelope {
    pub fn validate(&self) -> Result<(), BridgeError> {
        if self.version != 1
            || self.epoch.is_empty()
            || self.epoch.len() > 128
            || self.id.is_empty()
            || self.id.len() > 128
        {
            return Err(BridgeError::InvalidRequest);
        }
        Ok(())
    }
}
