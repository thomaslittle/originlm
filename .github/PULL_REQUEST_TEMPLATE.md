## Summary

<!-- What changes and why. Link the issue it closes. -->

## Verification

<!-- Commands you ran and what they showed. Never include real tokens or account data. -->

- [ ] `cargo fmt --all --check` and `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [ ] Relevant tests (`cargo test`, `npm run test:*`, `python scripts/verify.py --profile ...`)
- [ ] `npm run check:generated` if contracts or protocol files changed
- [ ] `npm run doctor` if `apps/demo` changed

## Checklist

- [ ] Tests use synthetic homes, fake clients, or mock servers; no real account was consumed.
- [ ] Imports and constructors still do no scanning, credential reading, spawning, or network calls.
- [ ] Unknown and unsupported states are preserved; nothing is guessed or fabricated.
- [ ] Docs, the [support matrix](https://github.com/thomaslittle/originlm/blob/main/docs/adapters/SUPPORT-MATRIX.md), and `CHANGELOG.md` (Unreleased) are updated where relevant.
- [ ] Status claims distinguish planned / implemented / fixture-tested / platform-executed / live-qualified.
