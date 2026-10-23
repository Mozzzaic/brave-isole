//! A browser bridge whose native processes run under macOS App Sandbox.
#[path = "../probe/src/checks.rs"]
pub mod diagnostics;
pub mod framing;
pub mod ipc;
pub mod mcp;
pub mod native_host;
pub mod protocol;
pub mod session;
pub mod verification;
