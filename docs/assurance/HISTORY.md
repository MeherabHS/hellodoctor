# Earlier performance, resilience and security documents

These documents were written between 2 October and 11 November 2026, each against whatever
commit was current that day. They are kept unchanged as the engineering record: they show how each
bottleneck and defect was found and fixed. **None of their verdicts describes the current baseline.**
For current status read [README.md](README.md).

Documents fall into two groups:

- **Design references** describe how a mechanism is meant to work. They stay current until the
  design changes, and the assurance pack links to them rather than repeating them.
- **Evidence runs** are dated measurements or reviews. Their numbers and open/closed verdicts are
  superseded by this pack; their findings and fixes remain part of the history below.

## Design references (still current)

| Document | What it defines |
|---|---|
| [AUTH_SECURITY.md](../AUTH_SECURITY.md) | Authentication, authorization and session architecture |
| [AUTH_ABUSE_AND_SESSION_THEFT_MODEL.md](../AUTH_ABUSE_AND_SESSION_THEFT_MODEL.md) | Threat model for credential abuse and session theft |
| [ECONOMIC_IDEMPOTENCY_MODEL.md](../ECONOMIC_IDEMPOTENCY_MODEL.md) | How payments, wallets and payouts stay exactly-once |
| [DISTRIBUTED_ABUSE_AND_LOAD_SHEDDING.md](../DISTRIBUTED_ABUSE_AND_LOAD_SHEDDING.md) | The admission-control and load-shedding design (`src/admission.rs`) |
| [RECOVERY_SECURITY_MODEL.md](../RECOVERY_SECURITY_MODEL.md), [DISASTER_RECOVERY_ARCHITECTURE.md](../DISASTER_RECOVERY_ARCHITECTURE.md), [DISASTER_RECOVERY_PLAN.md](../DISASTER_RECOVERY_PLAN.md) | Backup, restore and recovery |
| [INCIDENT_RESPONSE_RUNBOOK.md](../INCIDENT_RESPONSE_RUNBOOK.md) | Incident handling |

## Evidence runs — performance and capacity

| Date | Document | Setup and headline |
|---|---|---|
| 2026-10-04 | [CAPACITY_EVIDENCE_REPORT.md](../CAPACITY_EVIDENCE_REPORT.md) | First load test, laptop loopback, 50 workers. Found a real birthday-collision bug in appointment numbers (4 hex chars → 8). |
| 2026-10-04 | [CONSTRAINED_CAPACITY_TEST.md](../CONSTRAINED_CAPACITY_TEST.md) | First resource-capped stack (2 vCPU). Found Postgres JIT wasting ~270 ms per RLS query (`jit=off`) and an N+1 query at scale; passed to ~200 RPS after rebalancing. |
| 2026-10-20 | [POST_IMPLEMENTATION_LOAD_RESULTS.md](../POST_IMPLEMENTATION_LOAD_RESULTS.md) | Load results after the admission-control work. |
| 2026-10-20 | [API_TRAFFIC_CACHE_AND_POLLING_VERIFICATION.md](../API_TRAFFIC_CACHE_AND_POLLING_VERIFICATION.md) | Client polling, caching and backoff behaviour. |
| 2026-10-31 | [MILESTONE_8_0_PERFORMANCE_CHARACTERIZATION.md](../MILESTONE_8_0_PERFORMANCE_CHARACTERIZATION.md) | Session-ramp harness profiles; removed a thundering-herd artifact from the test itself. |
| Oct–Nov | [MILESTONE_8_0_DB_PERFORMANCE_REMEDIATION.md](../MILESTONE_8_0_DB_PERFORMANCE_REMEDIATION.md) | `chat_messages` row-level-security performance fix. |
| Oct–Nov | [MILESTONE_8_0R_ADMISSION_CONTROL_REMEDIATION.md](../MILESTONE_8_0R_ADMISSION_CONTROL_REMEDIATION.md) | Admission-control correctness and benchmark fixes. |
| Oct–Nov | [MILESTONE_8_0_BOOKING_FAIRNESS_REMEDIATION.md](../MILESTONE_8_0_BOOKING_FAIRNESS_REMEDIATION.md) | JWT admission identity and booking fairness. |
| Nov | [MILESTONE_8_CRITICAL_QUERY_INDEX_AUDIT.md](../MILESTONE_8_CRITICAL_QUERY_INDEX_AUDIT.md) | Critical query and index review. |
| Nov | [MILESTONE_8_SIX_SERVICE_PERFORMANCE_REMEDIATION.md](../MILESTONE_8_SIX_SERVICE_PERFORMANCE_REMEDIATION.md) | First six-service (4 vCPU / 8 GB) runs; lane-concurrency and DB-pool pressure diagnosis. |
| Nov | [MILESTONE_8_PHASE5_EFFICIENCY_INVESTIGATION.md](../MILESTONE_8_PHASE5_EFFICIENCY_INVESTIGATION.md), [MILESTONE_8_PHASE5_SIX_SERVICE_COMPARISON.md](../MILESTONE_8_PHASE5_SIX_SERVICE_COMPARISON.md) | Efficiency work and before/after comparison. |
| Nov | [MILESTONE_8_MEMORY_PROFILE.md](../MILESTONE_8_MEMORY_PROFILE.md) | Per-workload memory profile. |
| Nov | [MILESTONE_8_SUSTAINED_RELIABILITY.md](../MILESTONE_8_SUSTAINED_RELIABILITY.md) | Sustained reliability and recovery. |
| 2026-11-05 | [MILESTONE_8_ENDURANCE_VALIDATION_AND_HARNESS_FIXES.md](../MILESTONE_8_ENDURANCE_VALIDATION_AND_HARNESS_FIXES.md) | 60-minute six-service run: 2,750 sessions, 187.5 RPS, 0.28 % error rate (503s counted as errors); fixed harness token-refresh bug. |

