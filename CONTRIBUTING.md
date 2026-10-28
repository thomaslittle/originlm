# Contributing to OriginLM

Thanks for helping. This project is alpha, so small, well-tested, well-scoped changes are the easiest to review and land. By participating you agree to the [Code of Conduct](CODE_OF_CONDUCT.md). Security problems go through [SECURITY.md](SECURITY.md), not public issues.

## Ground rules

These come from the product's trust model; changes that weaken them will not be accepted.

- **Rust is canonical.** Detection and provider behavior live in the Rust crates. The SDKs are thin generated-type clients; React, Tauri, and the browser extension are optional consumers, never core requirements.
- **Importing does nothing.** Imports and constructors do no scanning, credential reading, spawning, network calls, downloads, or inference. Discovery is staged and explicitly permitted.
- **Evidence, not guesses.** Preserve provider/installation/profile/account/connection/route/model/billing/pool identity and field-level evidence. *Unknown* and *unsupported* are different. Do not invent access, quotas, prices, capabilities, or effort settings, and keep provider-native model IDs and option descriptors.
- **A signal is not permission.** Scope grants for workspaces, tools, artifacts, and processes. Cancel only work the toolkit owns.
- **Credentials stay with their owner.** Never export subscription tokens as API keys, read browser cookies, enable paid fallback silently, rotate accounts to evade limits, or race a client's own credential refresh.
- **No real account data.** Tests use virtual homes, synthetic credentials, fake clients, and mock servers. Never commit real tokens, account details, session logs, or personal paths. Never charge or consume a real AI account in a test; live checks must be opt-in and clearly marked.

## Getting set up

Requirements: Rust (the pinned toolchain installs itself from `rust-toolchain.toml`), Node.js 22.16+, and Python 3.11+. The C# example and `--profile release` also need a .NET 8 SDK.

```sh
git clone https://github.com/thomaslittle/originlm.git
cd originlm
npm ci --ignore-scripts
python -m venv .venv-build
# Activate .venv-build using your shell, then:
python -m pip install -r scripts/requirements-build.txt
npx --no-install playwright install chromium
npm run build
cargo build --locked -p originlm-cli
```

## Everyday commands

| Task | Command |
|---|---|
| Run the showcase with hot reload | `npm run dev` |
| Rust tests | `cargo test --workspace --locked` |
| Format and lint (CI enforces both) | `cargo fmt --all --check` and `cargo clippy --workspace --all-targets --all-features -- -D warnings` |
| JS/TS packages | `npm run test:unit`, `npm run test:sdk`, `npm run test:react` |
| Generated contracts are current | `npm run check:generated` (regenerate with `python scripts/generate_contracts.py`) |
| Project hygiene (stubs, broken links, package privacy) | `python scripts/check_project.py` |
| React Doctor (must report zero warnings or errors) | `npm run doctor` |
| Everything CI runs | `python scripts/verify.py --profile all` |

Contract changes start in `crates/originlm-contracts`; `protocol/` and both SDKs' `generated.*` files are generated from it and must be committed together with the change.

## Making a change

1. Open an issue first for anything larger than a bug fix or doc improvement, so we can agree on shape and scope.
2. Branch from `main`. Keep commits bounded and descriptive; this repository uses Conventional Commit prefixes such as `feat(discovery): ...`, `fix(adapters): ...`, `docs: ...`, `chore: ...`.
3. Write the failing test first where practical, then the fix.
4. Run the checks above that touch your change. CI runs the full matrix on Linux, Windows, and macOS.
5. Update the docs you affect: the [support matrix](docs/adapters/SUPPORT-MATRIX.md), the relevant [integration guide](docs/integration/), the [changelog](CHANGELOG.md) under *Unreleased*, and generated files.
6. Open a pull request using the template. Do not force-push over other people's work.

### Adding or changing a provider

Read the [adapter authoring guide](docs/adapters/AUTHORING.md). A provider contribution needs:

- Synthetic fixtures and negative cases (signed-out, malformed, oversized, hostile responses).
- Exact source, version, and license evidence for anything ported, with notices preserved in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) and `licenses/`.
- The supported operations and platforms, and an honest status for each: planned, implemented, fixture-tested, platform-executed, or live-qualified. Installed does not mean authenticated; fixture-tested does not mean live-qualified.
- A row in the support matrix and a doc under `docs/adapters/`.

### The React showcase (`apps/demo`)

- Stack: React 19 with the React Compiler, Vite, Tailwind CSS v4, shadcn/ui (style `base-mira`, Base UI, Phosphor icons), TanStack Router, zustand.
- Add or refresh shadcn primitives only with the shadcn CLI (`npx shadcn@latest add <name> --cwd apps/demo`, with `pnpm-workspace.yaml` temporarily moved aside so the CLI uses npm). Do not hand-write copies of shadcn components.
- State lives in the framework-free store (`src/state/store.ts`). Components subscribe to slices; do not add `useEffect` for anything other than subscribing to an external system.
- Every route in `src/lib/pages.ts` must also be listed in `apps/demo/server.mjs` and `crates/originlm-transports/src/http/routes.rs`.
- The demo uses only public SDK APIs and keeps synthetic data visibly separate from live results. `npm run doctor` must stay clean; a suppression needs a written reason next to it.

## Things we cannot accept

Registry publication, release signing, buying or resetting credits, or modifying a user's unrelated client settings are maintainer-only operations and are not part of any pull request.

## License

By contributing you agree that your contribution is licensed under the [MIT License](LICENSE). If you port code from another project, preserve its notice and say where it came from in the pull request.
