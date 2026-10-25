# Azure Blob integration plan

Status: **PLANNING ONLY — separate infrastructure authorization required.**

Azure Blob Storage is HelloDoctor's approved permanent object-storage provider.
Missing legacy S3 settings are intentional. Milestone 6D cloud verification is
**DEFERRED — AWAITING AZURE BLOB INTEGRATION**. This plan does not configure a
provider, install Azure dependencies, provision resources or request credentials.
In-memory storage remains the regression fixture.

## Current implementation and reusable boundaries

- `services/backend/src/repository/mod.rs`: `AppState.object_storage` is an optional
  `Arc<dyn object_store::ObjectStore>`. Its current factory is the legacy
  `AmazonS3Builder`; `Cargo.toml` enables only the `object_store` AWS feature.
  The factory remains unchanged and unconfigured during stabilization.
- `services/backend/src/services/document_storage_service.rs`: shared private-file
  put/get/delete operations used by clinical/profile services. Database metadata
  and service authorization govern downloads; provider URLs are not public APIs.
- `services/backend/src/crypto.rs`: AES-256-GCM, random 12-byte nonce followed by
  authenticated ciphertext. Existing readers also verify the persisted plaintext
  SHA-256 and enforce object-size limits. Keep the ciphertext format and access to
  the current encryption key during any future provider change.
- `services/backend/src/services/article_media_service.rs`: article-only
  `article-media/{UUID}.jpg` keys, validated/decoded/resized JPEG or PNG inputs,
  metadata stripping by re-encoding, encrypted create-only writes, persisted upload
  intent, article/version ownership checks, protected reads and abandoned-media
  maintenance. It uses the injected object-store trait directly. Reuse these
  controls rather than routing article uploads through clinical-document endpoints.
- `services/backend/src/repository/admin_article_repo.rs`: article/media database
  ownership, current cover references, approval versions and published-read rules.
  Provider choice must not change these rules or Doctor article RLS.

The shared object-store seam is reusable, but the current single store handle does
not express two separately permissioned containers. The future task must introduce
explicit service-selected clinical/article handles or a narrow typed router at this
boundary. Clients must never select a container or submit arbitrary object keys.
Existing database bucket metadata must be inventoried and mapped deliberately.

## Adapter decision to validate during the infrastructure task

| Option | Fit and work required |
|---|---|
| Existing `object_store` Azure backend | First candidate for lowest service churn: preserve `ObjectStore` and in-memory tests. Confirm the pinned crate's managed/workload identity support, Azurite compatibility, conditional create, bounded reads and error mappings before enabling its Azure feature. This would replace the factory, not add a permanent second provider. |
| Official Azure Rust Blob SDK plus `azure_identity` | Azure-native clients and identity support. Requires a small implementation of the existing storage boundary and explicit mapping of condition failures, not-found, retries, timeouts and streaming limits. Evaluate maintenance/support status and exact API versions at implementation time. |
| Direct Azure Blob REST | Technically possible, but application code would own token acquisition/refresh, REST-version headers, retries, streaming, conditional requests and error translation. Prefer a maintained adapter/SDK unless a verified gap justifies REST. |

