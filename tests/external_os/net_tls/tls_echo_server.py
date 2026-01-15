import argparse
import shutil
import socket
import ssl
import subprocess
import tempfile
from pathlib import Path


def generate_test_cert_and_key(tmp_path: Path) -> tuple[Path, Path]:
    cert_path = tmp_path / "cert.pem"
    key_path = tmp_path / "key.pem"

    openssl = shutil.which("openssl")
    if openssl is None:
        raise RuntimeError("missing tool: openssl")

    subprocess.run(
        [
            openssl,
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-keyout",
            str(key_path),
            "-out",
            str(cert_path),
            "-days",
            "1",
            "-nodes",
            "-subj",
            "/CN=localhost",
        ],
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return cert_path, key_path


def run_server(host: str, port: int, timeout_s: int) -> int:
    with tempfile.TemporaryDirectory() as tmp:
        tmp_path = Path(tmp)
        cert_path, key_path = generate_test_cert_and_key(tmp_path)

        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        ctx.load_cert_chain(certfile=str(cert_path), keyfile=str(key_path))

        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
            sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            sock.bind((host, port))
            sock.listen(16)
            sock.settimeout(timeout_s)

            served = 0
            while served < 1:
                try:
                    conn, _addr = sock.accept()
                except socket.timeout:
                    return 2

                with conn:
                    try:
                        tls = ctx.wrap_socket(conn, server_side=True)
                    except ssl.SSLError:
                        continue

                    with tls:
                        tls.settimeout(timeout_s)
                        try:
                            data = tls.recv(4096)
                        except (ssl.SSLError, socket.timeout):
                            continue
                        if data:
                            tls.sendall(data)
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
