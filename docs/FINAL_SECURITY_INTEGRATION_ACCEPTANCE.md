# HelloDoctor security integration acceptance — 20 October 2026

Current source revision: `dca9b3c5fcdb6303d31abbb77d3a1fd4ac1f879f` with a dirty worktree. Evidence is synthetic and local. The security-audit workflow was applied, but this Windows session cannot provide the skill’s full OS-enforced target sandbox; no external or production endpoint was contacted.

## Executed evidence

The serialized Rust API suite ran against the owned Redis fixture and disposable SQLx databases. Its nine `security_acceptance` cases passed, including SHA256 Admin TOTP, concurrent TOTP replay, refresh rotation/reuse, device proof HTTP binding/replay, dependency-loss fail closed, and direct economic-journal denial for every capability role. The HMAC BFF case passed in a focused rerun with a synthetic `WEB_AUTH_ASSERTION_SECRET` after its deliberate missing-secret guard was observed in the full run.

The same suite passed the RLS and negative authorization cases covering cross-patient and cross-doctor reads/writes, role mismatch, pool context reset, document ownership, terminal consultation mutation, and direct database capability-role restrictions. Admin Web’s five Redis-backed refresh tests passed against the owned `16380/14` fixture and verified single-use rotation, cross-instance coordination, lock expiry, token non-disclosure, logout behavior, and fixture ownership checks.

Admin and Doctor browser tests cover Turnstile validation rules, exact action/hostname, expiry/replay, provider failure and bounded reads. They do not prove Cloudflare Siteverify acceptance because no external provider call was made. Cookie and BFF code paths are source-present and locally tested; production cookie and origin behavior still needs an operator-configured browser run.

## States

| Control | State | Evidence level |
|---|---|---|
| Admin password/OTP/TOTP/refresh/logout/revocation | PASS | synthetic HTTP/API |
| SHA256 TOTP policy and replay window | PASS | synthetic API and RFC vectors |
| Admin BFF HMAC method/path/body/nonce | PASS | synthetic HTTP with test secret |
| Browser Turnstile route/provider behavior | INCONCLUSIVE | controlled helper responses; provider unverified |
| Session rotation and family revocation | PASS | synthetic HTTP/API |
| Android device proof | PASS | synthetic HTTP/API |
| Hardware-backed Keystore runtime | DEFERRED_BY_USER | physical device required |
| RLS/BOLA negative authorization | PASS | API and capability-role database tests |
| Redis outage fail closed and recovery | PASS | owned fixture/API cases |
| Production TLS, proxy and cookie deployment | INCONCLUSIVE | source/config review only |

No production secret, OTP, patient data, payment operation, provider credential, or external Siteverify request was used.
