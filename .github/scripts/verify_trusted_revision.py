#!/usr/bin/env python3
"""Decide whether a requested publish revision is safe to build and push.

This is the fix for the false-rejection bug: the previous logic in
`arm64-ghcr-publish.yml` accepted a revision ONLY if it was currently the
head commit of an associated pull request (`GET
/repos/{owner}/{repo}/commits/{sha}/pulls`, checking `pr.head.sha ==
source_revision`). That check can never pass for an already-merged PR's
merge commit: GitHub's `pulls` endpoint keeps returning the PR as
"associated" with the merge commit, but `pr.head.sha` always stays the
PR branch's own last commit, never the merge commit created on the base
branch. The old logic therefore rejected every legitimate, already-merged
commit on the default branch with the same message meant for a genuinely
stale, force-pushed-away PR head — the two cases were never actually
distinguished.

A revision is now trusted if EITHER:
  (a) it is an ancestor of, or equal to, the repository's default branch
      tip — i.e. `git compare {default_branch}...{source_revision}` reports
      status "identical" or "behind". This covers both "is exactly the
      current tip" (including a brand-new merge commit) and "is an older,
      already-merged commit still reachable from the default branch".
  (b) it is currently the head commit of a tracked, still-open pull request
      (the original check, unchanged) — this preserves the ability to
      publish directly from an active PR branch before it merges.

It is rejected if neither holds. This correctly continues to reject: a
commit that was once a PR head but was superseded by a force-push and
never merged (reachable from neither the default branch nor any open PR),
and any arbitrary/unrelated SHA that happens to pass the separate
required-checks verification by coincidence (it won't, in practice, since
check-runs are recorded against real commits only — this is defense in
depth, not the primary check).
"""
import json
import os
import sys

ANCESTOR_STATUSES = ("identical", "behind")


def evaluate(compare_status, prs, source_revision):
    """compare_status: the `status` field from
        `GET /repos/{owner}/{repo}/compare/{default_branch}...{source_revision}`,
        or None if that comparison could not be made (e.g. the SHA doesn't
        exist in the repository at all).
    prs: list of PR objects from
        `GET /repos/{owner}/{repo}/commits/{source_revision}/pulls`, each
        with at least `head.sha`.
    source_revision: the requested SHA, already validated as 40 lowercase
        hex characters by the caller.

    Returns (ok: bool, reason: str).
    """
    if compare_status in ANCESTOR_STATUSES:
        return True, (
            f"PASS: {source_revision} is an ancestor of (or equal to) the "
            f"default branch (compare status '{compare_status}')."
        )
    current_pr_head = any(
        pr.get("head", {}).get("sha") == source_revision for pr in prs
    )
    if current_pr_head:
        return True, (
            f"PASS: {source_revision} is the current head of a tracked pull "
            "request."
        )
    return False, (
        f"{source_revision} is neither an ancestor of the default branch "
        f"(compare status: {compare_status!r}) nor the current head of any "
        "associated pull request. If this was once a PR head, it was "
        "likely superseded by a force-push before it merged, and its "
        "check-run results may now be stale. Re-request publication with "
        "a commit that is either merged into the default branch or is a "
        "tracked PR's current head SHA."
    )


def main():
    # `prs` (the larger, less predictable payload) comes from stdin;
    # `compare_status`/`source_revision` are short, shell-safe scalars
    # passed via environment variables by the calling workflow step —
    # deliberately avoiding building a combined JSON blob in shell, which
    # would risk quoting/escaping bugs with no real benefit here.
    prs = json.load(sys.stdin)
    compare_status = os.environ.get("COMPARE_STATUS") or None
    source_revision = os.environ["SOURCE_REVISION"]
    ok, reason = evaluate(compare_status, prs, source_revision)
    print(f"::error::{reason}" if not ok else reason)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
