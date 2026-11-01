# Release evidence snapshots

`msrv-1.88.json` is the artifact produced by `.github/workflows/msrv.yml` on
PR #16. It records the exact command, rustc/cargo versions, declared MSRV,
exit code and complete compiler log.

The `testedCommit` field is the GitHub pull-request merge commit that the PR
workflow checked out. On an integrated `main` run the same workflow records the
integrated `main` commit. The artifact is regenerated on every relevant push and
uploaded as `msrv-1.88-evidence`; this file is a reviewable snapshot, not a
substitute for the CI artifact.
