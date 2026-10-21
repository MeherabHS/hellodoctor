# Single human ADMIN migration

The active human product role is ADMIN. Multiple individually authenticated accounts retain independent user/session/audit identities. The five existing PostgreSQL API/auth/payment/system/finance worker roles remain separate capability identities.

Migrations 20261106000001 and 20261106000002 add ADMIN and map legacy PLATFORM_ADMIN, CLINICAL_ADMIN, FINANCE_ADMIN, SUPPORT, COMPLIANCE and SECURITY_ADMIN users. Existing sessions are revoked so a pre-upgrade token cannot silently keep its old role. New logins/refreshes use the database's canonical role. A trigger prevents assigning retired human roles. Historical enum values and audit actors remain valid.

Authorization, permissions and RLS/function role comparisons use ADMIN. Historical role parsing remains for compatibility. Sensitive Admin actions record actor_user_id, ADMIN role, actual auth session, request and server timestamp. Technical system events have explicit system identity rather than an anonymous shared human Admin.

The reverse migration restores saved policy/function definitions and original legacy role assignments. New post-upgrade Admin accounts map to PLATFORM_ADMIN on rollback for old binary compatibility; historical ADMIN enum values remain. Revoked old sessions stay revoked. Trace rollback refuses to erase held grievance evidence.

`test_milestone_legacy_admin_upgrade_specific_accounts_and_reversible_schema` exercises all six old roles, revoked sessions, preserved historical audit roles, rejected new legacy assignment, SQLx undo/reapply and two independently attributed Admin accounts. No historical validation tag was rewritten.

Approved assertion changes are explicit in `audit_evidence/lifecycle_milestone/test_integrity_audit.json`: canonical role values, all-Admin privileges and exact 401 for retired forged claims. Patient/Doctor exact 403 denials remain. This is an intentional product-contract migration, not relaxed authorization testing.

A valid signed token with an active account/session but a mismatched retired role is rejected with 401 and leaves an `AUTH_ROLE_MISMATCH` permanent audit attributed to the actual database account role/session. The compliance-denial regression still requires four durable denials, with two specifically asserted at the authentication boundary. Generic invalid tokens are not attributed to an unverified claimed account.

The Admin header no longer impersonates a hardcoded Amina Rashid/Clinical Ops Director persona. It identifies the canonical ADMIN surface; investigation events retain the actual individual account/session. Decision buttons say refund decision, Doctor warning and dismissal to match the backend workflow, rather than suggesting a provider refund or generic resolution has happened.
