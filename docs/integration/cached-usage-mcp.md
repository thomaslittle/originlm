# Cached usage through MCP

The optional SDK entrypoints expose an existing, explicitly approved usage cache. Importing them creates no client, connection, timer, process, discovery scan or inference request.

| Import | API |
| --- | --- |
| `originlm/mcp` | `createCachedUsageReader`, `createCachedUsageMcp`, usage reports and tool definition |
| `originlm/mcp/host` | `createCachedUsageProducer`, `createT3CachedUsageTool` |
| `originlm/mcp/remote` | `createCachedUsageRemoteMcp` |
| `originlm/mcp/node` | `serveCachedUsageMcp` over explicitly supplied streams |

Pass explicit connection and account scopes to the reader. It returns a bounded, sanitized projection with freshness evidence; unknown or unsupported windows stay unknown or unsupported. Account mappings must be operator supplied. Provider credentials, private display labels and evidence references are excluded.

A producer subscribes only after `attach()`; `stop()` clears its published projection. Host bindings verify the caller's environment and approved client session, and can be revoked independently. Retire the producer when the owner revokes access.

The remote handler mounts on an existing approved HTTPS host and uses its authentication callback. It never starts a listener. The authenticated principal must carry `kit.usage.cached.read`; browser origins are explicitly allowed, request sizes are bounded, and revoked bindings deny subsequent reads. Keep the existing tool name `kit_usage_cached` and scope for consumers that already use them.

See [MCP examples](../../examples/mcp/) for a synthetic local demonstration and optional host integrations. The SQLite example requires explicit owner approval and reads only the owner-designated cache; it must not bypass a running broker's ownership boundary.

Packages are private source previews. Build the SDK first with `npm run build --workspace originlm`; registry installation becomes available only after publication.
