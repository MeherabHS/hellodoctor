#!/usr/bin/env python3
"""Validate and render a local disaster-recovery acceptance report.

The input is intentionally an operator-produced JSON document. This tool does
not connect to databases or cloud providers and never accepts credentials.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path


REQUIRED = {
    "incident_id",
    "fixture_id",
    "source_database",
    "target_database",
    "selected_recovery_timestamp",
    "latest_restored_transaction_timestamp",
    "durations_ms",
    "artifact_version",
    "migration_version",
    "checks",
    "outcome",
}
REQUIRED_DURATIONS = {
    "backup_creation",
    "database_restore",
    "object_restore",
    "application_reconstruction",
    "reconciliation",
    "total_validated_recovery",
}


def validate(payload: dict[str, object]) -> None:
    missing = REQUIRED.difference(payload)
    if missing:
        raise ValueError(f"recovery report is missing fields: {', '.join(sorted(missing))}")
    durations = payload["durations_ms"]
    if not isinstance(durations, dict) or REQUIRED_DURATIONS.difference(durations):
        raise ValueError("durations_ms must contain every measured recovery phase")
    for name, value in durations.items():
        if value is not None and (not isinstance(value, (int, float)) or value < 0):
            raise ValueError(f"duration {name} must be a non-negative number or null")
    checks = payload["checks"]
    if not isinstance(checks, list) or not checks:
        raise ValueError("checks must be a non-empty list")
    for check in checks:
        if not isinstance(check, dict) or not {"name", "status", "evidence"}.issubset(check):
            raise ValueError("each check requires name, status and evidence")
        if check["status"] not in {"PASSED", "FAILED", "NOT_EXECUTED", "PARTIAL"}:
            raise ValueError("unsupported recovery check status")
    if payload["outcome"] not in {"PASSED", "PARTIAL", "FAILED"}:
        raise ValueError("outcome must be PASSED, PARTIAL or FAILED")


def render_markdown(payload: dict[str, object]) -> str:
    durations = payload["durations_ms"]
    checks = payload["checks"]
    assert isinstance(durations, dict)
    assert isinstance(checks, list)
    lines = [
        f"# Local Recovery Acceptance {payload['incident_id']}",
        "",
        f"- Fixture: `{payload['fixture_id']}`",
        f"- Source database: `{payload['source_database']}`",
        f"- Target database: `{payload['target_database']}`",
        f"- Selected recovery timestamp: `{payload['selected_recovery_timestamp']}`",
        f"- Latest restored transaction: `{payload['latest_restored_transaction_timestamp']}`",
        f"- Artifact version: `{payload['artifact_version']}`",
        f"- Migration version: `{payload['migration_version']}`",
        "",
        "## Durations (milliseconds)",
        "",
    ]
    for name in sorted(REQUIRED_DURATIONS):
        lines.append(f"- {name}: `{durations[name]}`")
    lines += ["", "## Checks", ""]
    for check in checks:
        lines.append(f"- **{check['status']}** {check['name']}: {check['evidence']}")
    lines += ["", f"## Outcome: {payload['outcome']}", ""]
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("--markdown", type=Path)
    args = parser.parse_args()
    try:
        payload = json.loads(args.source.read_text(encoding="utf-8-sig"))
        if not isinstance(payload, dict):
            raise ValueError("recovery report must be a JSON object")
        validate(payload)
    except (OSError, json.JSONDecodeError, ValueError) as exc:
        raise SystemExit(f"ERROR: {exc}") from exc
    if args.markdown:
        args.markdown.write_text(render_markdown(payload), encoding="utf-8")
    print(json.dumps({"valid": True, "outcome": payload["outcome"]}, indent=2))


if __name__ == "__main__":
    main()
