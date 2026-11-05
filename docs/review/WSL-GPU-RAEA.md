# WSL2 + RTX 3080: raea and GPUI

Tested on Thomas's Windows gaming PC to answer one question: can WSL2 with the
RTX 3080 replace the Fedora dual-boot for testing GPUI apps?

**Verdict: yes for both, with caveats.** raea renders locally on the 3080, and
`originlm-desktop` builds and paints under WSLg on the 3080 through Vulkan
(Dozen/D3D12) with no CPU fallback. The caveats are WSLg stability, the Dozen
feature gap, and building on ext4 rather than NTFS.

- Date: 2026-11-05
- Host: Windows 11, build 10.0.26100.9711, WSL 2.6.3.0, kernel 6.6.87.2-1, WSLg 1.0.71
- Distro: Ubuntu 24.04.3 LTS, x86_64, 8 vCPU (Ryzen 7 7800X3D), 19.5 GiB visible RAM
- GPU: NVIDIA GeForce RTX 3080 (10 GiB), Windows driver 610.57.01 (KMD 610.88, CUDA UMD 13.3)
- Plus a Radeon integrated GPU (Ryzen 7800X3D) exposed as a second Dozen device

Reproduce the baseline with [`scripts/wsl_gpu_check.sh`](../../scripts/wsl_gpu_check.sh):

```sh
wsl -d Ubuntu -- bash /mnt/c/Users/User/originlm/scripts/wsl_gpu_check.sh
```

## Phase 0 - baseline

| Check | Result |
| --- | --- |
| `wsl --status` / `--version` | WSL2 is default; 2.6.3.0, kernel 6.6.87.2, WSLg 1.0.71 |
| `uname -m` | `x86_64` |
| Linux NVIDIA driver | none installed, correctly (CUDA-on-WSL uses the Windows driver) |
| GPU passthrough | `/usr/lib/wsl/lib/nvidia-smi` reports the RTX 3080 |
| Vulkan | instance 1.3.275, Mesa 26.2.4; two Dozen/D3D12 devices (3080 discrete, Radeon integrated) and llvmpipe |
| OpenGL | llvmpipe (software) by default; `GALLIUM_DRIVER=d3d12` gives accelerated `D3D12 (AMD Radeon)` |
| Display | `DISPLAY=:0`, `WAYLAND_DISPLAY=wayland-0`, X0 and wayland-0 sockets present, PulseAudio socket present |

The important split:

- **Vulkan** is GPU-backed through **Dozen**, Mesa's D3D12 translation layer.
  Dozen is flagged non-conformant by the loader and reports Vulkan 1.2.354.
- **OpenGL** is software (llvmpipe) unless `GALLIUM_DRIVER=d3d12` is set. It
  then uses the *integrated* Radeon by default; add
  `MESA_D3D12_DEFAULT_ADAPTER_NAME=NVIDIA` to get `D3D12 (NVIDIA GeForce RTX 3080)`.
- `vkcube` selects GPU 0, "Microsoft Direct3D12 (NVIDIA GeForce RTX 3080)",
  type DiscreteGpu, and presents 60 frames without error.

## Phase 1 - raea

Installed with the documented script (`curl -fsSL https://raea.io/install | sh`),
no account and no payment:

- version **0.1.0-canary.2**, prefix `~/.raea`, CLI linked at `~/.local/bin/raea`
- installer also copied the `raea`/`raea-bevy` agent skills into `~/.claude/skills`

### Smoke render

A 1280x720, 30 fps, 2 s card (`WSL2 + RTX 3080` / `raea smoke test`):

```sh
cd ~/raea-smoke
raea render card.tsx -o out.mp4
```

- wall time **5.1 s**, **22.2 ms/frame**, **68 fps including encode**
- output **196,057 bytes**, probe: h264 `avc1.42C01F`, 1280x720, 30 fps, 60 frames, 2.0 s
- log: `APPLIED 60 frames rendered to out.mp4 (22.2 ms a frame, pages handed to raea-render on the GPU)`

