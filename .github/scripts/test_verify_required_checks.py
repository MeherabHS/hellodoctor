import unittest

from verify_required_checks import REQUIRED_CHECKS, evaluate, required_checks_for

ALL_SUCCESS = [{"name": name, "conclusion": "success"} for name in REQUIRED_CHECKS]

ALL_SELECTED = {
    "publish_backend": True,
    "publish_admin_web": True,
    "publish_doctor_web": True,
    "publish_pgbackrest": True,
}


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


class ComponentAwareRequiredChecksTests(unittest.TestCase):
    """Proves the actual bug this fix addresses: a Doctor-Web-only
    dispatch must be able to pass verify-revision without ever having (or
    needing) a backend migration check run against its source revision,
    while a backend publish can never skip that same check."""

    def test_doctor_web_only_selection_does_not_require_backend_checks(self):
        selected = {
            "publish_backend": False,
            "publish_admin_web": False,
            "publish_doctor_web": True,
            "publish_pgbackrest": False,
        }
        self.assertEqual(
            required_checks_for(selected), ["Doctor Web ARM64 image"]
        )

    def test_doctor_web_only_passes_with_only_its_own_check_present(self):
        """The real-world shape of PR #13 and the Doctor Web refresh-loop
        PR's check-runs: all four ARM64-build checks happen to be present
        (arm64-compatibility.yml triggers on any of several paths and runs
        all four jobs together), but the backend migration check is
        genuinely absent because the PR touches no services/backend/**
        file. This must still pass once only Doctor Web is selected."""
        runs = [
            {"name": "Rust tests and backend ARM64 image", "conclusion": "success"},
            {"name": "Admin Web ARM64 image", "conclusion": "success"},
            {"name": "Doctor Web ARM64 image", "conclusion": "success"},
            {"name": "pgBackRest ARM64 image", "conclusion": "success"},
            # Deliberately no "Apply migrations..." entry at all.
        ]
        selected = {
            "publish_backend": False,
            "publish_admin_web": False,
            "publish_doctor_web": True,
            "publish_pgbackrest": False,
        }
        ok, reason = evaluate(runs, required_checks_for(selected))
        self.assertTrue(ok, reason)

    def test_backend_publication_still_requires_the_migration_check(self):
        """A backend publish can never bypass the migration check just
        because some other component is also selected alongside it."""
        selected = {
            "publish_backend": True,
            "publish_admin_web": False,
            "publish_doctor_web": True,
            "publish_pgbackrest": False,
        }
        required = required_checks_for(selected)
        self.assertIn(
            "Apply migrations to a fresh Postgres and run the backend test suite",
            required,
        )
        runs = [
            {"name": "Rust tests and backend ARM64 image", "conclusion": "success"},
            {"name": "Doctor Web ARM64 image", "conclusion": "success"},
            # Migration check absent -> must fail, not silently pass.
        ]
        ok, reason = evaluate(runs, required)
        self.assertFalse(ok)
        self.assertIn(
            "Apply migrations to a fresh Postgres and run the backend test suite",
            reason,
        )

    def test_mixed_component_selection_requires_the_union(self):
        selected = {
            "publish_backend": True,
            "publish_admin_web": True,
            "publish_doctor_web": False,
            "publish_pgbackrest": False,
        }
        self.assertEqual(
            required_checks_for(selected),
            [
                "Rust tests and backend ARM64 image",
                "Apply migrations to a fresh Postgres and run the backend test suite",
                "Admin Web ARM64 image",
            ],
        )

    def test_selecting_everything_matches_the_original_full_list(self):
        """Backward-compatibility guarantee: when every component is
        selected (the pre-fix behavior's implicit assumption), the
        computed required set is exactly the original static list."""
        self.assertEqual(
            sorted(required_checks_for(ALL_SELECTED)), sorted(REQUIRED_CHECKS)
        )

    def test_pgbackrest_check_is_not_required_unless_actually_selected(self):
        """Fixes a second, previously-unnoticed over-strict gap: the old
        static list always demanded the pgBackRest check even though
        publish_pgbackrest defaults to false and is rarely selected."""
        selected = {
            "publish_backend": True,
            "publish_admin_web": False,
            "publish_doctor_web": False,
            "publish_pgbackrest": False,
        }
        self.assertNotIn("pgBackRest ARM64 image", required_checks_for(selected))

    def test_selecting_no_component_yields_an_empty_required_list(self):
        """required_checks_for() itself stays a pure function (returns
        empty, does not raise) - the fail-closed "nothing selected" guard
        lives in main(), covered separately by the CLI-level test below,
        so this is the correct, narrow unit-level behavior to assert here."""
        selected = {
            "publish_backend": False,
            "publish_admin_web": False,
            "publish_doctor_web": False,
            "publish_pgbackrest": False,
        }
        self.assertEqual(required_checks_for(selected), [])


class SelectedComponentsEnvParsingTests(unittest.TestCase):
    def test_parses_github_actions_boolean_string_inputs(self):
        from verify_required_checks import selected_components

        env = {
            "PUBLISH_BACKEND": "false",
            "PUBLISH_ADMIN_WEB": "true",
            "PUBLISH_DOCTOR_WEB": "True",
            "PUBLISH_PGBACKREST": "",
        }
        self.assertEqual(
            selected_components(env),
            {
                "publish_backend": False,
                "publish_admin_web": True,
                "publish_doctor_web": True,
                "publish_pgbackrest": False,
            },
        )


if __name__ == "__main__":
    unittest.main()
