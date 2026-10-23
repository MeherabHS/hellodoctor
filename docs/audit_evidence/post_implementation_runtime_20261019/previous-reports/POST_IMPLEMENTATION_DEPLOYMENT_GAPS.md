# Post-implementation deployment gaps — 19 October 2026

**Read-only review. No Azure resource, firewall, DNS, ingress, host service, shared database, production identity, MFA enrollment or provider configuration was changed.** No deployed environment was inspected. Source requirements and operator-observed facts are different evidence classes.

## Gap categories

| Category | Current evidence | Required action / boundary |
| --- | --- | --- |
| Missing implementation | Live collection intent, authoritative production callback verification, provider refund/payout execution and automated provider reconciliation are not present | Implement approved provider adapters/fixtures before enabling real-money workflows; no fake checkout or manual local status presented as provider proof |
| Missing implementation / architecture mismatch | `Cargo.toml` enables object_store `aws`; `repository/mod.rs` constructs `AmazonS3Builder`. No Azure Blob adapter was found | Reconcile actual storage implementation with the required Azure Blob target before production document use; an S3 endpoint setting alone does not prove Azure Blob integration. No storage service/dependency was added here |
| Configuration acceptance | Turnstile keys/actions/exact hostnames, Admin BFF assertion secret, independent ingress secret, Azure email sender/settings, Admin SHA256 enrollment | Supply through managed server secrets; test with synthetic principals first; never send secrets through this report/chat |
| Runtime validation | Backend/Next/Flutter/Android builds and regressions; SQLx migration upgrades/checksums; HTTP auth/device replay/load/fault scenarios | Provision isolated offline toolchains and disposable PG/Redis before execution |
| Infrastructure/operator action | TLS, edge/CDN/WAF policy, direct-origin restrictions, private DB/Redis/storage, proxy limits/trust, monitoring/backups/recovery | Review actual Azure/edge configuration through a separately authorized operator; do not infer subscription capabilities |
| Distribution/operator action | Both mobile release Gradle configurations currently select debug signing | Configure approved release signing outside tests before real distribution; never use production signing secrets in this fixture |
| Optional future enhancement | App attestation, user-visible sessions, dedicated Admin recovery admission, server-side safe read single-flight | Prioritize using actual risk/capacity evidence; these are not automatically vulnerabilities |

## Edge and origin readiness

Expected design: Internet → selected CDN/WAF/bot protection → Azure origin reverse proxy → Rust API → private Redis/PostgreSQL/storage. No proprietary edge feature is required for backend correctness. Source admission remains necessary when traffic reaches origin.

`docs/PRODUCTION_DEPLOYMENT.md` explicitly identifies current compose manifests as development/capacity configurations. Development compose publishes PostgreSQL 5432 and Redis 6379 without a host-interface restriction. That is a source fact about a development manifest, **not proof those ports are public in production**. Do not reuse it as production networking policy. No production reverse-proxy manifest was found in the reviewed paths.

| Deployment control | Source-backed current position | Deployment evidence still required |
| --- | --- | --- |
| TLS / redirect / HSTS | Deployed API origins are required to use HTTPS; public transport validates secure forwarding | Certificate chain, HTTP redirect, HSTS rollout, origin listener exposure |
| Proxy identity | Exact trusted physical peer; single validated forwarded client/proto values | Actual ingress IPs, proxy replacement behavior and blocked spoofed/untrusted forwarding |
| Direct origin | Documentation requires intentional restriction | Azure NSG/private routing/authenticated origin path; maintain managed SSH, deployment, health and incident access |
| Connection/header controls | Application admission does not bound pre-header sockets | Proxy global/per-client connections, header size/count, request-header timeout and socket/backlog policy |
| Body/slow-client controls | API 15 MiB ceiling and 10s handler timeout; BFF auth body limits/readers/deadlines; sanitizer early validation | Proxy body cap/read timeout, slow-body behavior, idle/keepalive limits; independently test before exposure |
| Admin/Doctor upstream restrictions | Admin assertion gate; browser BFF challenge; native Doctor route remains accessible under backend limits | Intended BFF ingress path, private upstream policy, no secret leakage, actual role/CSRF/cookie behavior |
| Metrics | Deployed scrapes require bearer monitoring credential; method/route labels bounded in corrected source | Private monitoring network/access, token injection/rotation, no query-token exposure, actual alerts |
| Readiness/liveness | `/healthz` now checks DB/migrations plus bounded Redis PING when DB ready | Use for dependency readiness; provide an intentional liveness policy. Do not repeatedly restart healthy API processes just because Redis is down |
| Secret injection | Server-only assertion/Turnstile/email/role/token settings required; no new secret printed here | Managed secret ownership/access/rotation, startup validation, sanitized telemetry/export configuration |
| Data services | Capability roles/RLS and private-network requirements in source/docs | Real role ownership/membership, least privilege, private network and storage identity/access policy |
| Backups/recovery | Existing docs require encrypted off-host backups, restore drill, NTP and incident procedure | Successful restore evidence, recovery access inventory, backup/CPU/RSS/disk/payment-reconciliation alerts |
| Edge features | No subscribed Cloudflare plan was queried | Operator verifies actual available DDoS/WAF/bot/rate capabilities; no plan-specific promise |