## Evidence runs — resilience

| Date | Document | Headline |
|---|---|---|
| 2026-10-19 | [ADVERSARIAL_RESILIENCE_RESULTS.md](../ADVERSARIAL_RESILIENCE_RESULTS.md) | Isolated SQL fixture passed; HTTP surge and full regression not run. Verdict **inconclusive**. |
| 2026-10-20 | [FINAL_DISTRIBUTED_RESILIENCE_ACCEPTANCE.md](../FINAL_DISTRIBUTED_RESILIENCE_ACCEPTANCE.md), [FINAL_RESILIENCE_ROOT_CAUSE.md](../FINAL_RESILIENCE_ROOT_CAUSE.md) | Admission capacity policy root cause; distributed load left inconclusive. The spike test in this pack is the HTTP surge those runs could not complete. |

## Evidence runs — security

| Date | Document | Headline |
|---|---|---|
| 2026-10-04 | [FINAL_SECURITY_GAP_REVIEW.md](../FINAL_SECURITY_GAP_REVIEW.md) | Control-by-control review; 4 high and 7 medium open items. Re-checked in [SECURITY_STATUS.md](SECURITY_STATUS.md). |
| 2026-10-08 | [PUBLIC_EXPOSURE_HARDENING.md](../PUBLIC_EXPOSURE_HARDENING.md) | Session and public-exposure hardening. |
| 2026-10-09 | [PRE_PATIENT_SECURITY_PENTEST_REPORT.md](../PRE_PATIENT_SECURITY_PENTEST_REPORT.md) | Internal white-box assessment: BOLA, RLS, auth, endpoint inventory. |
| 2026-10-09 | [audit/DOCTOR_SECURITY_BUG_AUDIT.md](../audit/DOCTOR_SECURITY_BUG_AUDIT.md) | Doctor app bug, security and concurrency audit. |
| 2026-10-20 | [FINAL_SECURITY_INTEGRATION_ACCEPTANCE.md](../FINAL_SECURITY_INTEGRATION_ACCEPTANCE.md), [POST_IMPLEMENTATION_SECURITY_ACCEPTANCE.md](../POST_IMPLEMENTATION_SECURITY_ACCEPTANCE.md) | Integration acceptance; provider-dependent items inconclusive. |
| 2026-10-20 | [SECURITY_AUDIT_RUN1_REMEDIATION.md](../SECURITY_AUDIT_RUN1_REMEDIATION.md), [SECURITY_MILESTONE_IMPLEMENTATION_REPORT.md](../SECURITY_MILESTONE_IMPLEMENTATION_REPORT.md) | Audit remediation and implementation report. |
| 2026-10-20 | [CHAT_SECURITY_AND_HISTORY_VERIFICATION.md](../CHAT_SECURITY_AND_HISTORY_VERIFICATION.md) | Clinical chat access and retained history. |
| Nov | [REMEDIATION_E3_AZURE_CONTAINER_HARDENING_REPORT.md](../REMEDIATION_E3_AZURE_CONTAINER_HARDENING_REPORT.md) | Container and Azure hardening. |
| Nov | [MILESTONE_8_FINAL_ACCEPTANCE.md](../MILESTONE_8_FINAL_ACCEPTANCE.md) | Milestone 8 verdict: partial, production readiness not declared. |
