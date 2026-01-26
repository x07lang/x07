#!/usr/bin/env python3
"""Simple deterministic local HTTP barrier server.

Purpose
  Used by CI concurrency fixtures to verify that multiple HTTP requests are
  truly in-flight concurrently.

Semantics
  - Handles GET /barrier (any query string is ignored)
  - Does not respond until `--n` requests have reached the handler
  - Once `--n` arrivals are observed, all waiting requests are released
  - If the barrier isn't met within `--timeout-s`, requests return 504

Outputs
  - Writes a JSON "ready" file containing {host, port, url, barrier_n}
  - Prints the same JSON to stdout when ready
"""

from __future__ import annotations

import argparse
import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


class BarrierState:
    def __init__(self, n: int) -> None:
        self.n = n
        self.arrived = 0
        self.responded = 0
        self.shutdown_started = False
        self.lock = threading.Lock()
        self.released = threading.Event()

    def note_arrival(self) -> None:
        with self.lock:
            self.arrived += 1
            if self.arrived >= self.n:
                self.released.set()

    def note_response(self) -> bool:
        with self.lock:
            self.responded += 1
            if self.responded >= self.n and not self.shutdown_started:
                self.shutdown_started = True
                return True
            return False


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--bind", default="127.0.0.1", help="Bind address (default: 127.0.0.1)")
    ap.add_argument("--port", type=int, default=0, help="Port (0 = ephemeral)")
    ap.add_argument("--n", type=int, required=True, help="Barrier count")
    ap.add_argument("--timeout-s", type=float, default=15.0, help="Barrier wait timeout in seconds")
    ap.add_argument("--ready-file", required=True, help="Path to write readiness JSON")
    args = ap.parse_args()

    if args.n <= 0:
        raise SystemExit("--n must be > 0")

    state = BarrierState(args.n)

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self) -> None:  # noqa: N802
            if not self.path.startswith("/barrier"):
                self.send_response(404)
                self.send_header("content-type", "text/plain")
                self.end_headers()
                self.wfile.write(b"not found")
                return

            state.note_arrival()

            ok = state.released.wait(timeout=args.timeout_s)
            if not ok:
                self.send_response(504)
                self.send_header("content-type", "text/plain")
                self.end_headers()
                self.wfile.write(b"timeout waiting for barrier")
                return

            self.send_response(200)
            self.send_header("content-type", "text/plain")
            self.end_headers()
            self.wfile.write(b"ok")
            if state.note_response():
                threading.Thread(target=self.server.shutdown, daemon=True).start()

        def log_message(self, fmt: str, *fmt_args) -> None:
            return

    httpd = ThreadingHTTPServer((args.bind, args.port), Handler)
    host, port = httpd.server_address[0], httpd.server_address[1]
    info = {
        "host": host,
        "port": port,
        "url": f"http://{host}:{port}/barrier",
        "barrier_n": args.n,
    }

    with open(args.ready_file, "w", encoding="utf-8") as f:
        json.dump(info, f, indent=2)
        f.write("\n")

    print(json.dumps(info), flush=True)

    httpd.serve_forever()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
