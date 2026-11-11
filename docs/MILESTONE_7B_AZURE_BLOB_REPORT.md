# Milestone 7B — Azure Blob Storage Integration Report

## Verdict

**GREEN for the adapter and application contract; Azure cloud acceptance remains pending.**

No Azure resources, credentials, deployment or existing-object migration were
performed. All existing uncommitted work was preserved.

## Inspection and design

The repository was using `object_store 0.14.2` with only its AWS feature and a
single optional `AppState.object_storage` handle. Clinical documents, profile
photos, prescriptions and chat attachments use that handle; Admin article
covers use the same seam but a separate database/media lifecycle. The existing
AES-256-GCM encryption, SHA-256 verification, ownership metadata, RLS/ABAC and
durable cleanup paths were retained.

The pinned crate already supports `MicrosoftAzureBuilder`, managed identity,
account-key authentication, Azurite and conditional `PutMode::Create`. It was
selected to avoid a second SDK or provider-specific handler changes.

## Implementation

- Enabled the pinned `object_store` Azure feature in `services/backend/Cargo.toml`.
- Added typed provider selection and Azure validation to
  `services/backend/src/config.rs`.
- Added explicit `STORAGE_PROVIDER` modes: `disabled`, `azure` and
  `legacy_s3`. Azure never falls back to S3, MinIO, memory or local files.
- Legacy S3 compatibility is restricted to development/test; staging and
  production fail closed unless `STORAGE_PROVIDER=azure`.
- Added Azure account, auth-mode, endpoint, emulator, managed-identity and
  two-container settings.
- Added `AppState.article_media_storage` while preserving the clinical
  `object_storage` handle.
- Added Azure builders for managed identity, account key and Azurite modes in
  `services/backend/src/repository/mod.rs`.
- Routed Admin article media through the article-specific handle.
- Changed clinical document writes to `PutMode::Create`, preventing accidental
  overwrites.
- Changed chat attachment metadata to use the configured clinical container
  instead of a hard-coded name.
- Preserved existing encrypted bytes and cleanup/error semantics.

Relevant files:

- [Cargo.toml](../services/backend/Cargo.toml)
- [config.rs](../services/backend/src/config.rs)
- [repository/mod.rs](../services/backend/src/repository/mod.rs)
- [document_storage_service.rs](../services/backend/src/services/document_storage_service.rs)
- [article_media_service.rs](../services/backend/src/services/article_media_service.rs)
- [chat_attachment_handlers.rs](../services/backend/src/api/v1/chat_attachment_handlers.rs)
- [azure_storage.rs](../services/backend/tests/azure_storage.rs)
- [api_tests.rs](../services/backend/tests/api_tests.rs)

## Configuration and security

The unified configuration now supports:

```dotenv
STORAGE_PROVIDER=azure
AZURE_STORAGE_ACCOUNT_NAME=
AZURE_STORAGE_AUTH_MODE=managed_identity
AZURE_STORAGE_CLINICAL_CONTAINER=
AZURE_STORAGE_ARTICLE_CONTAINER=
AZURE_STORAGE_ACCOUNT_KEY=
AZURE_STORAGE_CLIENT_ID=
AZURE_STORAGE_ENDPOINT=
AZURE_STORAGE_USE_EMULATOR=false
STORAGE_ENCRYPTION_KEY_B64=
```

Validation rejects missing accounts/containers, duplicate containers, invalid
Azure names, incompatible credential combinations, non-loopback emulators and
emulators outside development/test. It also requires the existing application
encryption value to decode to exactly 32 bytes and rejects legacy S3 mode outside
development/test. Diagnostics do not print credential values.

Azure Storage authentication is distinct from the existing application
encryption key. Patient, Doctor and Admin clients receive no Blob credentials,
URLs or SAS tokens. Existing authenticated backend reads continue to enforce
ownership and publication state before storage access.

## Conditional writes and failure behavior

The pinned Azure adapter maps `PutMode::Create` to `If-None-Match: *`. Duplicate
objects return a conflict and do not overwrite existing ciphertext. Missing
objects remain safe to delete; other storage errors remain failures for the
existing durable cleanup/retry workflows. Azure initialization is explicit and
fails closed if configuration is invalid.

## Tests

- `cargo fmt --all -- --check`: PASS
- `cargo clippy --all-targets -- -D warnings`: PASS
- `cargo check --all-targets --locked --offline`: PASS
- Rust library: **96 passed, 0 failed**
- Azure builder and create-only unit tests: **2 passed**
- Azure configuration integration test: **1 passed**
- Azurite contract test: **1 passed** against owned `hd-7b-azurite` with
  separate `clinical-private` and `article-media` containers
- Serialized backend integration (final source): **446 passed, 0 failed, 4 ignored**
- `cargo audit --deny warnings --ignore RUSTSEC-2023-0071`: PASS

The Azurite test covered round trips, create-only conflicts, container
isolation, missing-object behavior after deletion and cleanup. It did not prove
Azure cloud RBAC, Entra token acquisition, firewall/private endpoint behavior,
retention, backup or managed identity in a VM.

## Operator documentation

- [AZURE_BLOB_SETUP.md](AZURE_BLOB_SETUP.md)
- [VM_CONFIGURATION_GUIDE.md](VM_CONFIGURATION_GUIDE.md)
- [`.env.example`](../.env.example)
- [AZURE_BLOB_INTEGRATION_PLAN.md](AZURE_BLOB_INTEGRATION_PLAN.md)

## Existing objects and migration

No objects were migrated. Before a real cutover, inventory every metadata
namespace and source object, preserve hashes and encryption-key references,
fence concurrent writes and cleanup, validate authorized decrypted reads, and
review Azure soft-delete/versioning/legal-hold settings against clinical and
financial retention requirements.

## Remaining external checks

The operator must later provide the Storage Account name, two private container
names, approved managed identity or account key, network policy and the
existing application encryption key. Actual Azure connectivity and production
RBAC acceptance are intentionally unverified until those resources are
authorized and configured.
