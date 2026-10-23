# Source installation on macOS

This is a developer preview with manual setup. Keep an existing working installation unchanged unless you intentionally plan to replace it. The app/container identity is shared across copies; two installations are not independent profiles.

## Requirements

- macOS, an existing Brave profile, Claude Desktop at `/Applications/Claude.app`, and Codex at `/Applications/Codex.app` or the older `/Applications/ChatGPT.app` name. This installer requires real launch evidence from **both** desktop clients; it cannot currently activate a Claude-only setup.
- Rust 1.88+ with Cargo, rustfmt and Clippy; Node.js 22+ for extension tests; Python 3.11+.
- Xcode Command Line Tools and `/usr/bin/codesign`. Signing is local and ad hoc; no paid Apple developer account is required by this procedure.
- Existing client configuration files: `~/.codex/config.toml` and `~/Library/Application Support/Claude/claude_desktop_config.json`. The installer reports missing configurations rather than creating an unrelated client setup.

The maintainer's local checks used Apple silicon and macOS 27.0.1. Compatibility with older systems and Intel Macs remains to be demonstrated. A bundle minimum-version field is not a compatibility certification.

## 1. Verify and build

From the repository root:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
node --test extension/tests/*.test.mjs
python3 -m unittest discover -s tests -p '*_test.py'
cargo build --bin brave-isole-probe
python3 scripts/prepare-release.py
python3 scripts/verify-isolation.py --candidate release --output work/isolation-release.json
```

`prepare-release.py` builds the native product, creates `work/BraveIsole.app`, applies the App Sandbox entitlement, verifies its signature and initializes its private container. It does **not** register the browser extension or enable account browsing. Keep the generated release metadata and private container secrets local.

The isolation result must be `PASS_CANDIDATE` for the exact executable. A failure or inconclusive control must be investigated; do not edit the report to force a pass. The check uses synthetic outside-file witnesses and temporary network controls. The diagnostic harness's private `sandbox_check` call is supplementary to observed denials.

## 2. Prepare diagnostic-only clients

```sh
python3 scripts/configure_clients.py --dry-run --stage bootstrap
python3 scripts/configure_clients.py --apply --stage bootstrap
```

Review the dry-run first. Applying installs the signed app at `~/Library/Application Support/BraveIsole/BraveIsole.app`, registers the exact native-host origin, and adds diagnostic-only MCP entries. The original client configurations are backed up privately. Existing unrelated settings are retained, and conflicting managed entries abort the operation.

Reload the MCP configuration through each client's supported settings/restart mechanism. Diagnostic MCP modes expose no browser tools. They record confinement booleans, the current build hash and nonce, and process-parent evidence. The temporary verification controls expire after 30 minutes; rerun the bootstrap stage if they expire before activation.

## 3. Load the Brave extension

Open `brave://extensions`, enable Developer mode and choose **Load unpacked**, selecting this repository's `extension/` folder. Review the permissions. The stable development extension ID is `cedaookbgnpnhdhbmhgnapbleebfpipj`.

The extension automatically starts the native host unless you paused it earlier. In verification mode, the host returns only the diagnostic result and does not relay browser commands. If needed, use **Connecter le pont local** in the popup to start that diagnostic launch. The popup distinguishes verification from a live connected bridge.

On macOS, Brave uses the per-user native messaging location under `~/Library/Application Support/Google/Chrome/NativeMessagingHosts`; the script uses that path and an exact `allowed_origins` entry.

## 4. Activate only after real-client proofs pass

```sh
python3 scripts/configure_clients.py --dry-run --stage activate
python3 scripts/configure_clients.py --apply --stage activate
```

Activation checks valid launch reports from Brave, Claude and Codex, matching the current nonce, extension ID and binary hash, with the expected application ancestry. A synthetic script launch is not accepted in place of a desktop client. If this refuses activation, keep it disabled and investigate the missing evidence.

The gate is a local installation check, not an attestation against a malicious process already running as your user. Parent paths are recognized for the app names listed above; custom renamed applications need a deliberate installer change.

After activation, reload the clients and reconnect/reload the extension. Pin the extension icon for easy access to Stop. Test on a non-sensitive page before using signed-in sites. The normal MCP configuration uses `--mcp-claude` and `--mcp-codex` on the signed installed executable, never `target/debug` or an unsigned binary.

## 5. Set the default route

Add the [assistant instruction](assistant-instructions.md) to each assistant's permanent user instructions. Disable competing Brave-control connectors such as an old AppleScript browser extension. The instruction tells the assistant to report an unavailable bridge instead of switching to keyboard/mouse control.

These are model instructions, not a system-wide block of every other assistant tool. Existing chats may need a fresh context. This project does not automatically edit remote account preferences or disable tools for unrelated software.

## Stop and uninstall

**Arrêter** revokes control immediately and remembers the pause across browser/worker restarts. **Connecter le pont local** resumes automatic startup connection. Old tabs remain open and are not reattached automatically.

```sh
python3 scripts/uninstall.py --dry-run
python3 scripts/uninstall.py --apply
```

Read the dry-run. The uninstaller restores only owned entries, preserves later user edits with conflict reporting and moves its installed app to a local backup. It does not purge browser profiles or cookies. Remove the Brave extension and the routing instruction separately if you no longer want them. No competing browser-control tool is re-enabled.

Never upload `work/`, the app container, client configuration, secrets or private receipts to a bug report.
