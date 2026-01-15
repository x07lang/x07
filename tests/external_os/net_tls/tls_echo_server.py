import argparse
import socket
import ssl
import tempfile
from pathlib import Path


CERT_PEM = """-----BEGIN CERTIFICATE-----
MIICpDCCAYwCCQDdSNl+smNXQzANBgkqhkiG9w0BAQsFADAUMRIwEAYDVQQDDAls
b2NhbGhvc3QwHhcNMjYwMTExMjEyOTM2WhcNMzYwMTA5MjEyOTM2WjAUMRIwEAYD
VQQDDAlsb2NhbGhvc3QwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQDq
c1JClbkgUf8dw790leCyZALTYJgEykXWcsd5t/XiTZnO1v1RqqtT3+FjoKcMYhrF
a4iFZyefqT6QQctl0+0QtynvZNcqTne3XNcoRPxxiy4jlG+yvdf9Wpoz4s2mT6ot
1zws1QeRYQgO/Au+w6Cp24lIFvgPUlIcevKynHkSJawzRuEvUk1amBPS3IMTb8sy
TDAiFiGFXIBNZByETSVHQrOhfNf8zbNd+rjdGs8Rvgvt2JS77rhIVb6QQQ+dF29+
5xrbwHNNj/mpnK1jkBa2arbRI2Rdo04aMbv6QGZOmh2OawJVjz3FbRizKIn72I3A
dk1/fhjQ2SfxdBgU9Ax9AgMBAAEwDQYJKoZIhvcNAQELBQADggEBABHz/GRzUVMP
WJ3olql66qGiMISSmY3lkTx2FPqDAgv+pLjMLhcLtLNEA/17D4dvC4qdHmL0WUP2
CrXKIXzdeFjtNjZhwJrG94+9LqSH/37+8WfYE66rZActKuUwPecl0T+i6n9otRGQ
ushZzRgB5L+SH82+WLt3GXgL+yNmC4pcGfXRjnzCM9k2/3rTyL0o6DsBrOExE1dQ
bn7VWH/cAfxZaW1qwK5Qq8zaQzv+KiR3YWy/DyLKPxBf0EXGk6k+iuKWA+jldbF5
0TWohHSmHvJnLlC4/avJ3jqd3MxO/467fMRKH+/SWy6WORLV3sqeAamNEZJejE/X
1dfnG8r2Eic=
-----END CERTIFICATE-----
"""

KEY_PEM = """-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQDqc1JClbkgUf8d
w790leCyZALTYJgEykXWcsd5t/XiTZnO1v1RqqtT3+FjoKcMYhrFa4iFZyefqT6Q
Qctl0+0QtynvZNcqTne3XNcoRPxxiy4jlG+yvdf9Wpoz4s2mT6ot1zws1QeRYQgO
/Au+w6Cp24lIFvgPUlIcevKynHkSJawzRuEvUk1amBPS3IMTb8syTDAiFiGFXIBN
ZByETSVHQrOhfNf8zbNd+rjdGs8Rvgvt2JS77rhIVb6QQQ+dF29+5xrbwHNNj/mp
nK1jkBa2arbRI2Rdo04aMbv6QGZOmh2OawJVjz3FbRizKIn72I3Adk1/fhjQ2Sfx
dBgU9Ax9AgMBAAECggEBAK1nDfhhdMMK4n3JQdmg4MgQYGamksk4Md8ilZbZEOuI
KbJqIII+kOlANRvSvrrR9Kr/lcHVQeE89CEOCLoPvM8YKdP96YZI8xKTgC8wluYD
4uQ97T9uWknwsQyfOys+0MeG4eLmzOohsiwjDyzQ1AvNbAP9uQrcAA9AgDDKumFI
QWN9wDzmf2wf+/tYCVeeRpglGsBIUOc4E7JJdYaRdq/5L028FpEbUrB2LVKLo48m
GIpIGbk2El5zMCyp0UOi554/xCO1/wzxCC1qoMl1e2fZn9vVZgvPM6BmIMrzx7px
chj57QDpM27JHUtANbu7UqgQe00iIbcH97cOMZAH36ECgYEA9wyfr6l99Xg23g8p
JNL5qZEoyt0vchRSX6ndct7WAbTjzbiZDocHcG3/53SFQ2PYYBm/U5TrDAzOn6nT
ITjd5lQ5c2KTMp26jTZpn6nKVHZGatmRZbabKUo7eKd65Qov4Vyw52NJZiW6jxuo
qWbyZzeZgqtixm51qwCs2ZbB8xcCgYEA8vHX+VCOsMRXnlGPF4Zuc3p1eW/P59lv
5S3JdnrhqzDFZz8Qd0vZ7jQrKkJMrB0NIq3boZANSYKF0XRXNgVOeaeIq/nSBIxB
+2+tgc903UNeWfc0Wy0cAT0sCt65vrW5+z3KIQ7NRtQ73+NPtKltcobRXeATzqOS
xhUgGFl4yYsCgYBwjpTWsM9NnnbJF3k0aNcM9bDzNHEgdbfOFBNr+bDhWCwOF5PM
daLjC4rzRjhNKtlzd2efShMJC3C8d+BUm5cmEKuYMYpFHm3XVroq323qq3SLzBKd
l+P7nPGZmBy667hC4jtLQQY4/umPuBdRDzFT65YKXdGD/OGphoY6IKC/AwKBgCTX
/owF5o3ySONutQe5UHjc4oH3Lg2YUTrtdbctLZo7vERLMSEWdMeGS+GNynjzsvFG
cp+O7CTw0YCRZ0R/C4axnK2QJoSgDMWoCyU7pBqGRAHa1qrZLX0WnN5NJthAUSNE
HKpkx0btmuL6YzUf2MRco9XbzMUy02iM/aATuZi/AoGBANb5W+t+xuLQBGKYNRfm
l9NWfSzmmXI6UAr3R45NbnL9SrWWlcdk9dh8Q/6a/F98leVK6LTk75pLVOpJCa+B
14hQWnNhg6gQp8EYu4JLc/+7pnIpA1dlmH1+F2BUVW0A2I9k6q0rrPsbiY1PkbhI
VGUUKDOvpxl6Vb9CCCxq25u9
-----END PRIVATE KEY-----
"""


def run_server(host: str, port: int, timeout_s: int) -> int:
    with tempfile.TemporaryDirectory() as tmp:
        tmp_path = Path(tmp)
        cert_path = tmp_path / "cert.pem"
        key_path = tmp_path / "key.pem"
        cert_path.write_text(CERT_PEM, encoding="utf-8")
        key_path.write_text(KEY_PEM, encoding="utf-8")

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

