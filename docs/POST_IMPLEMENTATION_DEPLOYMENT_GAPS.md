# Post Implementation Deployment Gaps — 20 October 2026

Updated from **executed local development verification**, 19–20 October 2026. This is not formal sandbox certification or production acceptance. The owner authorized local toolchains/disposable services and froze original application source, tests, assertions, configuration and the security skill. Only new fixture/evidence files and these reports were changed.

Branch `main`; commit `dca9b3c5fcdb6303d31abbb77d3a1fd4ac1f879f`; dirty working tree on entry. [Entry baseline](audit_evidence/post_implementation_runtime_20261019/baseline.json) and [final preservation check](audit_evidence/post_implementation_runtime_20261019/source-freeze-final.json): **539 original files checked, zero changes, skill unchanged**. Web and Flutter manifests independently checked 132 and 425 original files (overlapping inventories, not additive). Earlier reports remain in [the historical archive](audit_evidence/post_implementation_runtime_20261019/previous-reports/POST_IMPLEMENTATION_DEPLOYMENT_GAPS.md); earlier source corrections are not new fixes in this frozen campaign.

## 20 October continuation: two rows resolved

A later same-day continuation (authorized for non-destructive backend corrections, unlike the frozen run the rest of this report describes) closed the backend half of the first row and the API-test half of the second row below. Detail: [`FINAL_DEFECT_REMEDIATION_LOG.md`](FINAL_DEFECT_REMEDIATION_LOG.md), [`FINAL_RESILIENCE_ROOT_CAUSE.md`](FINAL_RESILIENCE_ROOT_CAUSE.md), [`FINAL_AUTOMATED_ACCEPTANCE_MATRIX.md`](FINAL_AUTOMATED_ACCEPTANCE_MATRIX.md).

- **Backend Clippy: now PASS** (0 diagnostics). Admin lint, Doctor Flutter lifecycle, and Flutter analyzer/format findings are **unchanged** — not touched this session.
- **All 264→275 `api_tests` cases now execute and pass**, using the suite's own already-disposable, ownership-verified Redis/Postgres fixtures (`hd-remediation-pg-20261020`, `hd-remediation-redis-20261020`, loopback-only, label-verified before any destructive operation) — the "hardcoded shared Redis endpoint" concern did not apply to this suite the way it does to the 5 Admin `tests/refresh.test.ts` cases, which remain genuinely blocked exactly as described below.

## Categories and operator actions

| Category | Current finding | Required next action |
| --- | --- | --- |
| Failed regression gate | Backend Clippy now PASS (see above). Admin lint, Doctor Flutter lifecycle, analyzer/format checks remain nonzero | Preserve frozen evidence for the remaining items; correct in a separately authorized change phase and rerun affected checks. No waiver is implied. |
| Missing runtime validation | **Resolved for the 263 backend API tests** (now executed and passing — see above). The five Admin `tests/refresh.test.ts` cases still cannot safely address their hardcoded shared Redis endpoint | Provision an isolated localhost namespace/toolchain for the Admin refresh tests, or authorize a focused future fixture correction, then run unchanged security invariants. |
| Missing cached build prerequisites | Native Gradle 9.3.1 absent; AGP 9.1.0/Kotlin 2.4.0 caches not established | Prepopulate approved offline toolchain before Android compilation. Physical tests remain DEFERRED_BY_USER. |
| Missing implementation/provider integration | Real collection intent, refund, payout and authoritative late-result reconciliation acceptance absent | Keep real-money behavior disabled until approved provider adapter/recovery/reconciliation tests pass. A local approval is not a provider transfer/refund. |
| Storage integration discrepancy | Required Azure Blob deployment versus currently reviewed AWS/S3 object-store builder | Resolve intended Blob compatibility/adapter before production clinical-document storage; current campaign did not change storage. |
| Missing browser/MFA configuration acceptance | Turnstile/helper and TOTP vector pass; production hostname/action/secret/BFF/CSRF/cookie flow and full SHA256 Admin MFA/recovery not demonstrated | Provision approved configuration/synthetic enrollment, run route/browser/MFA tests before relying on these guarantees. No actual Siteverify/email provider contacted. |
| Missing resilience validation |30,000 test has 19,750 transport failures; full DB saturation, sanitizer flood and legitimate consultation/webhook mix unproven | Diagnose transport outcomes and provision complete authorized synthetic workload/telemetry. Preserve actual limits; do not enlarge pools to hide contention. |
| Infrastructure/operator action | No live Azure/Cloudflare/DNS/firewall inspection performed | Verify the deployment checklist below with operator-approved read-only/runtime acceptance. |
| Optional enhancement | Play Integrity, user-visible session management, read single-flight, narrow Admin recovery reservation | Stage based on product threat/capacity evidence; absence alone is not a confirmed vulnerability or automatic launch blocker. |

