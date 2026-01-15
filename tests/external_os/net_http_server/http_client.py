#!/usr/bin/env python3

import argparse
import socket
import sys
import time
from typing import List, Optional


def _connect_retry(host: str, port: int, timeout_s: float) -> socket.socket:
    deadline = time.monotonic() + timeout_s
    last_err: Optional[BaseException] = None
    while time.monotonic() < deadline:
        try:
            s = socket.create_connection((host, port), timeout=1.0)
            s.settimeout(2.0)
            return s
        except Exception as e:
            last_err = e
            time.sleep(0.02)
    raise RuntimeError(f"connect timeout to {host}:{port}: {last_err!r}")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--host", required=True)
    ap.add_argument("--port", type=int, required=True)
    ap.add_argument("--timeout-s", type=float, default=5.0)
    args = ap.parse_args()

    s = _connect_retry(args.host, args.port, args.timeout_s)
    with s:
        req = b"GET /hello HTTP/1.1\r\nHost: localhost\r\n\r\n"
        s.sendall(req)

        chunks: List[bytes] = []
        for _ in range(4096):
            b = s.recv(4096)
            if not b:
                break
            chunks.append(b)
            if sum(len(x) for x in chunks) > 1_000_000:
                raise RuntimeError("response too large")
        resp = b"".join(chunks)

    if b"\r\n\r\n" not in resp:
        raise RuntimeError(f"invalid http response (missing header terminator): {resp[:200]!r}")

    head, body = resp.split(b"\r\n\r\n", 1)
    status_line = head.split(b"\r\n", 1)[0]
    if not status_line.startswith(b"HTTP/1.1 200"):
        raise RuntimeError(f"unexpected status line: {status_line!r}")

    if body != b"hello":
        raise RuntimeError(f"unexpected body: {body!r}")

    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except BrokenPipeError:
        raise
    except Exception as e:
        print(f"http_client.py: {e}", file=sys.stderr)
        raise
