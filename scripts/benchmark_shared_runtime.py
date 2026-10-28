#!/usr/bin/env python3
"""Measure the shared runtime without provider accounts or inference.

Covers cold process+passive scan, warm stdio snapshot latency, idle RSS/CPU,
and the broker-owned SQLite file size. Values are recorded, not promised.
"""
import argparse
import asyncio
import json
import math
import os
import pathlib
import platform
import statistics
import subprocess
import sys
import tempfile
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]


def summarize(values):
    ordered = sorted(values)
    return {
        "count": len(values),
        "min": ordered[0],
        "mean": statistics.mean(values),
        "p50": ordered[math.ceil(len(ordered) * 0.50) - 1],
        "p95": ordered[math.ceil(len(ordered) * 0.95) - 1],
        "max": ordered[-1],
    }


async def measure(binary, idle_seconds):
    import psutil

    with tempfile.TemporaryDirectory(prefix="originlm-shared-bench-") as temporary:
        home = pathlib.Path(temporary)
        for index in range(10):
            (home / f".codex-fixture-{index}").mkdir()
        cold = []
        for _ in range(10):
            started = time.perf_counter()
            process = await asyncio.create_subprocess_exec(
                str(binary), "scan", "--json", "--home", str(home),
                stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.DEVNULL,
            )
            raw, _ = await asyncio.wait_for(process.communicate(), 20)
            if process.returncode or len(raw) > 2 * 1024 * 1024:
                raise RuntimeError("cold scan failed")
            cold.append((time.perf_counter() - started) * 1000)
        process = await asyncio.create_subprocess_exec(
            str(binary), "serve", "--stdio", "--home", str(home),
            "--allow-passive-discovery",
            stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.DEVNULL, limit=4 * 1024 * 1024,
        )
        client = psutil.Process(process.pid)
        sequence = 0

        async def request(method, params):
            nonlocal sequence
            sequence += 1
            process.stdin.write(
                (json.dumps({"jsonrpc": "2.0", "id": sequence, "method": method, "params": params}) + "\n").encode()
            )
            await process.stdin.drain()
            line = await asyncio.wait_for(process.stdout.readline(), 20)
            if len(line) > 4 * 1024 * 1024:
                raise RuntimeError("unexpected frame")
            response = json.loads(line)
            if response.get("id") != sequence or "result" not in response:
                raise RuntimeError(f"request failed: {method}")
            return response["result"]

        try:
            await request("runtime.handshake", {"protocolVersion": "1.0", "clientName": "benchmark"})
            await request("discovery.refresh", {"level": "passive"})
            for _ in range(10):
                await request("snapshot.get", {})
            warm = []
            for _ in range(200):
                started = time.perf_counter()
                await request("snapshot.get", {})
                warm.append((time.perf_counter() - started) * 1000)
            times = client.cpu_times()
            cpu_start = times.user + times.system
            started = time.perf_counter()
            rss = []
            for _ in range(idle_seconds):
                rss.append(client.memory_info().rss)
                await asyncio.sleep(1)
            elapsed = time.perf_counter() - started
            times = client.cpu_times()
            cpu_percent = (times.user + times.system - cpu_start) / elapsed * 100
            state = home / ".originlm" / "state.db"
            db_bytes = state.stat().st_size if state.exists() else 0
            wal_bytes = state.with_name(state.name + "-wal").stat().st_size if state.with_name(state.name + "-wal").exists() else 0
        finally:
            if process.returncode is None:
                process.terminate()
                try:
                    await asyncio.wait_for(process.wait(), 3)
                except TimeoutError:
                    process.kill()
                    await process.wait()
    return {
        "binary": str(binary),
        "platform": platform.platform(),
        "logicalCPUs": os.cpu_count(),
        "fixture": {"emptyHome": True, "registeredProfiles": 10, "providerCalls": 0, "inferenceRequests": 0},
        "coldProcessAndScanMs": summarize(cold),
        "warmSnapshotMs": summarize(warm),
        "idle": {"seconds": elapsed, "oneCoreCPUPercent": cpu_percent, "peakRSSMiB": max(rss) / (1024 * 1024)},
        "stateDbBytes": db_bytes,
        "stateDbWalBytes": wal_bytes,
        "targetsMet": {
            "coldP95Below1s": summarize(cold)["p95"] < 1000,
            "warmP95Below50ms": summarize(warm)["p95"] < 50,
            "idleBelow50MiB": max(rss) / (1024 * 1024) < 50,
            "idleBelowOnePercent": cpu_percent < 1,
        },
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=pathlib.Path)
    parser.add_argument("--idle-seconds", type=int, default=10)
    parser.add_argument("--enforce", action="store_true")
    args = parser.parse_args()
    if not 5 <= args.idle_seconds <= 300:
        raise SystemExit("idle sampling must be 5..300 seconds")
    binary = args.binary or ROOT / "target" / "debug" / ("originlm.exe" if os.name == "nt" else "originlm")
    if not binary.is_file():
        raise SystemExit(f"binary is missing: {binary}")
    report = asyncio.run(measure(binary.resolve(), args.idle_seconds))
    output = ROOT / ".verification" / "shared-runtime-performance.json"
    output.parent.mkdir(exist_ok=True)
    output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))
    return 1 if args.enforce and not all(report["targetsMet"].values()) else 0


if __name__ == "__main__":
    sys.exit(main())
