# Measured clean-build baseline

Source `991ec2bd827cf6e3b6bc884c453ba1ca44bb2b91`; both builds had `workingTreeDirty=false`. These are measured developer-alpha values, not guarantees for every host, account, or future commit.

| Measurement | Windows x86-64 | Ubuntu/WSL2 x86-64 |
|---|---:|---:|
| Cold process + passive scan p95 (20 samples) | 25.652 ms | 3.820 ms |
| Cached stdio snapshot p95 (200 samples) | 0.580 ms | 0.646 ms |
| Peak idle resident set | 7.727 MiB | 6.625 MiB |
| Idle CPU delta as percentage of one core | 0.0% | 0.0% |
| Idle interval | 60.55 s | 62.37 s |
| Browser SDK gzip | 4,866 bytes | 4,866 bytes |

The fixture has nine registered passive adapters and ten synthetic profiles, not the full proposed 20-adapter benchmark. No network, provider startup, credential read, or inference is part of this measurement. Browser UI and native provider processes are excluded from helper memory. Zero sampled CPU delta is not a universal zero-CPU guarantee.

Windows uses Rust 1.97.1; the installed Ubuntu toolchain was Rust 1.90.0. Both use Node 22.20.0. The machine reports 16 Windows logical CPUs and eight WSL virtual CPUs. WSL results do not certify every Linux distribution/libc or macOS.

These historical measurements are context only. Generate fresh distributions, binary hashes and host-specific measurements for each OriginLM release using the command below.

Reproduce with `python scripts/benchmark.py --bundle ABSOLUTE_EXTRACTED_BUNDLE --idle-seconds 60`. The script records actual values in `.verification/performance.json`; rerun it against each changed binary rather than copying this table. Publication and signing are separate, unperformed actions.
