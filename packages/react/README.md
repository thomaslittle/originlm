# @originlm/react

> Alpha. Not yet published to npm.

Optional React bindings for the public OriginLM SDK. Provider discovery and execution remain in Rust; importing or mounting these components makes no requests.

Create a `SnapshotStore` from an explicitly connected public SDK client. Pass it to `LocalAIProvider`. `useLocalAISnapshot` subscribes through `useSyncExternalStore`; `useLocalAIStore` exposes explicit `load`, `scan`, and `probe` operations. The store deduplicates snapshot reads and owns immutable, stable snapshots. Hooks support SSR with a consistent initial snapshot.

`useModelCatalog`, `useModelCandidates`, and `useSelectionReview` wrap the framework-neutral selector logic. Selection reconciliation is advisory, not permission to silently change billing or effort settings.

The embedding app owns the client/store lifecycle: call `store.dispose()` when the owning application closes or changes accounts, then `client.close()` if it owns the connection. Unmounting a subscriber does not close another consumer's client. See `examples/react` and `packages/ui-react`.

React 18.3/19 is a peer dependency; discovery and SDK users need not install it. No credential is stored by these bindings.
