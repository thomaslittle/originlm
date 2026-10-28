# Local metadata change notifications

`originlm_platform::watcher::MetadataWatcher` is an opt-in Rust primitive for host applications. Construct it only after the host has obtained permission to observe the selected local profile directories. Imports and ordinary passive discovery do not construct a watcher.

The watcher uses OS notifications without recursively traversing profiles. Only explicit known metadata filenames produce notifications; unrelated session logs are ignored. Callback payloads contain the host's opaque connection IDs, not paths or file contents. A callback means cached evidence should be invalidated—not that authentication, plan, or quota has been verified. It must never trigger inference.

The watcher rejects missing, relative, and symlink roots; dropping it disables subsequent callbacks. Windows tests cover watched-file modification/deletion, ignored logs, lifecycle shutdown, and invalid roots. Backend errors invalidate observed IDs rather than fabricating a healthy state. Native OS events are not a guarantee on remote/network filesystems.

`NativeService` now exposes explicit `discovery.watch({ enabled: true|false })` for already-authorized clients. Enabling it requires passive-discovery permission and a completed inventory scan. A filesystem notification invalidates only cached metadata for the affected connection IDs; it does **not** launch a provider, refresh authentication, probe quotas, or run inference. Disabling the watch, revoking grants, closing the client, or dropping the host removes that client's watcher without affecting other applications.

Implementation source: `crates/originlm-platform/src/watcher.rs`. Tests: `crates/originlm-platform/tests/watcher.rs`. Upstream interface: https://docs.rs/notify/8.2.0/notify/.
