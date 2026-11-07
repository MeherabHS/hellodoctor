import unittest

from verify_required_checks import REQUIRED_CHECKS, evaluate

ALL_SUCCESS = [{"name": name, "conclusion": "success"} for name in REQUIRED_CHECKS]


class VerifyRequiredChecksTests(unittest.TestCase):
    def test_all_required_checks_succeeded_passes(self):
        ok, reason = evaluate(ALL_SUCCESS)
        self.assertTrue(ok, reason)

    def test_extra_unrelated_checks_are_ignored(self):
        """Checks like 'Secret scanning' or 'Rust dependency vulnerability
        scan' are real check-runs on the same commit but are not in the
        required list, and must not affect the outcome either way."""
        runs = ALL_SUCCESS + [
            {"name": "Secret scanning (git history + working tree)", "conclusion": "success"},
            {"name": "Rust dependency vulnerability scan", "conclusion": "failure"},
        ]
        ok, reason = evaluate(runs)
        self.assertTrue(ok, reason)

    def test_missing_required_check_fails(self):
        runs = [r for r in ALL_SUCCESS if r["name"] != "pgBackRest ARM64 image"]
        ok, reason = evaluate(runs)
        self.assertFalse(ok)
        self.assertIn("not found", reason)
        self.assertIn("pgBackRest ARM64 image", reason)

    def test_failed_required_check_fails(self):
        runs = [
            {"name": r["name"], "conclusion": r["conclusion"]}
            for r in ALL_SUCCESS
        ]
        for r in runs:
            if r["name"] == "Rust tests and backend ARM64 image":
                r["conclusion"] = "failure"
        ok, reason = evaluate(runs)
        self.assertFalse(ok)
        self.assertIn("did not succeed", reason)
        self.assertIn("Rust tests and backend ARM64 image", reason)

    def test_in_progress_required_check_fails(self):
        """A check that hasn't concluded yet (conclusion is null/None while
        running) must not be treated as success."""
        runs = [dict(r) for r in ALL_SUCCESS]
        runs[0]["conclusion"] = None
        ok, reason = evaluate(runs)
        self.assertFalse(ok)


if __name__ == "__main__":
    unittest.main()
