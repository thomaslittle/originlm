# OriginLM Python SDK (`originlm`)

> Alpha. Not yet published to PyPI; prepare a local platform wheel from source (see [quickstarts](../../docs/integration/quickstarts.md)).

```python
from originlm import Client
with Client.auto() as ai:
    print(ai.mode, ai.fallback_reason, ai.snapshot.get()['revision'])
```

Automatic startup uses a granted shared broker or the packaged private helper.
A protocol mismatch can select the private helper; authorization failure is fatal.

An async-first native-helper client with a synchronous facade. Python 3.11+, no runtime third-party dependencies. Imports do not inspect the filesystem, read accounts, start a process, or make network requests.

```python
from originlm import AsyncClient, AsyncStdioTransport

transport = await AsyncStdioTransport.open(
    executable="/absolute/path/to/originlm",
    args=["serve", "--stdio"],
)
async with await AsyncClient.connect(transport) as ai:
    snapshot = await ai.snapshot.get()
```

Synchronous callers can use `with Client.from_stdio(executable=..., args=...) as ai: ai.snapshot.get()`. Inside an existing event loop use `AsyncClient`, not the synchronous facade. The sync facade pumps its private event loop during calls; continuous high-volume event consumers should use the async client.

This client speaks OriginLM's public JSON-RPC protocol, not Codex/ACP directly. It needs the native `originlm` helper (see [native helper](../../docs/integration/native-helper.md)). Unit tests use synthetic native peers; the package smoke test runs the built wheel against the real helper with no provider inference.

Requests have bounded frames, correlation, deadlines, native error redaction, no automatic retries, and explicit owned-helper cleanup. `asyncio` cancellation cancels the local RPC wait, not necessarily a turn already accepted by the service. Use `turns.cancel` with its turn ID for native turn cancellation. Subscription queues are bounded and report `EVENT_GAP` rather than silently dropping output.

Only safe environment names are inherited by default. The caller explicitly supplies any additional environment entries. This is not a sandbox; the native helper must enforce grants, workspace/tool restrictions, billing, approvals, and cleanup of agent descendants. The SDK terminates only its own helper.

Domain types are generated from the Rust contracts (`originlm.generated`, plain `TypedDict`s). Responses stay dictionaries preserving the server's evidence and native values. There is no independent Python HTTP transport; use automatic startup, local IPC or stdio. No discovery/provider logic is duplicated in Python.

Tests: `python -m unittest discover -s packages/sdk-python/tests -v` from the repository root. All account values in fixtures are synthetic. The package is marked private and is not published to a registry.
