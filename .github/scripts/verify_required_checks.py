#!/usr/bin/env python3
"""Verify that every required named CI check succeeded for a commit.

Extracted, unmodified-in-behavior, from the inline bash/python block
previously embedded in `.github/workflows/arm64-ghcr-publish.yml`'s
`verify-revision` job — moved out so it can be unit tested (see
`test_verify_required_checks.py`) independently of the GitHub Actions
runner and the `gh` CLI.

Reads the check-runs API response (`GET /repos/{owner}/{repo}/commits/{sha}/check-runs`)
as JSON from stdin and exits 0 if every required check is present with
conclusion "success", else exits 1 with a `::error::` annotation naming
exactly what's missing or failed.
"""
import json
import sys

REQUIRED_CHECKS = [
    "Rust tests and backend ARM64 image",
    "Admin Web ARM64 image",
    "Doctor Web ARM64 image",
    "pgBackRest ARM64 image",
    "Apply migrations to a fresh Postgres and run the backend test suite",
]


def evaluate(check_runs, required=REQUIRED_CHECKS):
    """check_runs: list of {"name": str, "conclusion": str|None, ...}.

    Returns (ok: bool, reason: str).
    """
    runs = {r["name"]: r.get("conclusion") for r in check_runs}
    missing = [n for n in required if n not in runs]
    failed = [n for n in required if n not in missing and runs.get(n) != "success"]
    if missing:
        return False, f"required check(s) not found for this revision: {missing}"
    if failed:
        return False, (
            "required check(s) did not succeed for this revision: "
            f"{[(n, runs[n]) for n in failed]}"
        )
    return True, "All required checks succeeded for the requested revision."


def main():
    payload = json.load(sys.stdin)
    check_runs = payload.get("check_runs", [])
    ok, reason = evaluate(check_runs)
    print(f"::error::{reason}" if not ok else reason)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
