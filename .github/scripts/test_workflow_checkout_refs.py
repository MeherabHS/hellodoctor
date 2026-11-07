"""Static regression test for publication failure #4.

`verify-revision` calls verify_required_checks.py and
verify_trusted_revision.py, which only exist in the repository checkout a
job actually performs. The bug: `verify-revision` had NO checkout step at
all, so those scripts could never be found, regardless of which
`source_revision` was requested.

This test parses the real workflow YAML (not a copy or a fixture) and
pins down the fix structurally, so a future edit can't silently
reintroduce either direction of the mistake:

  1. `verify-revision` must have a checkout step, with NO `ref:` override
     — it must always load the verifier scripts from the revision the
     workflow itself is running from (trusted), never from the
     caller-supplied, untrusted `source_revision` input (requirement #4
     of the fix: never execute verification logic from an arbitrary PR
     head or application commit).
  2. Every `publish-*` job must still check out `${{ inputs.source_revision
     }}` explicitly — unchanged by this fix, these jobs genuinely need to
     build from the requested application revision, not the workflow's
     own.

Requires PyYAML (already present in this project's dev environment and on
GitHub's ubuntu-latest runners by default).
"""
import os
import unittest

import yaml

WORKFLOW_PATH = os.path.join(
    os.path.dirname(__file__), "..", "workflows", "arm64-ghcr-publish.yml"
)

PUBLISH_JOBS = [
    "publish-backend",
    "publish-admin-web",
    "publish-doctor-web",
    "publish-pgbackrest",
]


def load_workflow():
    with open(WORKFLOW_PATH, encoding="utf-8") as f:
        return yaml.safe_load(f)


class WorkflowCheckoutRefsTests(unittest.TestCase):
    def setUp(self):
        self.data = load_workflow()

    def test_verify_revision_has_a_checkout_step(self):
        steps = self.data["jobs"]["verify-revision"]["steps"]
        checkout_steps = [s for s in steps if s.get("uses", "").startswith("actions/checkout")]
        self.assertEqual(
            len(checkout_steps),
            1,
            "verify-revision must have exactly one checkout step so its "
            ".github/scripts/*.py calls can find the scripts on disk",
        )

    def test_verify_revision_checkout_has_no_source_revision_ref_override(self):
        steps = self.data["jobs"]["verify-revision"]["steps"]
        checkout = next(s for s in steps if s.get("uses", "").startswith("actions/checkout"))
        self.assertNotIn(
            "with",
            checkout,
            "verify-revision's checkout must never override `ref` to the "
            "untrusted source_revision input - it must always load the "
            "verifier scripts from the trusted workflow-owning revision",
        )

    def test_verify_revision_checkout_runs_before_the_verification_steps(self):
        steps = self.data["jobs"]["verify-revision"]["steps"]
        checkout_index = next(
            i for i, s in enumerate(steps) if s.get("uses", "").startswith("actions/checkout")
        )
        script_step_indices = [
            i
            for i, s in enumerate(steps)
            if "verify_required_checks.py" in s.get("run", "")
            or "verify_trusted_revision.py" in s.get("run", "")
        ]
        self.assertTrue(script_step_indices, "expected to find the script-calling steps")
        self.assertTrue(
            all(checkout_index < i for i in script_step_indices),
            "checkout must run before any step that invokes a .github/scripts/*.py file",
        )

    def test_every_publish_job_checks_out_the_requested_source_revision(self):
        for job_name in PUBLISH_JOBS:
            with self.subTest(job=job_name):
                steps = self.data["jobs"][job_name]["steps"]
                checkout = steps[0]
                self.assertTrue(checkout.get("uses", "").startswith("actions/checkout"))
                self.assertEqual(
                    checkout.get("with", {}).get("ref"),
                    "${{ inputs.source_revision }}",
                    f"{job_name} must build from the exact requested "
                    "source_revision, unchanged by this fix",
                )


if __name__ == "__main__":
    unittest.main()