### GPU stress render

3840x2160, 45 frames, 160 blurred layers:

- Dozen only, high quality: **113.2 s** wall, **2452 ms/frame**, output **11,332,201 bytes**
- the same comp cached; a forced llvmpipe run was not meaningfully comparable
  because page rasterization and the blur are CPU-side
- `nvidia-smi dmon` sampled from Windows during the render: SM utilization rose
  from a 28% idle baseline to **30-47%**, with encoder/engine activity 4-15%

Caveat: Linux raea encodes with **OpenH264 (software)**, not NVENC
(`"video":"h264 (openh264)"`). The GPU does the compositing; the final encode is CPU.

### raea verdict

**Pass.** Install, GPU render, and a valid mp4 all work on the 3080 under WSL2.
Render output is at `/home/tomlit/raea-smoke/out.mp4` (WSL path) and
`/home/tomlit/raea-smoke/proof.mp4` for the 4K stress case.

## Phase 2 - GPUI over WSL2

### What the stack needs

- Linux GPUI renders through **wgpu** now. This repo pins `gpui-kit 0.9.0`
  with the `gpui-fast` feature, which resolves to `gpui-fast 0.1.13` and
  `gpui-pre 0.3.11`; both have wgpu 30 backends.
- wgpu needs a Vulkan device with a working surface; under WSL that is Dozen.
- Old GPUI (Zed < 0.222) used **Blade**, which requires
  `VK_EXT_inline_uniform_block`. Dozen does not provide it, which is the root
  of the well-known "Zed does not run on WSL2" reports. The wgpu backend does
  not need that extension, so this stack is the one that works.
- WSLg's Weston used to expose `xdg_wm_base` v1 while old GPUI required v2. The
  pinned `gpui-pre-linux 0.3.11` accepts any version up to 6, so that panic is gone.
- WSLg provides both Wayland and X11 (XWayland). GPUI works on both; X11 is the
  one that can be screenshotted from inside WSL.

### Build

`/mnt/c` (NTFS) is slow for Cargo, so the test cloned the repo onto **ext4** at
`~/src/originlm` (GitHub `main`, `e8ee33f`) and built there:

```sh
git clone --branch main https://github.com/thomaslittle/originlm.git ~/src/originlm
cd ~/src/originlm
cargo build -p originlm-desktop
```

- cold build **168 s**, incremental **26 s**
- debug binary `target/debug/originlm-desktop`, **1.05 GB**
- all system deps were already present (`libxkbcommon-dev`, `libwayland-dev`,
  `libvulkan-dev`, `libfontconfig-dev`, Vulkan tools, clang, mold)

### Run

`originlm-desktop` has a credential-free example mode, which is what we used:

```sh
cd ~/src/originlm
LAI_DESKTOP_WINDOW=1440x920 ./target/debug/originlm-desktop --example
```

- **Wayland**: process stays up (22.5% CPU after 20 s), creates Dozen Vulkan
  devices, no crash. WSLg's Weston does not implement `wlr-screencopy`, so it
  cannot be screenshotted with `grim` - visual proof comes from the X11 run.
- **X11** (`WAYLAND_DISPLAY=`): full UI paints correctly at 1440x920. See
  [`screens/wsl-gpu/gpui-wslg-rtx3080.png`](screens/wsl-gpu/gpui-wslg-rtx3080.png).
- Under X11 the log shows `MESA-EGL: DRI3 error` / `failed to create dri2 screen`.
  That is WSLg's EGL/X11 path, not the renderer; Vulkan is unaffected and the
  window is correct.

### Proving it is the GPU, not llvmpipe

The clean test is to remove the CPU device from Vulkan entirely:

```sh
VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/dzn_icd.json \
ZED_DEVICE_ID=2206 \
./target/debug/originlm-desktop --example
```

