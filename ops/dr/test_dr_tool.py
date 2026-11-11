import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from erasure_journal import ErasureJournal, pseudonymous_subject_ref
from recovery_report import validate


TOOL = Path(__file__).with_name("hellodoctor_dr.py")


class DisasterRecoveryToolTests(unittest.TestCase):
    def run_tool(self, *args, env=None):
        merged = os.environ.copy()
        if env:
            merged.update(env)
        return subprocess.run(
            [sys.executable, str(TOOL), *args],
            text=True,
            capture_output=True,
            env=merged,
        )

    def test_restore_plan_requires_owned_disposable_target(self):
        with tempfile.TemporaryDirectory() as root:
            target = Path(root) / "restore"
            target.mkdir()
            result = self.run_tool("restore-plan", str(target), env={"HELLODOCTOR_DR_ROOT": root})
            self.assertNotEqual(result.returncode, 0)
            (target / ".hellodoctor-disposable-target").write_text(
                "HELLODOCTOR-DISPOSABLE-TARGET-V1\n", encoding="utf-8"
            )
            result = self.run_tool("restore-plan", str(target), env={"HELLODOCTOR_DR_ROOT": root})
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(result.stdout)["mode"], "dry-run")

    def test_restore_cannot_use_unmarked_or_external_target(self):
        with tempfile.TemporaryDirectory() as root, tempfile.TemporaryDirectory() as other:
            target = Path(root) / "restore"
            target.mkdir()
            result = self.run_tool("validate-target", str(other), env={"HELLODOCTOR_DR_ROOT": root})
            self.assertNotEqual(result.returncode, 0)
            result = self.run_tool("restore-db", str(target), "missing.dump", "--confirm-disposable", env={"HELLODOCTOR_DR_ROOT": root})
            self.assertNotEqual(result.returncode, 0)

    def test_recovery_check_fails_closed(self):
        result = self.run_tool("recovery-check", env={"RECOVERY_MODE": "false"})
        self.assertNotEqual(result.returncode, 0)
        result = self.run_tool(
            "recovery-check",
            env={
                "RECOVERY_MODE": "true",
                "STORAGE_ENCRYPTION_KEY_B64": "synthetic",
                "RECOVERY_OPERATOR_TOKEN_SHA256": "a" * 64,
            },
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(json.loads(result.stdout)["safe_mode"])

    def test_object_manifest_requires_recovery_metadata_and_safe_keys(self):
        with tempfile.TemporaryDirectory() as root:
            manifest = Path(root) / "objects.json"
            row = {
                "record_id": "synthetic-record",
                "container": "clinical-private",
                "object_key": "clinical/synthetic.bin",
                "sha256": "a" * 64,
                "encryption_key_id": "key-v1",
                "retention_state": "active",
            }
            manifest.write_text(json.dumps({"objects": [row]}), encoding="utf-8")
            result = self.run_tool("object-manifest", str(manifest))
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(result.stdout)["objects"], 1)
            row["object_key"] = "../clinical/escape"
            manifest.write_text(json.dumps({"objects": [row]}), encoding="utf-8")
            result = self.run_tool("object-manifest", str(manifest))
            self.assertNotEqual(result.returncode, 0)

    def test_erasure_journal_is_append_only_integrity_protected_and_fail_closed(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "erasure.journal"
            journal = ErasureJournal(path, b"k" * 32, anchor_key=b"a" * 32, key_version="key-v1")
            subject = pseudonymous_subject_ref("synthetic-user", "hellodoctor-test")
            journal.append("event-1", subject, "COMPLETED", "clinical-retention")
            self.assertEqual(journal.reconcile({subject})[subject], "ERASED_OR_RETAINED")
            original = path.read_text(encoding="utf-8")
            path.write_text(original.replace("COMPLETED", "PENDING"), encoding="utf-8")
            with self.assertRaises(ValueError):
                journal.read_verified()

    def test_erasure_journal_anchor_detects_truncation_reordering_and_missing_checkpoint(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "erasure.journal"
            journal = ErasureJournal(path, b"k" * 32, anchor_key=b"a" * 32, key_version="key-v2")
            subject = pseudonymous_subject_ref("synthetic-user", "hellodoctor-test")
            journal.append("event-1", subject, "PENDING", "clinical-retention")
            journal.append("event-2", subject, "COMPLETED", "clinical-retention")
            lines = path.read_text(encoding="utf-8").splitlines()
            path.write_text(lines[0] + "\n", encoding="utf-8")
            with self.assertRaises(ValueError):
                journal.read_verified()

            path.write_text("\n".join(reversed(lines)) + "\n", encoding="utf-8")
            with self.assertRaises(ValueError):
                journal.read_verified()

            path.write_text("\n".join(lines) + "\n", encoding="utf-8")
            journal.anchor_path.unlink()
            with self.assertRaises(ValueError):
                journal.reconcile({subject})

    def test_recovery_report_requires_durable_phase_measurements(self):
        report = {
            "incident_id": "synthetic-7c1b",
            "fixture_id": "fixture-1",
            "source_database": "pg-source",
            "target_database": "pg-target",
            "selected_recovery_timestamp": "2026-10-29T00:00:00Z",
            "latest_restored_transaction_timestamp": "2026-10-28T23:59:00Z",
            "durations_ms": {
                "backup_creation": 1,
                "database_restore": 2,
                "object_restore": 3,
                "application_reconstruction": 4,
                "reconciliation": 5,
                "total_validated_recovery": 6,
            },
            "artifact_version": "synthetic",
            "migration_version": "20261130000003",
            "checks": [{"name": "recovery-mode", "status": "PASSED", "evidence": "blocked"}],
            "outcome": "PARTIAL",
        }
        validate(report)
        report["durations_ms"].pop("database_restore")
        with self.assertRaises(ValueError):
            validate(report)


if __name__ == "__main__":
    unittest.main()