## Read-only deployment checklist

Expected path: Internet → subscribed CDN/WAF/bot controls → Azure origin reverse proxy → Rust API → private PostgreSQL/Redis/storage. Origin application admission protects accepted work; it does not bound pre-header sockets or absorb network-volume attacks.

| Surface | Source/design status | Operator evidence still needed |
| --- | --- | --- |
| Edge/origin | No verified subscription feature claim; direct-origin restriction required when edge-fronted | Actual subscribed protections, private/tunnel or approved origin allowlist, deployment/SSH/recovery access preserved. No host firewall changed. |
| TLS | HTTPS/cookie/transport validation source present | Real certificate, redirect, validated HSTS, correct original scheme, no public HTTP authentication. |
| Proxy identity | Transport guard validates physical trusted peer before forwarded identity | Narrow actual proxy list, forwarded-header stripping and origin bypass denial. Fixture trust was loopback only. |
| Connections/slow clients | Bounded API handlers do not cover all sockets | Proxy total/per-client limits, backlog, header bytes/count/read deadline, body-read deadline, idle/keepalive timeout. |
| Body handling |15MiB API limit,10s handler timeout, bounded BFF readers and early sanitizer checks | Actual reverse-proxy aggregate/body limits, slow uploads, early rejection and cleanup under authorized upload load. |
| Admin/Doctor BFF | Server challenge and Admin exact-body assertion source/helpers exist | Intended upstream restrictions, BFF-to-Rust assertion replay, cookies, CSRF/origin and mandatory production challenge flow. |
| Monitoring | Fixed-label metrics and guarded deployed scrape source; fixture metrics observed | Private scrape exposure, secret injection/rotation, CPU/RSS/pool/provider alerts and incident response. |
| Readiness | Redis failure/readiness and recovery executed in owned fixture | Deliberate readiness versus liveness policy; dependency outage should not trigger blind restart storms. |
| Data services/secrets | Capability pools/RLS source preserved | Actual role membership/ownership, private network, secret storage/access/rotation, no credential-bearing logs. |
| Recovery/backups | Runbooks/design only | Successful encrypted backup restore, deployment rollback/recovery/NTP evidence and operator access. |
| Mobile release | No physical device used; native offline compile missing prerequisites | Approved release signing (not debug signing), native compile and later authorized Keystore/device acceptance. |

## Migration evidence and rollout

Fresh SQLx application/test fixtures applied all62 up migrations. All62 applied SQLx SHA384 checksums matched the original raw migration files, with `success=true`: [migration-applied-checksums.json](audit_evidence/post_implementation_runtime_20261019/migration-applied-checksums.json). The independent raw SQL fixture also passed the economic down/up cycle. This supersedes the earlier source-only checksum claim for **fresh disposable DBs only**. No shared/deployed `_sqlx_migrations` table was read or edited. Real existing-data upgrade compatibility, role ownership and production rollback are not accepted by a clean-cluster pass.

`20261110000001_economic_idempotency`, `20261110000003_auth_session_device_keys`, and `20261110000004_admin_totp_algorithm` are numeric SQLx versions after earlier versions; their November-looking dates do not make them invalid in October. No applied migration was renamed/rewritten. Economic rollback drops journal/guards; device-key rollback removes binding evidence; TOTP rollback loses algorithm metadata. Preserve financial journal/session evidence and coordinate binary compatibility before any approved rollback. Existing SHA1 enrollment must not silently downgrade the SHA256 Admin requirement.

Read-only operator preflight (provided, **not executed against a shared/deployed DB**):

