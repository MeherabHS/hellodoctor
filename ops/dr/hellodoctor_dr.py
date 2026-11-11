#!/usr/bin/env python3
"""Guarded, provider-neutral disaster-recovery helper for HelloDoctor.

The default operations are read-only.  Any operation that can overwrite a
database requires a target directory below HELLODOCTOR_DR_ROOT containing the
operator-created .hellodoctor-disposable-target marker and an explicit
confirmation flag.  This intentionally does not know production connection
strings or cloud credentials.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path


MARKER = ".hellodoctor-disposable-target"
PROTECTED_NAMES = {
    ".env",
    ".env.production",
    "Cargo.lock",
    "pubspec.lock",
}


def fail(message: str) -> "NoReturn":
    print(f"ERROR: {message}", file=sys.stderr)
    raise SystemExit(2)


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()


def dr_root() -> Path:
    value = os.environ.get("HELLODOCTOR_DR_ROOT")
    if not value:
        fail("HELLODOCTOR_DR_ROOT is required; no implicit restore target is allowed")
    root = Path(value).resolve()
    if not root.exists() or not root.is_dir():
        fail("HELLODOCTOR_DR_ROOT must be an existing directory")
    return root


def disposable_target(value: str) -> Path:
    root = dr_root()
    target = Path(value).resolve()
    try:
        target.relative_to(root)
    except ValueError:
        fail("restore target is outside HELLODOCTOR_DR_ROOT")
    if target == root or not target.is_dir():
        fail("restore target must be an existing child directory")
    marker = target / MARKER
    if not marker.is_file():
        fail(f"restore target is not marked disposable ({MARKER} missing)")
    if marker.read_text(encoding="utf-8").strip() != "HELLODOCTOR-DISPOSABLE-TARGET-V1":
        fail("restore target marker has an unexpected value")
    return target


def require_confirmation(args: argparse.Namespace) -> None:
    if not getattr(args, "confirm_disposable", False):
        fail("destructive restore requires --confirm-disposable")


def command_output(command: list[str]) -> str | None:
    try:
        result = subprocess.run(command, capture_output=True, text=True, timeout=10)
    except (OSError, subprocess.SubprocessError):
        return None
    if result.returncode != 0:
        return None
    return result.stdout.strip()


def inventory(_: argparse.Namespace) -> None:
    root = Path.cwd()
    entries: dict[str, object] = {
        "generated_at": utc_now(),
        "workspace": str(root),
        "configuration": {
            "environment_set": bool(os.environ.get("ENVIRONMENT")),
            "storage_provider": os.environ.get("STORAGE_PROVIDER", "disabled"),
            "recovery_mode": os.environ.get("RECOVERY_MODE", "false"),
            "encryption_key_present": bool(os.environ.get("STORAGE_ENCRYPTION_KEY_B64")),
        },
        "docker": {},
    }
    for name, command in {
        "system_df": ["docker", "system", "df"],
        "containers": ["docker", "ps", "-a", "--format", "{{.ID}} {{.Names}} {{.Status}}"],
        "volumes": ["docker", "volume", "ls", "--format", "{{.Name}}"],
    }.items():
        entries["docker"][name] = command_output(command)  # type: ignore[index]
    print(json.dumps(entries, indent=2))


def hash_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def manifest(args: argparse.Namespace) -> None:
    source = Path(args.source).resolve()
    if not source.exists():
        fail("manifest source does not exist")
    rows: list[dict[str, object]] = []
    candidates = [source] if source.is_file() else source.rglob("*")
    for path in candidates:
        if not path.is_file() or path.name in PROTECTED_NAMES:
            continue
        if any(part in {".git", "node_modules", "target", "build", ".dart_tool"} for part in path.parts):
            continue
        rows.append({"path": str(path.relative_to(source) if source.is_dir() else path.name), "size": path.stat().st_size, "sha256": hash_file(path)})
    print(json.dumps({"generated_at": utc_now(), "source": str(source), "files": rows}, indent=2))


def object_manifest(args: argparse.Namespace) -> None:
    """Validate a DB/object reconciliation manifest without contacting storage."""
    path = Path(args.source).resolve()
    if not path.is_file():
        fail("object manifest does not exist")
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        fail("object manifest is not valid JSON")
    rows = payload.get("objects") if isinstance(payload, dict) else None
    if not isinstance(rows, list):
        fail("object manifest must contain an objects array")
    required = {"record_id", "container", "object_key", "sha256", "encryption_key_id", "retention_state"}
    allowed = {"clinical-private", "article-media"}
    for index, row in enumerate(rows, start=1):
        if not isinstance(row, dict) or not required.issubset(row):
            fail(f"object manifest row {index} is missing required recovery metadata")
        if row["container"] not in allowed or not isinstance(row["object_key"], str) or ".." in Path(row["object_key"]).parts:
            fail(f"object manifest row {index} has an unsafe container or object key")
        if not isinstance(row["sha256"], str) or len(row["sha256"]) != 64:
            fail(f"object manifest row {index} has an invalid SHA-256")
    print(json.dumps({"valid": True, "objects": len(rows), "source": str(path)}, indent=2))


def validate_target(args: argparse.Namespace) -> None:
    print(json.dumps({"target": str(disposable_target(args.target)), "disposable": True}, indent=2))


def restore_plan(args: argparse.Namespace) -> None:
    target = disposable_target(args.target)
    print(json.dumps({
        "generated_at": utc_now(),
        "mode": "dry-run",
        "target": str(target),
        "steps": [
            "validate approved recovery point and encryption-key custody",
            "restore PostgreSQL to this disposable target only",
            "verify roles, RLS, functions and migrations",
            "reconcile Blob object manifest without restoring every deleted object",
            "reconcile erasure, finance and notification external-operation journals",
            "run application health and security checks",
            "keep RECOVERY_MODE=true until an authorized operator approves cutover",
        ],
        "database_dsn_source": "PGTARGET_DSN environment variable (value never printed)",
    }, indent=2))


def restore_db(args: argparse.Namespace) -> None:
    require_confirmation(args)
    target = disposable_target(args.target)
    dump = Path(args.dump).resolve()
    if not dump.is_file():
        fail("database dump does not exist")
    dsn = os.environ.get("PGTARGET_DSN")
    if not dsn:
        fail("PGTARGET_DSN must be supplied through the environment, never as a command argument")
    pg_restore = shutil.which("pg_restore")
    if not pg_restore:
        fail("pg_restore is not installed")
    # The target marker is the ownership boundary.  --exit-on-error prevents a
    # partial restore from being reported as successful.
    result = subprocess.run([pg_restore, "--exit-on-error", "--no-owner", "--dbname", dsn, str(dump)], cwd=target)
    if result.returncode != 0:
        fail(f"pg_restore failed with exit code {result.returncode}")
    print(json.dumps({"restored": True, "target": str(target), "dump": str(dump)}, indent=2))


def recovery_check(args: argparse.Namespace) -> None:
    if args.mode != "recovery":
        fail("only recovery mode is supported by this check")
    required = ["RECOVERY_MODE", "STORAGE_ENCRYPTION_KEY_B64", "RECOVERY_OPERATOR_TOKEN_SHA256"]
    missing = [name for name in required if not os.environ.get(name)]
    digest = os.environ.get("RECOVERY_OPERATOR_TOKEN_SHA256", "")
    if missing or os.environ.get("RECOVERY_MODE", "").lower() != "true" or len(digest) != 64 or any(char not in "0123456789abcdefABCDEF" for char in digest):
        fail("recovery check requires RECOVERY_MODE=true, encryption-key availability and a 64-hex operator-token digest")
    print(json.dumps({"safe_mode": True, "outbound_operations": "disabled_by_application", "secrets_checked": ["present"]}, indent=2))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("inventory", help="read-only runtime and Docker inventory").set_defaults(func=inventory)
    p_manifest = sub.add_parser("manifest", help="hash a recovery artifact tree")
    p_manifest.add_argument("source")
    p_manifest.set_defaults(func=manifest)
    p_objects = sub.add_parser("object-manifest", help="validate DB/blob recovery metadata")
    p_objects.add_argument("source")
    p_objects.set_defaults(func=object_manifest)
    p_target = sub.add_parser("validate-target", help="validate a disposable restore directory")
    p_target.add_argument("target")
    p_target.set_defaults(func=validate_target)
    p_plan = sub.add_parser("restore-plan", help="emit a guarded dry-run plan")
    p_plan.add_argument("target")
    p_plan.set_defaults(func=restore_plan)
    p_restore = sub.add_parser("restore-db", help="restore a dump into a marked disposable target")
    p_restore.add_argument("target")
    p_restore.add_argument("dump")
    p_restore.add_argument("--confirm-disposable", action="store_true")
    p_restore.set_defaults(func=restore_db)
    p_check = sub.add_parser("recovery-check", help="validate emergency-mode prerequisites")
    p_check.add_argument("--mode", default="recovery")
    p_check.set_defaults(func=recovery_check)
    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
