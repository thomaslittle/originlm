# Tokscale review artifacts

`scripts/upstream_sync.py --open-pr` writes `latest/report.md` on the `upstream/tokscale-discovery-review` branch. Those reports are review proposals. They do not change the pinned revision or production discovery until a maintainer updates `lock.json`, `snapshot.json`, and the Rust adapters in the same change.