The [official Rust Blob SDK examples](https://github.com/Azure/azure-sdk-for-rust/tree/main/sdk/storage/azure_storage_blob)
cover Azure clients and identity; the existing abstraction's
[Azure builder](https://github.com/apache/arrow-rs-object-store/blob/main/src/azure/builder.rs)
is the alternative to evaluate. These upstream capabilities are not proof that an
unconfigured HelloDoctor deployment can currently use Azure.

## Private containers, authorization and encryption

Use separate private containers for clinical files and article media, with distinct
service-selected namespaces and least-privilege RBAC scopes. Disable anonymous blob
access at account/container level. Public article visibility means an authorized
Patient backend route returns a published cover; it does not require a public blob.
Admin preview uses its existing authenticated BFF. Draft, archived and unpublished
article checks remain database-authoritative on each request.

Keep all existing ABAC, RLS, metadata ownership and version checks. Never grant a
Patient storage credentials or a path to clinical objects. Any future SAS, CDN or
public asset route requires its own reviewed scope; none is proposed as a 6D shortcut.
See Microsoft's [anonymous-access prevention guidance](https://learn.microsoft.com/en-us/azure/storage/blobs/anonymous-read-access-prevent).

Retain application AES-256-GCM and SHA-256 integrity checks for clinical files and
article media. Azure encryption at rest complements this protection. Preserve the
current protected encryption key and ciphertext encoding; key rotation/versioning
would be a separate reviewed change. Do not log keys, tokens, blob bodies or
signed URLs. Bound encrypted downloads before buffering/decrypting, retain existing
plaintext limits, and return only authorized content with safe response headers.

Prefer Microsoft Entra managed identity/workload identity in staging/production,
with container-scoped data-plane permissions. A user-assigned identity needs its
approved client ID; production should not silently fall back to a developer login.
Local development may use an explicitly selected approved developer identity or
Azurite. If a secret-based method is unavoidable, use the approved secret manager
and rotation process, never client bundles or source control. See
[Microsoft's Entra authorization guidance](https://learn.microsoft.com/en-us/azure/storage/blobs/authorize-access-azure-active-directory).

## Object lifecycle contract

1. Authenticate, authorize and validate the request before storage access. Retain
   actual-file decoding, MIME checks, byte/pixel limits and metadata stripping.
2. Generate object IDs server-side; persist upload intent and ownership before
   writing bytes. Map article `PutMode::Create` to a conditional create, such as
   Azure `If-None-Match: *`, and treat a pre-existing object as a conflict. Azure's
   default overwrite behavior must not replace this protection. Validate against
   the [Put Blob contract](https://learn.microsoft.com/en-us/rest/api/storageservices/put-blob).
3. Confirm upload success before marking media ready. Attach the object only to the
   same article at the expected version. Concurrency, timeout and stale-version
   failures must not attach a cover or report success. Preserve cleanup intent for
   bytes written before a failed database attachment.
4. Read the authorized metadata first, select the fixed service container, fetch
   bounded ciphertext, decrypt and verify integrity. Keep published-state checks
   for Patient covers and clinical relationship checks for private documents.
5. Map missing-object deletion to success; retry other errors through existing
   durable cleanup. Article maintenance may delete only its own old unreferenced
   media, never clinical objects. Preserve reference rechecks and upload leases.
6. Review Azure soft delete, versioning, legal holds and retention explicitly.
   Logical deletion can leave recoverable versions; it must not be described as
   physical erasure. Clinical retention requirements take priority over generic
   abandoned-upload rules. Do not add a second scheduler.

## Future environment configuration

These are configuration requirements to design, **not active environment variable
names or requests to supply values during stabilization**.

| Environment | Required inputs and protection |
|---|---|
| Local unit/integration | Existing in-memory store and synthetic encryption key in isolated fixtures; no cloud credentials. |
| Local Azure adapter tests | Explicit Azurite endpoint and isolated clinical/article containers; emulator-only identity/key, protected synthetic app-encryption key, owned test data and loopback binding. Never point fixture cleanup at a shared account. |
| Staging | Approved HTTPS account URL, separate private container names, explicit identity mode/client ID if required, container-scoped RBAC, protected app-encryption key reference, retry/timeout and size-limit settings. |
| Production | Approved HTTPS account URL and containers, managed/workload identity, secret-manager access for the existing app-encryption key, reviewed network restrictions, retention/backup policy, monitoring and ownership of cleanup failures. No developer-credential fallback. |

Test the adapter contract against [Azurite](https://learn.microsoft.com/en-us/azure/storage/common/storage-use-azurite)
for conditional puts, binary round trips, prefix isolation, missing-object deletes,
failed attachment recovery and idempotent cleanup. Continue all in-memory tests.
Azurite does not establish cloud Entra/RBAC, firewall, service retention or production
reliability; those require explicitly authorized staging checks. No Azurite instance
or new storage dependency was introduced in this stabilization task.

## Data inventory and migration gate

Read-only local-development inventory found **zero** referenced keys in the checked
doctor/patient profile-picture, chat attachment, prescription-intake document and
article-media columns; `article_media` also had zero rows. Evidence:
`scratch/milestone6d-stabilization-storage-inventory.json`. This is a local database
observation, not proof that other environments or an external store contain no data.

Before a future cutover, inventory every storage caller and historical metadata
column, including prescription-generated documents and any orphaned objects.
Establish a protected manifest of source namespace, destination container/key,
ciphertext size/hash and required encryption-key references. Do not export patient
content or credentials into ordinary reports.

If the target environment is empty, document that finding and avoid a fabricated
data migration. Otherwise, authorize a bounded migration separately: copy existing
ciphertext without changing article IDs/keys or clinical ownership; validate byte
hashes and authorized decrypted reads; explicitly map any bucket/container metadata;
fence concurrent writes and cleanup during cutover; and test rollback before
switching the single active provider. Do not create an indefinite dual-provider
fallback. Retire source data only under the approved retention and migration plan.

## Required future acceptance

Verify real Admin upload/draft isolation, independent approval, publish, authenticated
Patient cover reads, unpublish denial, object ownership, failure recovery, bounded
reads, encryption compatibility, cleanup and clinical-document regressions against
approved Azure staging resources. Repeat the same persisted article walkthrough in
an actual Patient runtime. None of those cloud checks is claimed by this plan.
