# Read the OriginLM dashboard

The overview puts provider evidence, allowances and observed costs together.
An installed client is a discovery signal, not proof of an active account.

- **Authenticated now** counts fresh successful authentication evidence. Previously
  authenticated profiles are separate; expired sign-in can need attention.
- **Usage and allowances** show each provider pool/window once. A ring represents
  used allowance, not a universal balance. Unknown totals have no invented percent.
  Shared accounts can point to the same pool; their allowances are not added.
- **Reset times** count down only when a timestamp was reported. “Reset due” means
  the timestamp passed; it does not claim a successful refresh or a full allowance.
- **Models** are scoped to routes and retain provider-native identifiers and options.
  A catalog entry does not itself authorize execution.
- **Cost trend** covers observed turns in this runtime lifetime (up to 256 retained
  observations), not all account spending. A supported provider's reported USD cost
  takes precedence. Otherwise both input/output token counts and an exact matching
  price row are required. Those values are always labeled **estimated**. Missing
  inputs stay unknown. Subscription allowance is not converted into an invented bill.
- **Problems** show reported issues and sign-in attention. Missing issue evidence
  means unknown, not “everything is healthy.”

Settings contains the local price table: exact provider/model IDs, input and output
USD per million tokens, review date and source. Only an administrator can edit stored
prices. Invalid stored prices disable estimates and surface an error; they do not
silently reset to defaults. A runtime without configured storage labels changes as
in-memory. Example-mode edits affect only synthetic examples.

Connections shows application grants; revocation is explicit. Playground execution
requires its own grants and a selected route. Selector Lab demonstrates all eight
selector variants. Diagnostics reports the public handshake and permission evidence.

The GPUI app has an optional **in-app** allowance panel (Ctrl+Shift+M), including
unknown and stale readings. There is no OS tray/menu-bar integration in this build.
A web status badge distinguishes a reachable runtime from an authenticated client.

Example mode is always labeled and uses synthetic data. It never reads a real
account, invokes a provider or charges an API. To see real data, connect to a runtime
and authorize only the metadata operations you want. Local control state stays on
the machine; authorized cloud-provider operations can send requests to that provider.
