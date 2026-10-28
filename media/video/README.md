# OriginLM product film

A 35-second silent film made with [fframes](https://fframes.studio), using actual
example-mode UI and the same three.js scene as the site. No accounts are read.
The five chapters cover scattered AI, the dashboard, the developer platform,
permission and installation, followed by the OriginLM lockup.

Source is in `src/`; licensed fonts, logo and example captures are in `media/`.
Large renders belong in ignored `out/`. The commands below reproduce the source film; rendered media is staged explicitly and is not required to build the SDK or desktop application.

From the repository root, with dependencies installed:

```bash
# In separate terminals, start the example app and site on their reserved ports.
# Capture only explicitly labeled example data; live mode is rejected.
node scripts/capture_example_ui.mjs
npm --prefix apps/site run dev
node media/video/scripts/render-studio.mjs
cp media/video/out/studio.mp4 media/video/media/studio.mp4
cargo build --release --manifest-path media/video/Cargo.toml
env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET \
  media/video/target/release/originlm-film render \
  -o media/video/out/originlm-1080p.mp4
```

The studio encoder defaults to FFmpeg's `libx264`; a system exposing only
OpenH264 can set `VIDEO_ENCODER=libopenh264`. The fframes build needs compatible
FFmpeg development libraries. On this Linux machine, its downloaded FFmpeg
expected a different x264 ABI, so `FFMPEG_FORCE_BUILD=1` and NASM were used for
an isolated source build. No system library was replaced.

Stage the encoded MP4, WebM and poster with `node media/video/scripts/stage-site.mjs`,
then rebuild the site. Without all three assets the site displays an honest
source-only fallback. This script does not publish or deploy anything.
