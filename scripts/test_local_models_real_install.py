#!/usr/bin/env python3
"""Assert the server-off on-disk model inventory found a pulled Ollama model."""
from __future__ import annotations
import argparse, json, pathlib, sys

parser = argparse.ArgumentParser()
parser.add_argument("inventory", type=pathlib.Path)
args = parser.parse_args()
document = json.loads(args.inventory.read_text(encoding="utf-8"))
providers = {item["providerId"]: item for item in document.get("providers", [])}
ollama = providers.get("ollama", {"models": [], "diagnostics": []})
identifiers = [model.get("nativeId", "") for model in ollama.get("models", [])]
if not any(identifier.startswith("smollm2:") for identifier in identifiers):
    print("LOCAL MODEL REAL-INSTALL FAILURE", file=sys.stderr)
    print(f"ollama models: {identifiers}", file=sys.stderr)
    print(f"diagnostics: {ollama.get('diagnostics')}", file=sys.stderr)
    raise SystemExit(1)
print("Ollama on-disk models:", ", ".join(identifiers))
