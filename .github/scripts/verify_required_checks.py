#!/usr/bin/env python3
"""Verify that every required named CI check succeeded for a commit,
scoped to which component(s) this dispatch is actually publishing.

Extracted, unmodified-in-behavior, from the inline bash/python block
previously embedded in `.github/workflows/arm64-ghcr-publish.yml`'s
`verify-revision` job — moved out so it can be unit tested (see
`test_verify_required_checks.py`) independently of the GitHub Actions
runner and the `gh` CLI.

Reads the check-runs API response (`GET /repos/{owner}/{repo}/commits/{sha}/check-runs`)
as JSON from stdin and exits 0 if every check required by the selected
publish_* component(s) is present with conclusion "success", else exits 1
with a `::error::` annotation naming exactly what's missing or failed.

COMPONENT-AWARE REQUIRED CHECKS (added): previously this always demanded
all five checks below regardless of which publish_* inputs were selected —
meaning a Doctor-Web-only dispatch (publish_backend=false) could never pass
verify-revision, because "Apply migrations to a fresh Postgres and run the
backend test suite" only ever runs for a commit that touches
services/backend/** (backend-migrations.yml's own path filter), and a
Doctor-Web-only source revision legitimately never touches that path. The
four ARM64-build checks (Rust tests.../Admin Web.../Doctor Web.../pgBackRest...)
all come from the single arm64-compatibility.yml workflow, which triggers
together whenever ANY of services/backend/**, apps/admin-web/**,
apps/doctor-web/**, or deploy/single-vm/** changes — so in practice those
four are usually all present together regardless of which single path
changed, and only the backend-migrations.yml check is genuinely
component-scoped today. The matrix below is still defined per-component
(not just carving out the migrations check) so the same logic correctly
narrows in any future case where the ARM64-build checks also become
path-scoped per component.
"""
import json
import os
import sys

CHECK_MATRIX = {
    "publish_backend": [
        "Rust tests and backend ARM64 image",
        "Apply migrations to a fresh Postgres and run the backend test suite",
    ],
    "publish_admin_web": ["Admin Web ARM64 image"],
    "publish_doctor_web": ["Doctor Web ARM64 image"],
    "publish_pgbackrest": ["pgBackRest ARM64 image"],
}

# Preserved for backward compatibility with existing tests/callers that
# import this name directly: the full union of every component's checks,
# in the same stable order as before.
REQUIRED_CHECKS = [
    name for names in CHECK_MATRIX.values() for name in names
]


def required_checks_for(selected):
    """selected: {"publish_backend": bool, "publish_admin_web": bool,
    "publish_doctor_web": bool, "publish_pgbackrest": bool}.

    Returns the union of required check names for whichever components are
    selected, in CHECK_MATRIX's stable order, de-duplicated. Selecting
    multiple components requires the union of their checks (e.g.
    publish_backend + publish_doctor_web together still require the
    migration check, since it's backend's).
    """
    required = []
    for key, names in CHECK_MATRIX.items():
        if not selected.get(key):
            continue
        for name in names:
            if name not in required:
                required.append(name)
    return required


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


def selected_components(env=os.environ):
    def flag(name):
        return env.get(name, "").strip().lower() == "true"

    return {
        "publish_backend": flag("PUBLISH_BACKEND"),
        "publish_admin_web": flag("PUBLISH_ADMIN_WEB"),
        "publish_doctor_web": flag("PUBLISH_DOCTOR_WEB"),
        "publish_pgbackrest": flag("PUBLISH_PGBACKREST"),
    }


def main():
    payload = json.load(sys.stdin)
    check_runs = payload.get("check_runs", [])
    selected = selected_components()
    # Fail closed: a dispatch that selects nothing to publish has nothing
    # to verify either, and must never fall through to an empty required
    # list (which would trivially "pass" with zero checks demanded).
    if not any(selected.values()):
        print("::error::no publish_* component was selected - nothing to verify or publish")
        sys.exit(1)
    required = required_checks_for(selected)
    ok, reason = evaluate(check_runs, required)
    print(f"::error::{reason}" if not ok else reason)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
