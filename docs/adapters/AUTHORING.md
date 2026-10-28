# Adding an adapter without changing SDKs or UI

Provider code belongs in Rust. Reuse the same three host extension points used by the built-in integrations: `InventorySource`, `MetadataSource`, and `ExecutionSource` on `NativeService`. The `originlm-kit` builder is the assembled default; its public service can be embedded independently. Never add account detection to React, Python, or a browser content script.

An adapter submission needs a descriptor, exact upstream source references/licenses, synthetic protocol fixtures, negative tests, and an operation/platform evidence report. `AdapterDescriptor` describes implementation coverage, not whether a particular user has access. Models and settings must come from the selected connection, with native IDs and revisions.

Inventory is passive: no commands, credentials, network, or inference. Enrichment requires a probe grant and returns the allowlisted `ConnectionMetadata` record. Do not include paths, tokens, raw configuration, arbitrary server errors, or private metadata blobs. Absence and stale data stay distinct from confirmed availability.

Execution implements `originlm_runtime::Agent` and runs through the shared owner-scoped `Runtime`. It must own only its processes/sessions, support cancellation and close, validate the selected identity/model/options, and never retry an ambiguous inference request or change the billing route silently. Artifacts are registered bytes, not an arbitrary path to be read later. Native built-in tools must not be advertised as toolkit-sandboxed when only the client's own policy applies.

Tests must cover unauthenticated status, expired/stale metadata, two accounts with the same model name, unknown/future settings, partial/truncated streams, error redaction, denied approvals, cancellation, and closing one client without interrupting another. Use `cargo test -p originlm-adapters --test YOUR_SUITE`, then `python scripts/adapter_conformance.py --all` and the repository verification profile.

The reference template at `crates/originlm-kit/examples/synthetic_adapter.rs` uses public contracts/service/runtime interfaces only. Its records are explicitly synthetic. A new adapter must not require SDK or selector source edits; transport/schema changes require protocol-version review and regenerated contracts.

No runtime downloads third-party executable plugins or invokes arbitrary code based on a remote manifest. Registration is performed by trusted host code. Credentials remain owned by the provider client or explicitly configured API route. Community/private endpoints require a separate opt-in security review and may remain unsupported.