```sql
SELECT appointment_id,count(*),array_agg(id ORDER BY created_at,id)
FROM transactions WHERE payment_status <> 'FAILED'
GROUP BY appointment_id HAVING count(*) > 1;
SELECT provider_code,gateway_reference,count(*)
FROM disbursement_items WHERE gateway_reference IS NOT NULL
GROUP BY provider_code,gateway_reference HAVING count(*) > 1;
SELECT version,description,success,encode(checksum,'hex')
FROM _sqlx_migrations ORDER BY version;
SELECT schemaname,tablename,policyname,roles,cmd,qual,with_check
FROM pg_policies WHERE schemaname='public'
AND tablename IN ('transactions','disbursement_items','financial_economic_effects');
```

If conflicting historical identities exist, stop rollout and reconcile legitimate FAILED history versus inconsistent active obligations through an approved Finance process. Never delete financial history or forge SQLx metadata to force migration success. FORCE RLS, capability grants, fixed-search-path SECURITY DEFINER triggers and PUBLIC execute revocations remain in source; fresh financial superuser tests do not replace negative-role acceptance.

## Requested residual-risk classifications

These are scoped limitations, not a speculative list of confirmed vulnerabilities. NOT_ASSIGNED means no demonstrated exploit severity was established. Launch blockers are tied to the relevant proposed production feature.

| FINDING | CURRENT_CONTROL | RESIDUAL_RISK / EXPLOITABILITY_ASSUMPTIONS | PRODUCTION_IMPACT | SEVERITY | RECOMMENDED_ACTION | LAUNCH_BLOCKER_YES_NO |
| --- | --- | --- | --- | --- | --- | --- |
| Play Integrity absent | Account/IP/global limits and installation proof | Scripted/repackaged clients can reach native auth; still need credentials | Abuse pressure within budgets | NOT_ASSIGNED | Define attestation enforcement/risk policy | NO by itself |
| Dedicated Admin recovery capacity absent | Auth/Expensive lane limits and authorization | Flood shares incident-response admission | Recovery delay, unmeasured | NOT_ASSIGNED | Validate an intentional narrow recovery path | NO by itself |
| Server-side read single-flight absent | Admission; client coalescing | Identical admitted reads can duplicate queries | Bounded avoidable SQL load | NOT_ASSIGNED | Profile and cache/coalesce only nonauthoritative reads if needed | NO |
| User-visible session/device management absent | Server revocation/logout; synthetic revocation passes | Lost-device user lacks convenient review/revoke UI | Slower self-service response | NOT_ASSIGNED | Stage safe session metadata/revocation UI | NO by itself |
| Browser token-theft residual | BFF HttpOnly/Secure/SameSite and origin checks | Active XSS/browser compromise can act as user | Authorized-session misuse | NOT_ASSIGNED | Browser CSP/CSRF/cookie tests | NO inherent design blocker; acceptance pending |
| Non-Android bearer residual | Short JWT, active session, rotation/revocation | Stolen valid unbound credential can be reusable | Impersonation until policy/expiry/revocation | NOT_ASSIGNED | State binding scope accurately | NO automatic blocker |
| Consultation shared DB | Reserved lane and acquisition limits | Lane permit does not ensure shared DB availability | Clinical latency/failure under saturation | NOT_ASSIGNED | Complete legitimate consultation + full-pool workload | YES for resilience acceptance |
| Invalid webhook lane starvation | Bounded dedicated lane and provider validation policy | Invalid flood can compete before authoritative validation | Delayed provider truth | NOT_ASSIGNED | Valid/invalid callback mix plus edge/retry/reconciliation | YES before live payment acceptance |
| Native Doctor login without browser Turnstile | Direct backend account/IP/global/hash/OTP limits;100-call role test executed | Browser challenge can be avoided by native-style calls; not an auth bypass | Bounded abuse | NOT_ASSIGNED | Complete direct native-route and optional attestation tests | NO by itself |
| Late-provider reconciliation incomplete | Locked callback and expired activation guard | Real funds arrive late with no independently accepted reconciler | Paid-but-unbooked unresolved money | NOT_ASSIGNED | Provider-result recovery/reconciliation before real money | YES for real-money launch |

Registration duplicate-conflict account-existence behavior remains a product-policy exception to generic login errors. Some BFF503 branches/readiness responses omit Retry-After; this was not silently changed. No real provider, patient, Azure resource, DNS/firewall or shared development data was accessed. No DDoS immunity claim is made.
