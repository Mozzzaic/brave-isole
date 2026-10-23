# Launch kit

These are draft posts for the author to publish from their own accounts. Do not claim independent security certification, zero risk, adoption numbers or compatibility beyond the documented developer preview.

## Short description

Brave Isolé is a free, MIT-licensed local MCP bridge for Claude Desktop and Codex on macOS. Assistants use your existing Brave sessions in their own background tabs. Signed Rust processes use macOS App Sandbox, native messaging and private Unix IPC, with no bridge telemetry or hosted relay.

Repository: https://github.com/Mozzzaic/brave-isole

## Show HN draft

**Title:** Show HN: Brave Isolé – background Brave tabs for Claude and Codex on macOS

I wanted assistants to work with my existing browser sessions while I kept using Brave. Keyboard automation kept interrupting what I was doing, so I built a local MCP bridge with assistant-owned background tabs.

Brave Isolé combines a Manifest V3 extension with signed Rust processes running under macOS App Sandbox. Transport uses native messaging, stdio and authenticated private Unix IPC. There is no bridge telemetry or hosted relay, and the tool API exposes typed browser operations rather than a host shell or arbitrary CDP methods.

This is an experimental source-only macOS release. Installation currently requires both Claude Desktop and Codex, local signing and verification. The sandbox covers the bridge processes, not the entire assistant; websites still use the network and content returned to cloud models can reach their providers. The security model and limitations are documented in the repository.

I'd welcome feedback on the installation flow, ownership boundaries and confinement tests.

https://github.com/Mozzzaic/brave-isole

## Product Hunt draft

**Name:** Brave Isolé

**Tagline:** Let AI assistants work in Brave while you keep browsing

**Description:** A free local MCP bridge for Claude Desktop and Codex on macOS. Keep your existing Brave sessions while assistants work in separate background tabs. Signed Rust processes use macOS App Sandbox, without bridge telemetry or a hosted relay. Experimental source installation; see the documented security boundaries and setup requirements.

**First comment:** I built this because browser automation kept taking over my screen. The goal is to let me keep working while an assistant uses its own tabs with my existing sessions. This developer preview is intended for people comfortable building and signing a macOS tool locally. I would especially appreciate reproducible bug reports and installation feedback. The repository explains both the protections and their limits.

## Where to submit

| Channel | Account / next step | Notes |
| --- | --- | --- |
| GitHub | Existing account; publish repository and source preview | Free public source repository; do not include private installation receipts. |
| mcp.tc | [Submit the public GitHub URL](https://mcp.tc/submit); no account needed | Free listing submission. A queued submission is not an accepted listing. Describe the native macOS source installation and its limits. |
| Awesome MCP Servers | GitHub pull request after meeting its current validation requirements | Its [validation workflow](https://github.com/punkpeye/awesome-mcp-servers/blob/main/.github/workflows/check-glama.yml) requests a Glama listing, score badge and working introspection in Docker. Native macOS compatibility needs clarification before submission; do not add a fake badge or bypass activation to satisfy a Linux check. |
| Hacker News | [Create an account](https://news.ycombinator.com/login), then [submit](https://news.ycombinator.com/submit) | Use the Show HN draft, make the runnable source easy to find and be available to discuss it. Follow the [Show HN rules](https://news.ycombinator.com/showhn.html). Do not solicit votes. |
| Product Hunt | [Create a free personal account](https://www.producthunt.com/launch/how-product-hunt-works), then prepare a launch | New accounts normally wait one week before posting. See [eligibility](https://help.producthunt.com/en/articles/481909-how-can-i-get-access-to-post) and [launch guide](https://www.producthunt.com/launch). No paid hunter is needed. Do not subscribe to anything merely to bypass the wait. |
| Glama MCP directory | Sign in with GitHub at [Glama](https://glama.ai/mcp), choose Add MCP Server and submit the public repository | [Submission FAQ](https://glama.ai/mcp/faq); [GitHub permission verification](https://glama.ai/mcp/methodology). Request a directory listing for a local macOS server, not a hosted relay. Automated checks are not a security certification. |

The official MCP Registry is a later step: publish a suitable distributable package first. This source-only, locally signed macOS preview is not a public remote endpoint. See the [registry quickstart](https://github.com/modelcontextprotocol/registry/blob/main/docs/modelcontextprotocol-io/quickstart.mdx).

## Claims to preserve

- Say “local transport, no bridge telemetry, macOS App Sandbox for the signed processes.”
- Say “existing Brave sessions, assistant-owned background tabs, role isolation.”
- State source-only preview, both-client activation requirement and powerful browser permissions.
- Explain that cloud assistants can receive returned page content; the bridge does not confine their other tools.
- Never promise malware-free software, absolute safety, full offline operation, notarization, or chat-by-chat isolation.
