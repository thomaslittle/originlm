# Embedded Rust entry point

`originlm-kit` assembles the same adapters and native service used by the CLI. Depend on this crate for a one-import Rust integration; depend on `originlm-discovery` alone for a smaller detection-only library. No React, Tauri, Node, or Python is required.

```rust
use originlm_kit::{Kit, ClientPolicy};
# async fn example() -> originlm_kit::Result<()> {
let kit = Kit::builder().build()?;
let app = kit.connect("my-application", ClientPolicy::passive())?;
let snapshot = app.scan().await?;
println!("{} connections", snapshot.connections.len());
app.close();
# Ok(()) }
```

Building the kit and reading an empty snapshot do not scan. Client policies are set only by trusted embedding-host code; do not expose their construction as an untrusted renderer RPC. `ClientPolicy::denied()` grants no discovery. Metadata needs provider/connection probe grants. `.execution(true)` registers drivers but does not authorize a turn; the host must separately call `authorize_execution` with exact providers, workspaces, and billing permissions.

Embedded connections use unique internal identities even if two apps choose the same label. Closing/dropping one releases its permissions and sessions without closing another. The service and adapter factories are shared with the CLI, not independently reimplemented in the example.

Run `cargo run -p originlm-kit --example inspect -- --home ABSOLUTE_DIRECTORY` for an explicit passive inventory. A nonexistent or empty directory does not fall back to the real user's account data.