The edge can protect network and application entry capacity; it cannot substitute for database economic uniqueness, authoritative sessions or provider reconciliation. Do not claim DDoS immunity.

## Migration acceptance and compatibility

Parent-side source inspection found **62 up migrations, 62 matching down files and no duplicate numeric versions**. [SHA384 inventory](audit_evidence/post_implementation_20261019/migration-inventory.json) records exact raw source hashes. Local SQLx 0.8.6 source (`sqlx-core/src/migrate/migration.rs`) hashes SQL bytes with SHA384. No live `_sqlx_migrations` state was queried or modified, so these are prospective checksum inputs, not a passed applied-checksum comparison.

| New milestone migration | Ordering/behavior | Rollout/rollback concern |
| --- | --- | --- |
| `20261110000001_economic_idempotency` | Follows `20261109000001`; partial obligation index, payout reference uniqueness, journal/triggers, completion function | Fails on conflicting history. Down drops journal/economic guards and restores old completion semantics; preserve journal evidence and coordinate binary before any approved rollback |
| `20261110000003_auth_session_device_keys` | Follows economic; immutable refresh-family key binding | Down removes binding evidence; coordinate binary/session invalidation, never silently downgrade bound families |
| `20261110000004_admin_totp_algorithm` | Latest current version; existing records default SHA1 | Existing SHA1 enrollment fails closed under SHA256-only verifier. Provision a synthetic tested SHA256 enrollment and controlled recovery before deploy; rollback loses algorithm metadata |

Date-looking filenames are numeric SQLx versions, not an instruction to wait until November. The absent `...000002` version is not itself a defect. No filename, applied history or existing migration source was renamed/rewritten in this campaign. Actual applied checksum/upgrade compatibility remains BLOCKED_ENVIRONMENT.

The earlier disposable raw-SQL fixture applied all current up migrations and economic down/up, but did not exercise SQLx migration metadata/checksum validation. Its successful down/up is SQL executability on disposable synthetic data, **not a safe production rollback plan**. Never drop retained financial evidence to repair a live deploy.

The journal has ENABLE/FORCE RLS, ADMIN read policy, migration-owner internal insertion, fixed-search-path SECURITY DEFINER triggers and PUBLIC execute revocations. Application roles lack direct mutation grants. Actual production migration ownership, role memberships, privileged bypass paths and effective policies require isolated capability-role tests followed by authorized deployment inspection.

Read-only operator preflight queries, **not executed here**:

```sql
SELECT appointment_id, count(*) AS obligations,
       array_agg(id ORDER BY created_at,id) AS transaction_ids
FROM transactions
WHERE payment_status <> 'FAILED'
GROUP BY appointment_id HAVING count(*) > 1;

SELECT provider_code, gateway_reference, count(*) AS items,
       array_agg(id ORDER BY created_at,id) AS item_ids
FROM disbursement_items
WHERE gateway_reference IS NOT NULL
GROUP BY provider_code,gateway_reference HAVING count(*) > 1;

SELECT version,description,success,encode(checksum,'hex') AS checksum_hex
FROM _sqlx_migrations ORDER BY version;

SELECT c.relname,pg_get_userbyid(c.relowner) AS owner,
       c.relrowsecurity,c.relforcerowsecurity
FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace
WHERE n.nspname='public'
  AND c.relname IN ('transactions','disbursement_items','financial_economic_effects');

SELECT schemaname,tablename,policyname,roles,cmd,qual,with_check
FROM pg_policies
WHERE schemaname='public'
  AND tablename IN ('transactions','disbursement_items','financial_economic_effects');

SELECT grantee,table_name,privilege_type
FROM information_schema.role_table_grants
WHERE table_schema='public' AND table_name='financial_economic_effects';
```

If duplicate identities exist, stop migration rollout, identify legitimate FAILED history versus inconsistent active obligations, and reconcile through an approved Finance procedure. Never delete financial history or edit `_sqlx_migrations` to force acceptance. No production query authorization is implied by including these diagnostics.

## Ten requested residual-risk classifications

These are source/deployment limitations, not ten newly confirmed vulnerabilities. `SEVERITY=NOT_ASSIGNED` means no demonstrated exploit outcome or planned-feature deviation was promoted to a vulnerability. Launch decisions below are contextual and do not override the overall unverified campaign gate.

