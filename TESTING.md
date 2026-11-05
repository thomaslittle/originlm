# Testing OriginLM

The test pyramid is deliberately weighted toward fast, deterministic checks.
The desktop app is a GPUI application; its UI is tested in-process, not by
driving the operating system.

| Layer | Share | What it covers | Default PR CI |
| --- | --- | --- | --- |
| Logic | ~60% | Crates, contracts, adapters, CLI, SDKs | Linux, Windows, macOS |
| In-process GPUI views | ~30% | Real desktop views, clicks, focus, layout | Linux, Windows |
| Linux golden frames | ~8% | Rendered pixels against Mesa/lavapipe | Optional, Linux only |
| Self-driving smoke | ~2% | A real binary launch, report file, bounded time | Nightly or path-filtered |

## Everyday commands

```sh
# Fast Rust logic tests
cargo test --workspace --locked

# Desktop logic plus headless in-process GPUI view tests
cargo test -p originlm-desktop --locked

# Only the view smoke test
cargo test -p originlm-desktop --locked view_tests

# Same desktop suite through the repository verifier
python scripts/verify.py --profile desktop

# Optional nextest runner (reads .config/nextest.toml)
cargo nextest run --workspace

# Full deterministic repository verification
python scripts/verify.py --profile all
```

JavaScript, Python and UI checks keep their existing entry points:

```sh
npm run test:unit
npm run test:sdk
npm run test:react
npm run test:ui
npm run test:a11y
```

## In-process desktop view tests

`crates/originlm-desktop` enables `gpui-kit/test-support` only for test
targets. The smoke test in `src/app.rs` opens the real `Desktop` shell in a
headless window, draws a frame, finds production navigation and header
controls, clicks them, and asserts the resulting state. It does not open an OS
window, move the pointer, or read a real account.

To add a view test:

1. Give the production element a stable `ElementId`.
2. Add `.test_support()` after its builder methods (before
   `.into_any_element()`). The method is inert in normal builds.
3. Use `#[gpui_kit::test]` with a `TestAppContext`, `gpui_kit::open_window`,
   `window.draw(cx).clear(cx)`, and `window.find` / `window.click`.
4. For asynchronous effects, use `cx.run_until_parked()` or advance the test
   clock deliberately. Do not sleep on wall time.

Tests are hermetic: `load_prefs` returns defaults and `save_prefs` is a no-op
under `cfg!(test)`, so a developer's real desktop preferences are never read or
written.

## CI shape

- `ci.yml` → `verify` runs the full deterministic matrix on Linux, Windows and
  macOS.
- `ci.yml` → `desktop` runs desktop logic and the in-process view tests on
  Linux and Windows.
- `detection-real-install.yml` is a real-install soak. It runs nightly, on
  demand, and on PRs that touch detection code. It is not a default PR gate.

## OS-level checks are not PR gates

These stay out of default pull-request CI because they need a real desktop,
real installs, or long wall-clock waits:

- `scripts/capture_desktop_example.ps1` and `scripts/capture_desktop_example.sh`
  capture real windows for design review.
- `scripts/test_detection_real_install.py` and
  `scripts/test_local_models_real_install.py` assert real client and model
  installs.
- Any click-through, `SendInput`, Xvfb/Wayland input injection, or long soak.

Run them manually or from the nightly/path-filtered workflow. A PR should not
wait on them.

## Linux golden frames

Rendered-frame checks are opt-in. Use a software Vulkan adapter such as Mesa
lavapipe and the GPUI Kit rendering target, or the capture script, on Linux
only. Golden frames are supplementary evidence; logic and in-process view
tests remain the required signal.
