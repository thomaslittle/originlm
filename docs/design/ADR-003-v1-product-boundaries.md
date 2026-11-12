# ADR-003: OriginLM product name and privacy boundary

- Status: accepted
- Date: 2026-10-28

## Decision

**OriginLM** names the product, dashboard, desktop app and website. **Local AI
Kit** names the implementation and integration surface. Existing package names
(`originlm`, `originlm`), Rust crate names (`originlm-*`), executable
names, socket paths and environment variables remain stable. Renaming those
would break applications and grants without improving the product.

The two product paths are peers: **See your AI** and **Build with it**. Every
public capability claim must follow the operation-specific support matrix.
Installation presence does not prove usable models, quotas or authentication.

The privacy promise is a local control plane with no OriginLM telemetry or
hosted relay. History and grants stay local. Explicitly authorized provider
operations may contact the selected provider; prompts, attachments and tool
results can leave the machine as part of that operation. Local model routes
can operate locally. Credentials are never returned to consuming apps.

Do not say “nothing leaves the machine” without limiting the statement to the
local control plane. Some granted metadata adapters access provider-owned
credentials in memory; do not claim that only the native CLI ever reads them.
The security policy and adapter evidence describe those boundaries precisely.

### Provider CLI updates (2026-11-12)

OriginLM can update provider CLI binaries on the host
(`originlm providers update`). This is the first operation that mutates the
host outside OriginLM's own state, so its boundary is recorded here:

- It is explicit and local-operator-only: a CLI command, with `--dry-run`. It
  never runs automatically, in the background, or on discovery or refresh.
- No broker method, SDK call or MCP tool exposes it, so a connected
  application cannot reach host mutation. The normal application token is not
  widened. A desktop control-center surface, if added, must be
  administrator-gated and explicitly granted.
- It runs only an installer proven to own the binary by its real path.
  Manual-only installs show a command and never run it. Success requires the
  re-probed version to be current.
- `originlm providers check` contacts package registries and release channels
  only when the operator runs it. Imports, scans and broker startup still make
  no network request.

This does not change the self-update path (`originlm-update`), which keeps its
Ed25519 verification. Third-party CLIs are not signed by OriginLM, and
OriginLM does not claim to verify their publisher.

## Consequences

Human-facing headings and navigation use OriginLM. Code snippets and commands
keep their actual names. Historical qualification records retain their wording
and source SHAs; they are evidence, not current marketing copy. Publication,
signing and hosting remain separate release operations.
