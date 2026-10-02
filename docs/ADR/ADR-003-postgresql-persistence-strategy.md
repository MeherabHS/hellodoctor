# ADR-003: PostgreSQL Persistence Strategy & Concurrency Control

> **Status:** APPROVED  
> **Date:** 2026-10-01  
> **Deciders:** Principal Systems Architect, Database Architect

---

## 1. Context
HelloDoctor manages clinical records, secure prescription-photo records, payment-hold ledgers, and appointment slots. Preventing concurrent double-booking of doctor slots and ensuring double-entry financial accounting requires strict ACID transaction guarantees.

## 2. Decision
We choose **PostgreSQL 16+** as the central relational database, paired with **`sqlx`** in Rust for asynchronous connection pooling and compile-time verified queries (`sqlx::query!`).

## 3. Alternatives Considered
1. **MongoDB / Document Store:** Flexible schema, but lacks multi-document ACID transactions with row-level locks needed to prevent slot double-booking without race conditions.
2. **MySQL:** Solid relational database, but PostgreSQL offers superior JSONB querying, arrays (`TEXT[]` for qualifications and investigations), and rich indexing.

## 4. Rationale
- **Pessimistic Row-Level Locking:** `SELECT ... FOR UPDATE` ensures concurrent booking requests for the identical slot are serialized, preventing overbooking.
- **CHECK Constraints:** Database-level enforcement of financial mathematics (`gross_amount = platform_fee_amount + net_amount`) guarantees ledger consistency regardless of client bugs. The ledger/financial event log ensures complete auditability.
- **Relational Integrity:** Foreign keys with `ON DELETE RESTRICT` prevent accidental cascading deletion to ensure medical record protection and integrity.
- **Row-Level Security (RLS):** PostgreSQL Row-Level Security provides defense-in-depth for patient-owned tables.
- **Audit & Idempotency:** The `idempotency_records` and `audit_events` tables provide reliable retry mechanisms and full traceability of all actions.
- **Exclusion Constraints:** `btree_gist` extension allows for strict slot exclusion constraints (preventing overlapping appointments at the database level).

## 5. Consequences
- **Positive:** Complete ACID safety, compile-time SQL verification, robust indexing.
- **Trade-off:** Requires dedicated connection pool tuning (`max_connections`) and regular index vacuuming.
