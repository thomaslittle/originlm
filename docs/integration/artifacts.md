# Owned result artifacts

Every successfully completed text turn exposes a `response.txt` artifact through its final event stream. An adapter may also publish binary results through `TurnContext::artifact`; arbitrary artifact events are rejected. No input accepts a filesystem path.

Read the event’s artifact ID with `artifacts.read({sessionId, artifactId, offset: "0", maxBytes: 49152})`. Offsets, total size, and next offset are decimal strings; `dataBase64` contains the requested bytes. Stop when `eof` is true. The same interface works over stdio, HTTP, the native broker, and browser Native Messaging.

Results are memory-only, session/owner-scoped, expire after 15 minutes, and are discarded on close/revocation. Limits: 4 MiB per result, 32 MiB per runtime, 128 retained results, and 48 KiB per read before base64 encoding. Large or unmanageable output fails explicitly rather than silently truncating a successful response. Applications must save a result explicitly before closing a session. Downloads must treat media types as untrusted and must not execute returned HTML/script content.

The runtime stores only output submitted by its own active agent. It never reads the native client’s arbitrary output paths or exposes a generic file-read service.
