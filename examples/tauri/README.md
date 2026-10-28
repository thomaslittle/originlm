# Tauri host integration

Keep the native `originlm-kit` dependency in the Tauri Rust host, not in the renderer. The renderer imports the optional TypeScript/React selector packages. A host-created `EmbeddedClient` can be stored in Tauri managed state and exposed through a narrow command that returns `Snapshot`; it must not accept arbitrary `ClientPolicy`, executable paths, grant scopes, or raw method names from JavaScript.

```rust
// Run in the host's existing asynchronous runtime after user approval.
let kit = originlm_kit::Kit::builder().build()?;
let client = kit.connect("tauri-app", originlm_kit::ClientPolicy::passive())?;
let snapshot = client.scan().await?;
// Serialize only snapshot back to the renderer.
client.close();
```

Use `crates/originlm-kit/examples/inspect.rs` for the compiled/tested Rust call pattern. This guide does not add Tauri as a dependency of the core and is not a separately compiled Tauri application. Metadata and execution require separate host decisions; opening a window must not scan credentials or run an agent. On app shutdown, close its embedded client without terminating a broker used by other apps.
