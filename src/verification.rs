//! Launch-path checks use synthetic witnesses only and expose no browser tools.
use crate::{diagnostics, ipc::BridgeConfig, protocol::BridgeError};
use rmcp::{ServerHandler, model::ServerConfig};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    os::unix::{
        fs::OpenOptionsExt,
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProofRole {
    Native,
    Codex,
    Claude,
}
impl ProofRole {
    pub fn label(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Codex => "codex",
            Self::Claude => "claude",
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VerificationInput {
    nonce: String,
    witness: PathBuf,
    binary_sha256: String,
    extension_id: String,
    tcp4: String,
    tcp6: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchProof {
    pub passed: bool,
    pub role: ProofRole,
    pub nonce: String,
    pub binary_sha256: String,
    pub extension_id: String,
    pub pid: u32,
    pub parent_pid: u32,
    pub checks: BTreeMap<String, bool>,
}
pub fn requested(config: &BridgeConfig) -> bool {
    config.root.join("verification.json").exists()
}
fn denied(result: std::io::Result<()>) -> bool {
    result.is_err_and(|error| matches!(error.raw_os_error(), Some(1 | 13)))
}
pub fn verify_launch(config: &BridgeConfig, role: ProofRole) -> Result<LaunchProof, BridgeError> {
    let bytes =
        fs::read(config.root.join("verification.json")).map_err(|_| BridgeError::Unavailable)?;
    if bytes.len() > 16384 {
        return Err(BridgeError::InvalidRequest);
    }
    let input: VerificationInput =
        serde_json::from_slice(&bytes).map_err(|_| BridgeError::InvalidRequest)?;
    if input.nonce.len() != 64
        || input.extension_id != config.extension_id
        || !input.witness.is_absolute()
    {
        return Err(BridgeError::InvalidRequest);
    }
    let executable = fs::read(std::env::current_exe().map_err(|_| BridgeError::Unavailable)?)
        .map_err(|_| BridgeError::Unavailable)?;
    let binary_sha256 = Sha256::digest(executable)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let mut checks = BTreeMap::new();
    let witness = input.witness.to_str().ok_or(BridgeError::InvalidRequest)?;
    checks.insert(
        "read_denied".into(),
        denied(diagnostics::check("read", witness)),
    );
    checks.insert(
        "write_denied".into(),
        denied(diagnostics::check("write", witness)),
    );
    for (key, op, target) in [
        ("tcp4_denied", "tcp", &input.tcp4),
        ("tcp6_denied", "tcp", &input.tcp6),
        ("udp4_denied", "udp", &input.tcp4),
        ("udp6_denied", "udp", &input.tcp6),
    ] {
        checks.insert(key.into(), denied(diagnostics::check(op, target)));
    }
    let marker = config
        .root
        .join(format!("v-{}-{}", role.label(), std::process::id()));
    let container = (|| {
        fs::write(&marker, b"synthetic-verification")?;
        let ok = fs::read(&marker)? == b"synthetic-verification";
        fs::remove_file(&marker)?;
        Ok::<_, std::io::Error>(ok)
    })();
    checks.insert("container".into(), container.unwrap_or(false));
    let endpoint = config.root.join(format!("p-{}", std::process::id()));
    let ipc = (|| {
        let server = UnixListener::bind(&endpoint)?;
        let client = UnixStream::connect(&endpoint)?;
        drop(client);
        drop(server);
        fs::remove_file(&endpoint)?;
        Ok::<_, std::io::Error>(())
    })();
    checks.insert("private_ipc".into(), ipc.is_ok());
    let report = LaunchProof {
        passed: checks.values().all(|ok| *ok) && binary_sha256 == input.binary_sha256,
        role,
        nonce: input.nonce,
        binary_sha256,
        extension_id: config.extension_id.clone(),
        pid: std::process::id(),
        parent_pid: nix::unistd::getppid().as_raw() as u32,
        checks,
    };
    let target = config
        .root
        .join(format!("proof-{}-{}.json", role.label(), report.pid));
    let temporary = config
        .root
        .join(format!("report-{}.tmp", std::process::id()));
    let data = serde_json::to_vec(&report).map_err(|_| BridgeError::Unavailable)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|_| BridgeError::Unavailable)?;
    file.write_all(&data)
        .map_err(|_| BridgeError::Unavailable)?;
    drop(file);
    fs::rename(temporary, target).map_err(|_| BridgeError::Unavailable)?;
    Ok(report)
}
pub struct VerificationServer;
impl ServerHandler for VerificationServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::default().with_instructions("Local launch verification only. No browser commands are exposed. Activation awaits local checks for Brave and both desktop clients.")
    }
}
