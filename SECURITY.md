# Security model

Brave Isolé is an experimental browser-control bridge, not an antivirus, a malware-free certification, or a sandbox for an entire AI assistant. This document describes concrete boundaries and their limits.

## Shipped design

- A Manifest V3 extension exposes a fixed set of browser commands. The assistant cannot send arbitrary CDP methods through the public tool API.
- The extension creates tabs for the authenticated Claude or Codex role. It checks ownership and HTTP(S) navigation before operations. Old session handles are revoked on stop or a new epoch. Isolation is by role, not by chat.
- The native executable is packaged and ad hoc signed with `com.apple.security.app-sandbox`. The same signed identity permits private Unix IPC between separately launched native/MCP processes. Running an unsigned or bare development binary does not provide this boundary.
- The production bridge uses stdio, Chromium native messaging and private Unix IPC with per-role secrets. Its API has no shell or general host-file endpoint. It does not create an Internet or loopback TCP control listener, send telemetry, or contact a hosted relay.
- The extension accepts the configured native host and exact extension ID. Its packaged public key fixes an unpacked development ID; it is public data, not a signing credential or protection against a malicious same-user installation.
- Stop revokes ownership and suppresses late responses. Requests have deadlines and mutations are not automatically replayed. An action already performed on a website cannot be undone by Stop.

## What the sandbox does not promise

The locally signed build was tested with synthetic outside-file witnesses, active TCP/UDP IPv4/IPv6 endpoints, positive private-container controls and IPC between confined peers. The release procedure also requires diagnostic launches from Brave, Claude and Codex for the exact binary hash before browser commands are enabled. Crashes, connection refusals and missing controls are not accepted as a sandbox pass.

These checks are evidence on the tested machine. They are not an independent audit or a proof that every file is unreadable. macOS may permit ordinary public system files. Development and verification tools run outside the product sandbox; temporary test listeners are not part of the production runtime.

The browser extension is powerful. An assistant can open authenticated sites in its own tabs and take actions with that account. Page JavaScript can read DOM and script-accessible browser storage, and make web requests. There is no dedicated cookie-export tool; this does not mean page scripts have no access to script-readable cookies or storage. The browser itself still uses its session cookies normally.

Text and screenshots given to cloud assistants may reach their providers. Other assistant tools retain their own permissions. This bridge cannot enforce an assistant's approval policy, prevent every prompt injection, undo a site action, or guarantee that a site never opens a focus-taking popup.

A malicious process already running as the same OS user may read local secrets, interfere with configuration, deny service or impersonate a local client. This is outside the claimed boundary. OS/browser vulnerabilities, dependency compromise, unsafe user-installed extensions and unknown flaws remain possible.

## Installation and permissions

Build locally and read the scripts before running them. Installation edits only the managed MCP entries and native-host manifest, with private backups and conflict checks. The extension requests native messaging, debugger, tabs, tab groups and local storage. Local storage remembers the automatic-connection preference, not page data. There is no wildcard host-permission grant.

Do not bypass the verification gate or point your assistant at an unconfined build. Ad hoc signing is not Apple notarization or third-party endorsement. Stop the bridge before rebuilding or changing its identity. The current installer expects both supported desktop clients in /Applications and does not silently enable a partial installation.

## Report a vulnerability

Use the repository's **Security > Advisories > Report a vulnerability** form when private reporting is enabled. If that form is unavailable, open an issue asking only for a private contact method; do not include exploit details, secrets or private account data publicly. There is no guaranteed response time or paid bug-bounty program.

Only the latest developer preview is supported. Share a minimal synthetic reproduction, affected version and platform. Never attach cookies, session tokens, personal configuration backups, verification receipts or account screenshots.
