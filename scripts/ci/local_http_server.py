#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import signal
import sys
import threading
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Optional


class QuietHandler(SimpleHTTPRequestHandler):
    # Python's handler is chatty; in CI we usually want silence.
    def log_message(self, fmt: str, *args: object) -> None:  # noqa: N802 (stdlib signature)
        return


def _write_atomic_json(path: Path, payload: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    tmp.replace(path)


def main() -> int:
    ap = argparse.ArgumentParser(
        description="Serve a local static directory (CI helper). Writes a ready JSON and runs until killed."
    )
    ap.add_argument("--root", required=True, help="Directory to serve.")
    ap.add_argument("--host", default="127.0.0.1", help="Bind host (default: 127.0.0.1).")
    ap.add_argument("--port", type=int, default=0, help="Bind port (0 = ephemeral).")
    ap.add_argument(
        "--ready-json",
        default="",
        help="Write server info JSON here once listening. If empty, prints JSON to stdout once listening.",
    )
    ap.add_argument("--pid-file", default="", help="Optional file to write the server PID.")
    ap.add_argument("--quiet", action="store_true", help="Suppress request logging.")
    args = ap.parse_args()

    root = Path(args.root).resolve()
    if not root.is_dir():
        sys.stderr.write(f"ERROR: --root is not a directory: {root}\n")
        return 2

    # Ensure cwd doesn't affect relative path serving.
    os.chdir(str(root))

    handler_cls = QuietHandler if args.quiet else SimpleHTTPRequestHandler
    handler = partial(handler_cls, directory=str(root))

    httpd = ThreadingHTTPServer((args.host, args.port), handler)
    actual_host, actual_port = httpd.server_address[0], httpd.server_address[1]

    info = {
        "host": actual_host,
        "port": actual_port,
        "url": f"http://{actual_host}:{actual_port}/",
        "root": str(root),
        "pid": os.getpid(),
    }

    if args.pid_file:
        Path(args.pid_file).write_text(str(os.getpid()) + "\n", encoding="utf-8")

    if args.ready_json:
        _write_atomic_json(Path(args.ready_json), info)
    else:
        sys.stdout.write(json.dumps(info) + "\n")
        sys.stdout.flush()

    # Graceful shutdown on SIGTERM/SIGINT.
    def _shutdown(_signum: int, _frame: Optional[object]) -> None:
        try:
            # `HTTPServer.shutdown()` must be called from a different thread than
            # the one running `serve_forever()`, or it can deadlock.
            threading.Thread(target=httpd.shutdown, daemon=True).start()
        except Exception:
            pass

    signal.signal(signal.SIGTERM, _shutdown)
    signal.signal(signal.SIGINT, _shutdown)

    try:
        httpd.serve_forever(poll_interval=0.25)
    finally:
        try:
            httpd.server_close()
        except Exception:
            pass

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
