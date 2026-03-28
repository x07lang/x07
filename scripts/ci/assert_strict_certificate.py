#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Assert strict strong-profile certificate invariants and re-run proof checks."
    )
    parser.add_argument("--cert", required=True, help="Path to certificate.json")
    parser.add_argument("--x07-bin", required=True, help="Path to x07 binary")
    parser.add_argument("--label", required=True, help="Short label for diagnostics")
    parser.add_argument(
        "--cwd",
        required=True,
        help="Project directory to use when re-running x07 prove check",
    )
    parser.add_argument(
        "--require-entry-formally-proved",
        action="store_true",
        help="Reject certificates whose operational entry body is not formally proved.",
    )
    return parser.parse_args()


def fail(code: str, message: str) -> None:
    print(f"{code}: {message}", file=sys.stderr)
    raise SystemExit(1)


def main() -> int:
    args = parse_args()
    cert_path = Path(args.cert)
    x07_bin = Path(args.x07_bin)
    cwd = Path(args.cwd)
    label = args.label

    cert = json.loads(cert_path.read_text(encoding="utf-8"))

    entry = cert.get("entry")
    operational_entry = cert.get("operational_entry_symbol")
    if entry != operational_entry:
        fail(
            "X07REL_ESURROGATE_ENTRY",
            f"entry {entry!r} must match operational_entry_symbol {operational_entry!r}",
        )

    if cert.get("verdict") != "accepted":
        fail(label, f"certificate verdict must be 'accepted', got {cert.get('verdict')!r}")

    if cert.get("accepted_depends_on_bounded_proof"):
        fail("X07REL_EBOUNDED_PROOF", "accepted certificate must not depend on bounded proof")

    if cert.get("accepted_depends_on_dev_only_assumption"):
        fail(
            "X07REL_EDEV_ONLY_ASSUMPTION",
            "accepted certificate must not depend on developer-only proof assumptions",
        )

    if cert.get("imported_summary_inventory"):
        fail(
            "X07REL_ECOVERAGE_ONLY_IMPORT",
            "strong certificate must not accept coverage-only imported summaries",
        )

    proof_inventory = cert.get("proof_inventory")
    if not isinstance(proof_inventory, list) or not proof_inventory:
        fail(label, "strong certificate produced no proof inventory")

    proved_symbol_count = cert.get("proved_symbol_count")
    if proved_symbol_count != len(proof_inventory):
        fail(
            label,
            f"proved_symbol_count {proved_symbol_count!r} must equal proof inventory size {len(proof_inventory)}",
        )

    if cert.get("formal_verification_scope") not in {
        "entry_body",
        "whole_certifiable_graph",
    }:
        fail(
            label,
            "formal_verification_scope must reflect accepted operational-entry proof coverage",
        )

    if args.require_entry_formally_proved and not cert.get("entry_body_formally_proved"):
        fail(label, "expected entry_body_formally_proved=true for strong-profile certificate")

    claims = cert.get("claims")
    if not isinstance(claims, list) or "certificate_includes_formal_proof" not in claims:
        fail(label, "accepted strong certificate must include certificate_includes_formal_proof")
    if args.require_entry_formally_proved and "operational_entry_formally_proved" not in claims:
        fail(label, "accepted strong certificate must include operational_entry_formally_proved")

    operational_refs = cert.get("operational_entry_proof_inventory_refs")
    if args.require_entry_formally_proved and not operational_refs:
        fail(label, "operational entry proof inventory refs must not be empty")

    for item in proof_inventory:
        symbol = item.get("symbol")
        proof_object = item.get("proof_object") or {}
        proof_check = item.get("proof_check_report") or {}
        proof_path = proof_object.get("path")
        proof_check_path = proof_check.get("path")
        if not proof_path:
            fail(label, f"missing proof object for {symbol!r}")
        if not proof_check_path:
            fail(label, f"missing proof-check report for {symbol!r}")
        if item.get("proof_check_result") != "accepted":
            fail(
                label,
                f"proof-check result for {symbol!r} must be 'accepted', got {item.get('proof_check_result')!r}",
            )
        if not item.get("proof_check_checker"):
            fail(label, f"missing proof-check checker id for {symbol!r}")
        if not item.get("proof_object_digest"):
            fail(label, f"missing proof object digest for {symbol!r}")
        try:
            subprocess.run(
                [str(x07_bin), "prove", "check", "--proof", proof_path],
                check=True,
                cwd=cwd,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.STDOUT,
            )
        except subprocess.CalledProcessError as exc:
            fail(
                label,
                f"x07 prove check rejected proof object for {symbol!r} with exit code {exc.returncode}",
            )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
