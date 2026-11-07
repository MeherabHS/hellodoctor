import unittest

from verify_trusted_revision import evaluate

MERGE_COMMIT = "b9d72deab79fe868b5d9af561bda341a2f2a8af6"
OLDER_MAIN_COMMIT = "e3488e54d23ca520f35e49c9f98b8a9290a5a15f"
OLDER_APPLICATION_COMMIT_PREDATING_VERIFIER_SCRIPTS = (
    "b9d72deab79fe868b5d9af561bda341a2f2a8af6"
)
STALE_PR_HEAD = "f801f7b3a1b09e33a1cbcd1f0b23d30cff4af398"
SUPERSEDING_FORCE_PUSH_SHA = "deadbeef00112233445566778899aabbccddeef"
CURRENT_OPEN_PR_HEAD = "1111111111111111111111111111111111aaaa"
ARBITRARY_SHA = "0000000000000000000000000000000000dead"


class VerifyTrustedRevisionTests(unittest.TestCase):
    def test_merged_pr_commit_that_is_the_current_main_tip_passes(self):
        """The exact bug report: a just-merged PR's merge commit, which IS
        the current default-branch tip, must be accepted — even though the
        PR's own head.sha (the pre-merge branch tip) is a different SHA and
        is no longer associated with this commit at all."""
        ok, reason = evaluate(
            compare_status="identical",
            prs=[{"head": {"sha": STALE_PR_HEAD}}],
            source_revision=MERGE_COMMIT,
        )
        self.assertTrue(ok, reason)
        self.assertIn("ancestor of (or equal to) the default branch", reason)

    def test_merged_pr_commit_with_no_associated_prs_still_passes(self):
        """Ancestry of the default branch is sufficient on its own; it must
        not additionally require a PR association, since GitHub's
        commits/{sha}/pulls can legitimately return an empty list for an
        old, merged commit long after its PR closed."""
        ok, reason = evaluate(
            compare_status="identical", prs=[], source_revision=MERGE_COMMIT
        )
        self.assertTrue(ok, reason)

    def test_older_already_merged_commit_still_on_main_passes(self):
        """An earlier commit that is still part of main's history (not the
        current tip, but reachable from it) must also be accepted —
        'behind' the default branch tip still means 'on' the default
        branch."""
        ok, reason = evaluate(
            compare_status="behind", prs=[], source_revision=OLDER_MAIN_COMMIT
        )
        self.assertTrue(ok, reason)

    def test_older_application_commit_predating_the_verifier_scripts_still_eligible(self):
        """Publication failure #4's exact scenario: the requested
        application source_revision is an older, fully-merged commit whose
        own tree predates PR #8 (the PR that introduced
        verify_required_checks.py/verify_trusted_revision.py). This
        function must evaluate trust from the SAME ancestry/PR-head logic
        regardless of what files exist in that commit's own tree - the
        verifier scripts are loaded from the workflow's own (newer,
        trusted) revision by the calling workflow step, never from
        source_revision's tree, so an older commit's missing scripts are
        irrelevant to this function and must not affect its eligibility."""
        ok, reason = evaluate(
            compare_status="behind",
            prs=[],
            source_revision=OLDER_APPLICATION_COMMIT_PREDATING_VERIFIER_SCRIPTS,
        )
        self.assertTrue(ok, reason)

    def test_current_open_pr_head_still_passes(self):
        """Unmerged publication directly from an active PR branch's current
        head must keep working exactly as before this fix."""
        ok, reason = evaluate(
            compare_status="diverged",
            prs=[{"head": {"sha": CURRENT_OPEN_PR_HEAD}}],
            source_revision=CURRENT_OPEN_PR_HEAD,
        )
        self.assertTrue(ok, reason)
        self.assertIn("current head of a tracked pull request", reason)

    def test_stale_pr_head_superseded_by_force_push_is_rejected(self):
        """A commit that used to be a PR's head, but the branch was
        force-pushed past it and it was never merged, must still be
        rejected: it is neither an ancestor of main nor any PR's current
        head."""
        ok, reason = evaluate(
            compare_status="diverged",
            prs=[{"head": {"sha": SUPERSEDING_FORCE_PUSH_SHA}}],
            source_revision=STALE_PR_HEAD,
        )
        self.assertFalse(ok)
        self.assertIn("superseded by a force-push", reason)

    def test_commit_ahead_of_main_but_not_a_pr_head_is_rejected(self):
        """'ahead' means main is an ancestor of the requested commit, not
        the other way around — the requested commit is NOT itself part of
        main's own history and must not be accepted on ancestry grounds."""
        ok, reason = evaluate(
            compare_status="ahead", prs=[], source_revision=ARBITRARY_SHA
        )
        self.assertFalse(ok)

    def test_arbitrary_unassociated_sha_with_no_compare_result_is_rejected(self):
        """A SHA the compare API couldn't resolve at all (e.g. it doesn't
        exist in the repository) must fail closed, not be treated as
        trusted by default."""
        ok, reason = evaluate(
            compare_status=None, prs=[], source_revision=ARBITRARY_SHA
        )
        self.assertFalse(ok)


if __name__ == "__main__":
    unittest.main()
