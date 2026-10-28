# Generated domain and API contracts

`originlm-contracts` is the authoring source for domain records and public method input/output maps. The service uses these canonical selection and session request records. The type generator performs no I/O at import; its explicit command asks the Rust schema example for current contracts and deterministically creates:

- `protocol/contracts.schema.json`: JSON Schema for the public domain and RPC maps.
- `protocol/methods.json`: method names and request/result schemas; negotiate actual availability with `runtime.handshake`.
- `protocol/openapi.json`: the implemented local HTTP routes, JSON-RPC requests, pairing, REST reads, and event stream.
- TypeScript exported domain types and inferred method return/argument types.
- Python TypedDict domain types and client type stubs.

Run `python scripts/generate_contracts.py` after changing canonical contracts. `python scripts/check_generated.py` compares without overwriting and fails on drift. The generator rejects external references and unsafe definition names; it does not download schemas. `python -m unittest discover -s tests/generation` tests the generator and documentation model. `node node_modules/typescript/bin/tsc -p tests/generation/tsconfig.json` verifies a real consumer, including expected compile-time failures.

## TypeScript

```ts
import type { Model, Snapshot, Session } from 'originlm';
// `ai` is returned by createLocalAI with an explicitly created transport.
const snapshot: Snapshot = await ai.snapshot.get();
const models: Model[] = await ai.models.list();
const partial = await ai.snapshot.get({ fields: ['connections', 'revision'] });
// `partial.models` is a type error: the projection did not request models.
```

The SDK retains an explicit generic override for integrations with extension-specific responses. Dynamic diagnostic payloads intentionally remain `unknown`, not guessed structured objects. Runtime validation and grants still apply; static types do not authorize a request.

## Python

```python
from originlm.generated import Snapshot, Model
snapshot: Snapshot = client.snapshot.get()
models: list[Model] = client.models.list()
```

Generated records describe wire objects, not classes that run discovery or hold credentials. Integer counts and decimal monetary values retain their wire string forms. Provider-native choices remain strings supplied by the current connection, not a fixed model/effort enum. Projection requests can omit fields at runtime; code consuming a projection must handle only requested keys.

The OpenAPI document describes *compiled protocol shapes*, not a guarantee of an installed provider, active subscription, or live-qualified operation. Account-specific metadata comes from the paired runtime. Unsupported methods return a typed error, never a successful empty placeholder.
