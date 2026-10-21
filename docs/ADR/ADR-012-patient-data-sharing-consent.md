# ADR-012: Patient Data-Processing and Health-Information Sharing Consent

## Decision

Patient account creation requires acceptance of the current server-owned
`PATIENT_DATA_SHARING_CONSENT` document. The document is fetched from
`GET /api/v1/legal/documents`; the app does not duplicate its legal text. The
OTP verification request sends only the document type and version. The
auth-worker transaction validates that the version is current, creates the
Patient/profile, and appends an immutable `legal_acceptances` row. A missing or
stale consent blocks first-time account creation.

Patients can read their own evidence through
`GET /api/v1/patients/legal/acceptances`. The ledger is append-only and the
database trigger recomputes the document hash from the server-owned body.

The Patient form deliberately separates data-processing consent from informed
consent for a particular health service or treatment. Guardian/dependent copy
states that a parent or lawful guardian must act for a child or dependent.

## Legal basis and limits

The copy is an implementation baseline informed by the Kenya Data Protection
Act principles and the ODPC *Guidance Note on the Processing of Health Data*.
It describes purpose limitation, transparency, data minimization, rights,
withdrawal where consent is the lawful basis, and controlled sharing. Kenya's
Digital Health (Health Information Management Procedures) Regulations, 2025
(Legal Notice 76 of 2025) govern health-data handling and controlled access.
Onboarding acceptance is not a substitute for the purpose-specific consent,
approvals and data-sharing agreements required for a particular disclosure.

Sources checked on 2026-10-10:

- [Kenya Law: Digital Health (Health Information Management Procedures) Regulations, 2025](https://new.kenyalaw.org/akn/ke/act/ln/2025/76/eng@2025-04-11/source).
- [ODPC Guidance Note on Processing of Health Data](https://www.odpc.go.ke/wp-content/uploads/2024/02/ODPC-Guidance-Note-on-Processing-of-Health-Data.pdf).
- [ODPC rights of a data subject](https://www.odpc.go.ke/rights-of-a-data-subject/).

The earlier 2024 Ministry draft is not the operative reference. The form is
an onboarding baseline, not authorization for unrelated research, marketing,
or every future recipient. Optional processing needs separate choices.

This ADR does not claim that a checkbox alone makes HelloDoctor legally
compliant. Counsel/DPO review, a published privacy notice and contacts,
controller/processor and cross-border safeguards, retention/erasure processes,
child/guardian procedures, incident handling, and any required regulator or
provider agreements and Digital Health Agency certification requirements
remain production release gates. The test build uses synthetic data only.

## Consequences

- New Patient registration has a verifiable, versioned consent record.
- Existing account login remains available; forced re-consent on document
  version changes is a later lifecycle feature.
- Doctor onboarding continues to enforce its own required documents and does
  not display the Patient-only document.
