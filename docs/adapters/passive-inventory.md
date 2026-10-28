# Native passive inventory

An explicit `Scanner::scan` probes allowlisted client/configuration locations and bounded directory entries. It never reads file content, unlocks a credential store, launches clients, or contacts a provider. The injected `LocalFilesystem` boundary allows tests to fail immediately on accidental file reads.

Initial pattern registry: Codex, Claude Code, Gemini CLI, Antigravity, Grok, OpenCode, Cursor, Ollama, LM Studio. These are candidate location patterns, not certification of authenticated access. Provider directories and command names are based on the project source audit; installed-version authentication/execution remains separate work.

Windows shims are detected but not run. Symlink executables are reported as not followed; explicit approved shim/symlink resolution remains pending. Relative/network roots are rejected. Known home overrides and XDG locations are respected, including `GEMINI_CLI_HOME`, `GROK_HOME`, and `LM_STUDIO_HOME`. Blank overrides are unset. On Windows, only an absolute native `HOME` redirects the profile. Relative discovery literals are stored with native separators. Personal/work Codex/Claude directories have independent stable identities; no account or shared allowance is inferred. OpenCode's configuration and data roots remain separate candidates until later authenticated identity linking. GitHub Copilot is a `~/.copilot` profile candidate and does not open its session database. Pi is a `PI_CODING_AGENT_DIR` or `~/.pi/agent` candidate and does not open `sessions/`. OpenCode's installer directory `~/.opencode/bin` is searched for `opencode` when that directory is not on `PATH`. Antigravity CLI (`antigravity-cli`) is a second profile under the Gemini home; the extension directory stays under `~/.gemini/antigravity` when `GEMINI_CLI_HOME` is set.

Tokscale is a pinned upstream source for these locations, not a runtime dependency. See [Tokscale upstream discovery](../upstream/tokscale.md).

Public snapshots expose normalized hints and opaque path-derived IDs, not full private paths. Actual executable/profile paths remain in the native `ScanResult.locations` map for later authorized adapters. Snapshot models, authenticated accounts, plans, and quotas remain empty/unknown until explicitly enriched.

Limits: at most 128 home entries and PATH entries, 16 profile roots and 8 executable candidates per provider. Partial enumeration is explicitly diagnosed. Progress publishes one partial snapshot per provider; cancellation is checked between metadata probes. A filesystem metadata call can still block at the OS level; this synchronous library is not a deadline-enforced sandbox. Native service integration must keep it off request/UI threads.

No automatic WSL/container/network inventory, no claims of inactive subscription detection, and no credentials are returned.

Generic executables named `agent` are not attributed to Cursor: other clients, including Grok, ship that name. A Cursor configuration directory is insufficient evidence of executable ownership. The explicit `cursor-agent` command remains a passive candidate; ambiguous wrappers require a separately verified provider integration.
