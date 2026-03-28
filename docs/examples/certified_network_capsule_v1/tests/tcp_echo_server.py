#!/usr/bin/env python3

from __future__ import annotations

import argparse
import socket


def run_server(host: str, port: int, timeout_s: int) -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        sock.bind((host, port))
        sock.listen(16)
        sock.settimeout(timeout_s)

        served = 0
        while served < 8:
            try:
                conn, _addr = sock.accept()
            except socket.timeout:
                return 0 if served > 0 else 2

            with conn:
                conn.settimeout(timeout_s)
                try:
                    data = conn.recv(4096)
                except socket.timeout:
                    continue
                if data:
                    conn.sendall(data)
                    served += 1
    return 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=30030)
    ap.add_argument("--timeout-s", type=int, default=20)
    args = ap.parse_args()
    return run_server(host=args.host, port=args.port, timeout_s=args.timeout_s)


if __name__ == "__main__":
    raise SystemExit(main())
