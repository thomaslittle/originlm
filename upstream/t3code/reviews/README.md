# T3 Code review reports

`scripts/upstream_t3code.py --open-pr` writes `latest/report.md` on the `upstream/t3code-provider-review` branch. Those reports are review proposals. They do not change the pinned revision, discovery, or ChatGPT sign-in until a maintainer updates `snapshot.json`, `UPSTREAM_T3CODE.md`, and the Rust adapters in the same change.

The watched paths cover local model providers, cloud CLI providers, installed CLI detection, and the ChatGPT OAuth flow.
