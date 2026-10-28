#!/usr/bin/env python3
"""Synthetic loopback OpenCode gateway for guarded GUI qualification.

The process never contacts OpenCode, reads credentials, or runs a model. It
serves only the public `/provider` catalog shape used by OriginLM, with two
distinct upstream providers:

* `opencode-go` - the subscription route class, models change in "updated" mode
* `opencode`    - the Console pay-as-you-go route class

`FAKE_OPENCODE_STATE` names a JSON file read on every request:
  {"mode": "normal"}   both providers, baseline model set
  {"mode": "updated"}  one additional Go model (incremental refresh)
  {"mode": "recovered"} updated plus a fresh Go model for outage recovery
  {"mode": "error"}    HTTP 503 (failure injection)

No inference endpoint is implemented; a session request gets HTTP 501. This is
deliberate: the qualification exercises discovery, model selection, refresh,
cached state and failure display without any provider charge.
"""
import argparse
import json
import os
import pathlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


def provider_payload(mode: str) -> dict:
    go_models = {
        "kimi-k3": {
            "id": "kimi-k3",
            "providerID": "opencode-go",
            "name": "Kimi K3",
            "capabilities": {
                "input": {"text": True, "image": False},
                "output": {"text": True},
                "toolcall": False,
            },
            "limit": {"context": 262144},
        }
    }
    if mode in ("updated", "recovered"):
        go_models["glm-5.3"] = {
            "id": "glm-5.3",
            "providerID": "opencode-go",
            "name": "GLM-5.3",
            "capabilities": {
                "input": {"text": True, "image": False},
                "output": {"text": True},
                "toolcall": False,
            },
            "limit": {"context": 131072},
        }
    if mode == "recovered":
        go_models["kimi-k4"] = {
            "id": "kimi-k4",
            "providerID": "opencode-go",
            "name": "Kimi K4",
            "capabilities": {
                "input": {"text": True, "image": False},
                "output": {"text": True},
                "toolcall": False,
            },
            "limit": {"context": 262144},
        }
    return {
        "connected": ["opencode-go", "opencode"],
        "default": {"opencode-go": "kimi-k3", "opencode": "gpt-6-sol"},
        "all": [
            {
                "id": "opencode-go",
                "name": "OpenCode Go",
                "models": go_models,
            },
            {
                "id": "opencode",
                "name": "OpenCode Console",
                "models": {
                    "gpt-6-sol": {
                        "id": "gpt-6-sol",
                        "providerID": "opencode",
                        "name": "GPT 6 Sol",
                        "capabilities": {
                            "input": {"text": True, "image": False},
                            "output": {"text": True},
                            "toolcall": False,
                        },
                        "limit": {"context": 196608},
                    }
                },
            },
        ],
    }


def current_mode() -> str:
    path = os.environ.get("FAKE_OPENCODE_STATE")
    if not path:
        return "normal"
    try:
        return json.loads(pathlib.Path(path).read_text(encoding="utf8")).get(
            "mode", "normal"
        )
    except (OSError, ValueError):
        return "normal"


class Handler(BaseHTTPRequestHandler):
    server_version = "FakeOpenCode/1.0"

    def log_message(self, _format, *_args):
        return

    def send_json(self, status: int, payload: dict):
        body = json.dumps(payload).encode("utf8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path.split("?", 1)[0] == "/provider":
            mode = current_mode()
            if mode == "error":
                self.send_json(503, {"error": "synthetic gateway outage"})
                return
            self.send_json(200, provider_payload(mode))
            return
        if self.path.split("?", 1)[0] in ("/health", "/"):
            self.send_json(200, {"ok": True})
            return
        self.send_json(404, {"error": "not implemented by the synthetic gateway"})

    def do_POST(self):
        # The synthetic gateway never runs inference. Model selection and
        # failure qualification must not accidentally charge an account.
        self.send_json(501, {"error": "inference is not implemented in this fixture"})


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, default=0)
    parser.add_argument("--port-file")
    args = parser.parse_args()
    server = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    port = server.server_address[1]
    if args.port_file:
        path = pathlib.Path(args.port_file)
        path.write_text(str(port), encoding="utf8")
    print(f"FAKE_OPENCODE_READY port={port}", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
