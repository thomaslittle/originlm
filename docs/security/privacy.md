# Privacy

OriginLM has no hosted account service or cloud relay. Imports perform no scanning,
credential reads, process starts, downloads or inference. Explicit startup/discovery
can inspect locally configured clients under its granted policy.

Provider credentials remain with their native clients or explicitly configured
credential sources. They are not returned in snapshots or exported as API keys.
Local state can include inventory/evidence, grants, provider metadata and history;
applications receive only their permitted projection. Price-table storage contains
rates, model IDs, dates and source links, not prompts or credentials. Cost observations
are bounded and retained in the runtime's memory for its lifetime.

“Local” describes the control plane, not every provider operation. Authorized cloud
operations can send prompts, tool/workspace content and metadata to the provider.
The provider's terms, retention and billing apply. Local-model routes can run locally.
OriginLM does not promise to sandbox every native client's hooks or extensions.

Example mode uses synthetic values and does not read or execute real accounts.
Diagnostics are intended to contain redacted contract evidence, not secret values;
review a report before sharing it. No telemetry endpoint is configured by this project.

Uninstall keeps user data unless purge is explicitly requested. Provider clients and
their own data are outside OriginLM's uninstall scope. See the
[threat model](threat-model.md) for same-user process/filesystem limitations and
[service removal](../integration/service.md) for the implemented scripts.
