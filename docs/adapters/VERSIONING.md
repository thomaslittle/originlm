# Compatibility and versioning

Protocol major 1 is negotiated before any request that can act on local state. Unknown/unadvertised operations return typed errors. Generated TypeScript/Python types and HTTP/RPC schemas derive from Rust, and `check_generated.py` fails on drift. Adding a method does not imply that every runtime or provider implements it.

Native model identifiers, route IDs, and supported option values must not be rewritten into application-specific enums. A changed model descriptor invalidates old selections. Future enum-like strings remain inspectable where the provider permits them, while unrecognized controls remain non-editable.

Provider support has separate axes: code implemented, synthetic contract-tested, operating-system executed, installed-client metadata checked, and live inference checked. Never collapse those into one green badge. An installed executable or a cached subscription label is not a live entitlement check.

This repository is an unpublished alpha. Registry names are not cleared or published. Release archives are checksummed, not signed. The source commit and dirty-worktree flag are in every manifest. Cross-compiling or authoring a CI job does not prove that an OS runner executed successfully.
