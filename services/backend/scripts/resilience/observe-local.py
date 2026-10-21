"""Separate observer: authoritative DB/telemetry snapshots grade the invariant.

Snapshots must be collected by a fixture owner using a separate read-only DB
connection and metrics/process sampling. Attacker response bodies are not an
economic oracle. Missing authoritative evidence yields INCONCLUSIVE.
"""
import argparse
import json
from pathlib import Path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--before", type=Path, required=True)
    parser.add_argument("--after", type=Path, required=True)
    parser.add_argument("--attack", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    before, after, attack = [json.loads(path.read_text(encoding="utf-8")) for path in (args.before, args.after, args.attack)]
    case = attack["case_id"]
    checks = {}

    def exactly(field, value):
        checks[field] = None if field not in after else after[field] == value

    if case.startswith("payment_") or case == "webhook_replay":
        exactly("active_payment_attempts", 1)
        exactly("payment_obligations", 1)
        if case in ("payment_same_key", "payment_unique_key", "payment_two_browser"):
            exactly("active_provider_intents", 1)
            exactly("authoritative_result_recovered", True)
        if case in ("payment_success_race", "webhook_replay"):
            for field in ("payment_confirmation_effects", "consultation_activations"):
                exactly(field, 1)
    elif case == "admin_double_action":
        exactly("financial_transition_effects", 1)
        checks["winning_admin_recorded"] = None if "winning_admin_id" not in after else bool(after["winning_admin_id"])
    elif case in ("distributed_surge", "distributed_login", "distributed_upload"):
        for peak, ceiling in (("inflight_peak", "inflight_ceiling"), ("argon2_peak", "argon2_ceiling"), ("sanitizer_peak", "sanitizer_ceiling"), ("db_waiters_peak", "db_waiters_ceiling")):
            checks[peak] = None if peak not in after or ceiling not in after else after[peak] <= after[ceiling]
        checks["rejected_excess"] = bool(attack["http_statuses"].get("429", 0) + attack["http_statuses"].get("503", 0))
        checks["ordinary_latency_budget"] = None if "ordinary_p95_ms" not in after or "ordinary_p95_budget_ms" not in after else after["ordinary_p95_ms"] <= after["ordinary_p95_budget_ms"]
        checks["core_database_protected"] = after.get("core_database_protected")
    elif case == "recovery_stampede":
        checks["recovery_jitter_observed"] = after.get("recovery_jitter_observed")
    else:
        checks["supported_case"] = None
    # Comparable independently identified fixture snapshots are mandatory.
    checks["same_fixture"] = (before.get("fixture_id") == after.get("fixture_id")) if before.get("fixture_id") else None
    verdict = "DEFENSE_FAIL" if any(value is False for value in checks.values()) else (
        "INCONCLUSIVE" if any(value is None for value in checks.values()) else "DEFENSE_PASS")
    # Retain only explicit noncredential observer fields, never arbitrary rows.
    safe_fields = {"fixture_id", "active_payment_attempts", "payment_obligations", "active_provider_intents",
                   "authoritative_result_recovered", "payment_confirmation_effects", "consultation_activations",
                   "financial_transition_effects", "winning_admin_id", "inflight_peak", "inflight_ceiling",
                   "argon2_peak", "argon2_ceiling", "sanitizer_peak", "sanitizer_ceiling", "db_waiters_peak",
                   "db_waiters_ceiling", "ordinary_p95_ms", "ordinary_p95_budget_ms", "core_database_protected",
                   "recovery_jitter_observed", "cpu_percent", "rss_bytes", "db_acquire_timeouts", "otp_sends"}
    with args.output.open("x", encoding="utf-8") as output:
        json.dump({"case_id": case, "request_count": attack["request_count"], "checks": checks,
                   "db_before": {key: value for key, value in before.items() if key in safe_fields},
                   "db_after": {key: value for key, value in after.items() if key in safe_fields}, "verdict": verdict}, output, indent=2)


if __name__ == "__main__":
    main()
