# ADR-009: File Storage Strategy for Multi-Prescription Photos & PDF EMRs

> **Status:** APPROVED  
> **Date:** 2026-10-01  
> **Deciders:** Principal Systems Architect, Security Reviewer

---

## 1. Context
HelloDoctor allows patients to upload 1 to 5 high-resolution physical prescription or lab report photos (up to 10 MB each) prior to consultation. In addition, the system generates cryptographically signed PDF prescriptions that must be archived indefinitely in compliance with medical record retention regulations. Storing binary files directly inside PostgreSQL would cause database bloat and degrade query performance.

## 2. Decision
We adopt **Encrypted S3-Compatible Object Storage (MinIO for self-hosted / AWS S3 for cloud)** paired with **PostgreSQL Metadata Tables** and **Time-Limited Pre-Signed URLs**.

## 3. Alternatives Considered
1. **Storing File Blobs in PostgreSQL (`BYTEA` columns):** Significantly inflates database backup size, degrades cache efficiency, and increases disk I/O during routine queries.
2. **Local Server Filesystem:** Fails to scale across multiple container instances and introduces complex volume-sharing requirements.

## 4. Rationale
- **Encrypted at Rest:** Object storage buckets enforce server-side encryption with customer-managed keys (SSE-KMS / AES-256).
- **Pre-Signed URLs:** Secure, time-limited access (15-minute expiration) ensures medical images and prescriptions cannot be publicly indexed or scraped.
- **Relational Metadata Separation:** File metadata (size, MIME type, page number, appointment ID) is stored in `prescription_intake_documents` with check constraints enforcing the 5-photo cap.

## 5. Consequences
- **Positive:** Scalable storage, zero database bloat, fine-grained access control via temporary pre-signed links.
- **Trade-off:** Client must handle pre-signed URL retrieval or multipart stream proxying.
