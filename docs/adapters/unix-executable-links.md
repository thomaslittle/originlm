# Unix executable command links

Unix discovery can resolve a symlink at a registered executable candidate, such as
`~/.local/bin/codex` pointing to a versioned standalone executable. The command is
still a candidate from the existing discovery roots/command names; following a
link does not authenticate its publisher, account, plan or capabilities, or grant
permission to launch it. Windows executable/reparse-point policy is unchanged.

The executable-only resolver uses metadata and `read_link`, with at most 32 links,
512 component steps and 32,768 bytes per path. It resolves absolute and relative
targets, including parent components inside link targets. Loops, broken links,
over-budget paths, directories/special files and targets without an executable bit
are unavailable. A configuration directory can still yield a configuration-only
connection when no supported executable is available.

Native `LocatedClient` records retain the logical command and an optional private
`ExecutableIdentity`: resolved path, link bindings, device/inode/birth information,
size, modification/change times and mode. This evidence is not serialized into
public account or inventory records. Linked executable installation identities
include the binding so a changed target does not inherit a previous catalog.
Rust hosts constructing `LocatedClient` explicitly supply `executable_identity:
None` for existing trusted candidates, or retain discovery's captured identity.
Wire schemas and SDK method shapes do not change.

The service validates captured identity before metadata/session adapter work.
Codex validates the logical `codex` name before resolving a versioned target; a
direct arbitrary target filename does not become an accepted Codex command.
Managed Codex keeps the same logical name and binding. The common Unix process
launcher resolves the executable and rechecks its metadata before spawning the
resolved path, rather than asking a shell to interpret the command.

These are bounded path/identity checks, not atomic handle-based execution or a
sandbox against a hostile process of the same OS user. A target can still be
modified in the final check-to-exec interval. No executable signatures, security
settings, OS permissions or trust roots are changed.

Credential, configuration and log reads continue through the original strict
filesystem API, which rejects symlink components. Executable support never calls
that API to read linked credential/config contents and does not relax it.

## Evidence and limits

The focused fixture is `crates/originlm-kit/tests/unix_executable_symlink.rs`. It creates
an isolated home and synthetic Codex protocol peer, not a real installation or
account. Passive discovery must not launch it; a separately granted metadata
probe can. No prompt, turn, thread, paid API or live credential is used.

The baseline failed on Linux/WSL2 because the linked command was
`configuration_only`. The implementation's discovery-to-probe run passed on that
platform. The same fixture also covers managed-command identity, retargeting,
replacement, broken/looped/deep/non-executable/directory targets, and continued
rejection of linked data reads. The fixture checks executable ownership and bounded resolution
for the final-source result and bounds.

macOS execution is **unqualified** until actual macOS evidence is retained. A
Linux run does not qualify macOS, Windows symlinks, a real Codex account, or the
separate shared-helper stdio output path. This source change does not install a
binary, restart a shared service or publish a release.