| FINDING | CURRENT_CONTROL | RESIDUAL_RISK | EXPLOITABILITY_ASSUMPTIONS | PRODUCTION_IMPACT | SEVERITY | RECOMMENDED_ACTION | LAUNCH_BLOCKER_YES_NO |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1. Play Integrity absent | Account/IP/global budgets, credentials, Android installation proof | A scripted/repackaged client can call native authentication | Public endpoint access; no attestation required by current policy | More abuse pressure; not an authentication bypass | NOT_ASSIGNED | Define package/signing/risk policy; stage approved attestation | NO by itself |
| 2. No dedicated Admin recovery lane | All Admin routes use Expensive; backend audit/authorization | Admin recovery may contend with other expensive work | Flood fills same lane/budgets; actual recovery action depends on it | Potential incident-response delay, unmeasured | NOT_ASSIGNED | Measure recovery endpoint availability; reserve only required narrow actions if justified | NO by itself; operator needs an intentional recovery path |
| 3. No server read single-flight | Global/lane admission, client coalescing | Same read can produce repeated admitted queries | Identical reads/cache expiry; hot spot exists | Bounded but avoidable SQL load | NOT_ASSIGNED | Profile before narrowly caching nonauthoritative reads | NO |
| 4. User-visible device/session management absent | Server family revocation/logout | Users lack a convenient lost-device/session review UI | A device is lost or session suspected compromised | Slower self-service revocation | NOT_ASSIGNED | Stage safe metadata/session revocation UI; retain operator recovery | NO by itself |
| 5. Browser token-theft residual | BFF HttpOnly/Secure/SameSite cookies, origin checks, active sessions | Active XSS can act as user; whole cookie theft remains useful | Browser/endpoint compromise or executable content flaw | Session misuse within existing authorization | NOT_ASSIGNED | Execute CSP/CSRF/cookie/XSS regressions; preserve short life/revocation | NO inherent design blocker; required regression remains unverified |
| 6. Non-Android/legacy bearer sessions | Short access JWT, active family check, rotation/reuse/logout | Stolen bearer credentials alone can be usable | Credential theft and unrevoked valid family | Impersonation until expiry/revocation/policy response | NOT_ASSIGNED | State supported scope honestly; expand binding only with client interoperability/recovery | NO automatic blocker; do not claim universal sender constraint |
| 7. Consultation shares DB capacity | Separate four-handler lane; early pool sample/deadlines | Reservation does not guarantee DB availability | Concurrent admitted work fills shared auth/API pool | Consultation latency/failure under saturation, unmeasured | NOT_ASSIGNED | Execute mixed-load/pool tests before acceptance; avoid blind pool enlargement | YES for resilience acceptance until measured; no architecture change presumed |
| 8. Invalid callbacks can fill webhook lane | Dedicated two-handler lane, callback budget, signature gate; production refuses absent adapter | Legitimate callback can be delayed/rejected | Invalid flood reaches origin before provider authentication | Delayed payment truth; reconciliation needed | NOT_ASSIGNED | Edge protection, provider retry/reconciliation, isolated invalid-vs-valid flood test | YES before live payment acceptance; current adapter absent |
| 9. Native Doctor login lacks browser Turnstile | Backend account/IP/global limits, bounded hashing/OTP | Native-style scripted calls avoid browser challenge | Direct credential endpoint access | Bounded abuse; CAPTCHA not universal | NOT_ASSIGNED | Test native routes directly; consider attestation as risk input | NO by itself |
| 10. Late provider reconciliation incomplete | Locked conditional callback, no expired activation, trace/manual evidence | Captured late funds may need refund/reconciliation without automated authoritative resolution | Real provider enabled; payment arrives after expiry/new attempt | Paid-but-unbooked incident / unresolved money state | NOT_ASSIGNED | Implement/test provider-result reconciliation and operator runbook before real money | YES for real-money launch |

## Additional scoped observations

- The OTP correction removes partial quota consumption on ordinary recipient denials; it does not stop legitimate-shaped distributed requests from exhausting the 30/min provider allowance. Target throttling can itself affect legitimate sign-in availability. Measure policy on target hardware/product volume.
- Registration duplicate-conflict behavior discloses account existence despite generic login errors. Record the product-policy exception or implement an approved neutral registration flow; no new authentication bypass is established.
- Some BFF generic/body-reader 503 catch branches omit `Retry-After`; synthetic route acceptance should pin the response contract. This is response consistency debt, not proof of token or MFA bypass.
- Explicit no-SMS policy remains. Azure email source exists; actual delivery is unverified. No real OTP/email was sent here.

## Prioritized operator gates

1. **Acceptance environment:** provide the required isolated offline toolchains and disposable services, then obtain real compilation/regression/adversarial results. Source alone is insufficient for launch acceptance.
2. **Financial readiness:** keep real collection/refund/payout disabled until approved adapters, provider semantics, late-result reconciliation and recovery are independently verified.
3. **Public exposure/data:** verify private DB/Redis/storage, TLS, ingress/header/body/connection limits, direct-origin controls and monitored recovery. Resolve Azure Blob implementation compatibility before clinical document production use.
4. **Authentication deployment:** provision tested Admin SHA256 enrollment/recovery, synchronized clocks, assertion/Turnstile settings, real email configuration; validate Android proof on an authorized device and browser login end-to-end.
5. **Migration/distribution:** reconcile read-only preflight/checksums and preserve financial journal on rollback; use approved release signing before distributing mobile builds.

Each operator/cloud/device/provider action requires its own authorized environment. None was silently performed in this campaign.
