# Third-party notices and reuse ledger

## Bootstrap status

This repository currently contains original planning and project-guidance documents. No implementation code, provider logos, screenshots, model weights, or runtime binaries have been copied from reference repositories into this bootstrap.

The research references T3 Code, CodeNotch, CodeNotch cross-platform, and Prmptr. Reference is not a claim of affiliation, endorsement, inherited security guarantees, or blanket rights to use vendor services.

## Reference projects

| Project | Source | Intended reference |
| --- | --- | --- |
| T3 Code | https://github.com/pingdotgg/t3code | Native client integration, provider instances, dynamic model options, picker patterns. |
| CodeNotch cross-platform | https://github.com/thomaslittle/codenotch-crossplatform | Rust/platform discovery and usage parsing. |
| CodeNotch | https://github.com/vinzdg/codenotch | Broader provider/platform discovery. |
| Prmptr | https://github.com/thomaslittle/prmptr | Vertical provider rail and grouped model selection. |

See [SOURCE-AUDIT.md](SOURCE-AUDIT.md) for the previously inspected revisions and exact paths. Before porting each module, verify the license at the exact source revision and preserve required copyright and license text.

## Required ledger for every future reuse

Record the source URL, exact commit/blob, original path, copied or adapted symbols, destination path, license/SPDX identifier, copyright holder, retained notices, modifications, and applicable asset/model/dependency conditions. Add the required license text under a dedicated licenses directory when applicable. Do not mark this ledger complete merely because the root repository is MIT-licensed.

Provider names are used for identification. Logos and trademarks are not granted by this repository's license. Provider authentication, subscription access, and endpoint use remain separate integration-policy questions.