- `dzn_icd.json` only exposes Dozen (RTX 3080 + Radeon); llvmpipe is not available.
- `ZED_DEVICE_ID=2206` is the 3080's PCI device id, which wgpu's adapter sort uses
  to prefer it (`parse_pci_id` accepts 4 hex digits, no `0x` needed).
- Result: window opens, full example-data UI renders, screenshot is identical to
  the default run. With no CPU Vulkan device present, this cannot be software rendering.
- For contrast, `VK_ICD_FILENAMES=.../lvp_icd.json` also renders the UI (the
  software fallback works), but it is not the path taken by default: wgpu sorts
  DiscreteGpu first and uses the first adapter that passes a real surface-config test.
- During the Dozen-only run, `nvidia-smi` SM utilization sat at 16-58% against a
  29% idle baseline, with memory-utilization spikes on repaint.

### GPUI verdict

**Pass** for `originlm-desktop` on this machine: it builds, opens a window,
paints the full UI, and does so on the RTX 3080 with no CPU fallback available.
twinhaus, remotif and crntly were **not exercised**, but all three pin the same
stack (`gpui-fast 0.1.13` / `gpui-kit 0.9.0` / `gpui-pre 0.3.11`), so the result
should carry over; only `tvapp` on `main` in `tv-studio` was inspected read-only,
and its tree was left untouched.

Not measured: a synthetic interaction/FPS benchmark. The app is idle when
nothing changes (about 1% CPU), so there is no continuous animation to time
without driving the UI. What is established is correct painting plus GPU
execution; "interactive rates" is inferred, not benchmarked.

## Phase 3 - what it means for the dual-boot

| Item | Verdict |
| --- | --- |
| raea on WSL + 3080 | **Pass** - 2 s 720p in 5.1 s; 4K stress render on the 3080 |
| GPUI on WSL + 3080 | **Pass** - `originlm-desktop` paints on the 3080, no CPU fallback |
| Replace dual-boot for day-to-day GPUI testing | **Yes**, with X11 and the caveats below |

What would make it boring instead of workable:

1. **Keep the source on ext4.** Build in `~/src/...`, not `/mnt/c/...`; Cargo on
   NTFS was avoided entirely and is the single biggest quality-of-life factor.
2. **Prefer X11 for GPUI today.** `WAYLAND_DISPLAY=` avoids the EGL/DRI3 noise and
   makes screenshots/capture scripts work with `import`. Wayland works but cannot
   be captured under WSLg.
3. **Watch for WSLg crashes.** WSLg's `WSLGd` died once mid-session
   (`Input/output error @../main.cpp:324`) and both display sockets disappeared;
   `wsl --shutdown` brought it back. Long GPU sessions may hit this.
4. **Do not move back to a Blade-based GPUI** without checking Dozen for
   `VK_EXT_inline_uniform_block`. The current wgpu backend is why this works.
5. **raea has no hardware encode here.** Renders are GPU-composited but encoded
   with OpenH264; that is the main performance gap versus a native Fedora/NVENC box.
6. The AMD iGPU is present as a second Dozen device. If an app ever lands on it,
   `ZED_DEVICE_ID=2206` (or `MESA_D3D12_DEFAULT_ADAPTER_NAME=NVIDIA` for GL)
   pins the 3080.

## Should OriginLM detect `raea` as a local tool?

Yes, later, but as a **Linux/macOS local runtime**, not a Windows one. OriginLM's
discovery already scans a configured PATH plus known roots, so raea would fit as a
tool entry resolved from `raea` on `PATH` and `~/.raea/bin/raea` (the installer's
default prefix), with the version read from `raea --version` or the installer's
`~/.raea/share/raea/VERSION`. It should only be offered on hosts where that binary
exists - WSL is a separate Linux host from the Windows desktop, so a WSL install
must not be advertised as a native Windows capability. Detection alone is cheap and
read-only; wiring it into a render flow should wait until raea's Linux output is
wanted in the product, because it also brings a large `~/.raea` engine and an
OpenH264 encode path that OriginLM would have to surface honestly.
