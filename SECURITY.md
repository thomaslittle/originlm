# Security policy

OriginLM discovers local AI clients and brokers access to them, so security reports are taken seriously.

## Reporting a vulnerability

**Please do not open a public issue for a security problem.** Report it privately through GitHub:

1. Go to the repository's **Security** tab.
2. Choose **Report a vulnerability** (private vulnerability reporting).

Include a minimal, redacted reproduction, the affected version or commit, the platform, and what an attacker gains. Never include real tokens, account credentials, private prompts, session logs, or personal paths. If you are unsure whether something is a vulnerability, report it privately anyway.

You can expect an acknowledgement within a few days. This is a small, alpha-stage project maintained by volunteers, so fix timelines depend on severity; we will keep you informed and credit you in the advisory unless you prefer otherwise.

## Supported versions

Only the latest `0.1.0-alpha.N` release and `main` receive fixes. There are no long-term-support branches yet.

## Scope

In scope: the native helper and Rust crates, the stdio/HTTP/broker/Native Messaging transports, pairing and grant enforcement, credential handling and redaction, process ownership and cancellation, path/symlink containment, the TypeScript and Python SDKs, and the build/release scripts.

Examples of what we consider vulnerabilities: a way to execute or consume an account without the required grant; credential or token disclosure in events, logs, URLs, snapshots, or UI; a cross-origin or cross-application path to the local transports; escaping a granted workspace; killing or reading processes the toolkit does not own.

Out of scope: vulnerabilities in the third-party AI clients and providers themselves (report those upstream), social engineering, and issues that require an attacker who already controls the user's account or the host application's trusted code.

## Security design

The properties below are what the code is built and tested to uphold; a violation is a security bug.

- Importing a package is side-effect free. Passive discovery never runs inference, launches agents, contacts providers, or reads secrets.
- Grants are revocable and per-application/per-connection. Permission to *detect* is not permission to *execute* or to incur paid usage.
- Native sign-ins stay with their owning clients. Provider credentials never appear in snapshots, events, UI, logs, URLs, or sample code. Subscription tokens are never exported as API keys.
- Local transports are authenticated, loopback- or owner-scoped, and enforce scopes on every path including streams and artifacts. CORS is not treated as authentication.
- Workspaces and operations are scoped; approval denial and cancellation are enforced; only processes the toolkit started are terminated.
- Protocol frames, provider responses, symlink containment, redirects, and credential-to-endpoint binding are validated.
- Metadata mechanisms that rely on undocumented behavior are classified, reviewed, and opt-in.

See the [threat model](docs/security/threat-model.md) for the full analysis and the [architecture specification](docs/design/architecture.md) for the intended boundaries.

## Notes on test material

`crates/originlm-chatgpt-auth/tests/fixtures/test-rsa.pem` is a throwaway RSA key generated only to sign synthetic tokens in unit tests. It protects nothing and is not used by any shipped code. Likewise, every account, token, and session value in fixtures is synthetic. Secret scanners may flag the PEM; that finding is expected.

## Qualification status

Security-relevant claims are recorded with their evidence level (fixture-tested, platform-executed, live-qualified) in the [support matrix](docs/adapters/SUPPORT-MATRIX.md). A property that is only fixture-tested is not described as live-verified.
