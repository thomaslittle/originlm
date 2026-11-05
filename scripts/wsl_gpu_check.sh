#!/usr/bin/env bash
# What this WSL2 distro can and cannot do with the Windows GPU.
#
#   bash scripts/wsl_gpu_check.sh
#
# Prints a short report: WSL version, whether the NVIDIA passthrough driver is
# visible, which Vulkan devices exist (and which of them are real GPUs vs CPU
# fallbacks), and whether WSLg is serving X11/Wayland right now.
#
# CUDA-on-WSL rule: never install a Linux NVIDIA driver inside WSL. The Windows
# driver is mounted at /usr/lib/wsl/lib and is the only one that should exist.
set -uo pipefail

say() { printf '%s\n' "$*"; }
section() { printf '\n== %s ==\n' "$*"; }
present() { command -v "$1" >/dev/null 2>&1; }

section "kernel / distro"
uname -m
grep -E '^(PRETTY_NAME|VERSION_ID)=' /etc/os-release 2>/dev/null
if grep -qi microsoft /proc/version 2>/dev/null; then
  say "WSL: yes ($(uname -r))"
else
  say "WSL: no (plain Linux kernel) - GPU passthrough checks below are not meaningful"
fi

section "NVIDIA passthrough"
if [ -x /usr/lib/wsl/lib/nvidia-smi ]; then
  /usr/lib/wsl/lib/nvidia-smi --query-gpu=name,driver_version,memory.total \
    --format=csv,noheader 2>&1 | sed 's/^/  /'
elif present nvidia-smi; then
  nvidia-smi --query-gpu=name,driver_version,memory.total --format=csv,noheader 2>&1 | sed 's/^/  /'
else
  say "  no nvidia-smi: GPU passthrough is not available"
fi
if [ -e /usr/lib/x86_64-linux-gnu/libnvidia-ptxjitcompiler.so.1 ]; then
  say "  WARNING: a Linux NVIDIA driver appears to be installed; remove it and keep only the Windows driver"
fi

section "Vulkan devices"
if ! present vulkaninfo; then
  say "  vulkaninfo missing: sudo apt install vulkan-tools"
else
  vulkaninfo --summary 2>/dev/null | awk '
    function flush() {
      if (gpu != "") printf "  %-7s %-55s %-24s %s\n", gpu, name, drv, typ
      name = "?"; drv = "?"; typ = "?"
    }
    /^GPU[0-9]+:/ { flush(); gpu = $0; next }
    /deviceName/ { sub(/.*deviceName *= */, ""); name = $0 }
    /driverName/ { sub(/.*driverName *= */, ""); drv = $0 }
    /deviceType/ { sub(/.*deviceType *= */, ""); typ = $0 }
    END { flush() }'
  dzn=$(vulkaninfo --summary 2>/dev/null | grep -c 'deviceName *= *Microsoft Direct3D12')
  lvp=$(vulkaninfo --summary 2>/dev/null | grep -c 'deviceName *= *llvmpipe')
  say ""
  say "  Dozen (D3D12 -> real GPU) devices: $dzn"
  say "  llvmpipe (CPU) devices:            $lvp"
  if [ "$dzn" -eq 0 ]; then
    say "  NOTE: without Dozen, Vulkan is CPU-only; wgpu-based apps will be slow or refuse a GPU adapter."
  else
    say "  NOTE: Dozen is not a conformant Vulkan implementation; apps that need extensions"
    say "        it lacks (for example VK_EXT_inline_uniform_block for Blade-era GPUI) will fail."
  fi
fi

section "WSLg display"
say "  DISPLAY=${DISPLAY:-<unset>}"
say "  WAYLAND_DISPLAY=${WAYLAND_DISPLAY:-<unset>}"
if present xdpyinfo && [ -n "${DISPLAY:-}" ] && xdpyinfo >/dev/null 2>&1; then
  say "  X11: reachable"
else
  say "  X11: NOT reachable (restart WSL with 'wsl --shutdown' if WSLg crashed)"
fi
if [ -n "${WAYLAND_DISPLAY:-}" ] && [ -S "${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/${WAYLAND_DISPLAY}" ]; then
  say "  Wayland: socket present"
else
  say "  Wayland: socket missing"
fi
say ""
say "  WSLg's OpenGL is llvmpipe by default. Vulkan (wgpu, raea, GPUI) goes through Dozen instead."
say "  For OpenGL apps, D3D12 GL works: GALLIUM_DRIVER=d3d12"
say "  (add MESA_D3D12_DEFAULT_ADAPTER_NAME=NVIDIA to put it on the discrete GPU)."
