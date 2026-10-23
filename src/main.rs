use brave_isole::{
    framing::write_native,
    ipc::validate_origin,
    protocol::{Envelope, Payload},
    session::token,
    verification::{self, ProofRole, VerificationServer},
};
use brave_isole::{
    ipc::BridgeConfig,
    mcp::BrowserServer,
    native_host::run_native,
    protocol::{BridgeError, Role},
};
use rmcp::ServiceExt;
use std::{env, path::PathBuf, process::ExitCode};
const BUNDLE_ID: &str = "dev.david.brave-isole";
fn root() -> Result<PathBuf, BridgeError> {
    let home = PathBuf::from(env::var_os("HOME").ok_or(BridgeError::Unavailable)?);
    let suffix = format!("Library/Containers/{BUNDLE_ID}/Data");
    Ok(if home.ends_with(&suffix) {
        home
    } else {
        home.join(suffix)
    }
    .join("b"))
}
fn run() -> Result<(), BridgeError> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args
        .first()
        .is_some_and(|arg| arg == "--diagnose-isolation")
    {
        let code = brave_isole::diagnostics::run(
            args.into_iter().skip(1).collect(),
            &["--diagnose-isolation"],
        )
        .map_err(|_| BridgeError::Unavailable)?;
        return if code == ExitCode::SUCCESS {
            Ok(())
        } else {
            Err(BridgeError::Unavailable)
        };
    }
    let root = root()?;
    match args.as_slice() {
        [mode, id] if mode == "--initialize" => {
            BridgeConfig::initialize(&root, id)?;
            Ok(())
        }
        [origin] if origin.starts_with("chrome-extension://") => native_entry(&root, origin),
        [mode, origin] if mode == "--native-host" => native_entry(&root, origin),
        [mode] if ["--verify-codex", "--verify-claude"].contains(&mode.as_str()) => {
            let role = if mode == "--verify-codex" {
                ProofRole::Codex
            } else {
                ProofRole::Claude
            };
            verification::verify_launch(&BridgeConfig::load(&root)?, role)?;
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .map_err(|_| BridgeError::Unavailable)?;
            runtime.block_on(async {
                let server = VerificationServer
                    .serve(rmcp::transport::stdio())
                    .await
                    .map_err(|_| BridgeError::Unavailable)?;
                server
                    .waiting()
                    .await
                    .map_err(|_| BridgeError::Unavailable)?;
                Ok(())
            })
        }
        [mode] if ["--mcp-codex", "--mcp-claude"].contains(&mode.as_str()) => {
            let config = BridgeConfig::load(&root);
            if config.as_ref().is_ok_and(verification::requested) {
                return Err(BridgeError::Unavailable);
            }
            let role = if mode == "--mcp-codex" {
                Role::Codex
            } else {
                Role::Claude
            };
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .map_err(|_| BridgeError::Unavailable)?;
            runtime.block_on(async {
                let server = BrowserServer::new(role, root)
                    .serve(rmcp::transport::stdio())
                    .await
                    .map_err(|_| BridgeError::Unavailable)?;
                server
                    .waiting()
                    .await
                    .map_err(|_| BridgeError::Unavailable)?;
                Ok(())
            })
        }
        _ => Err(BridgeError::InvalidRequest),
    }
}
fn native_entry(root: &std::path::Path, origin: &str) -> Result<(), BridgeError> {
    let config = BridgeConfig::load(root)?;
    validate_origin(origin, &config.extension_id)?;
    if verification::requested(&config) {
        let report = verification::verify_launch(&config, ProofRole::Native)?;
        return write_native(
            &mut std::io::stdout(),
            &Envelope {
                version: 1,
                expires_at_ms: None,
                epoch: token()?,
                id: token()?,
                payload: Payload::Verification {
                    passed: report.passed,
                },
            },
        );
    }
    run_native(std::io::stdin(), std::io::stdout(), config, origin)
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
