# Azure Blob Storage Setup

Milestone 7B adds an explicit Azure provider to the existing Rust
`object_store` boundary. This guide configures a future Azure VM; it does not
provision resources or enable a deployment by itself.

## Storage layout

Use one Storage Account with two private Blob containers:

- `AZURE_STORAGE_CLINICAL_CONTAINER`: consultation documents, prescriptions,
  chat attachments and profile photos.
- `AZURE_STORAGE_ARTICLE_CONTAINER`: Admin article covers, including draft
  media.

Container names must be distinct, 3–63 characters, lowercase, and use only
letters, digits and single hyphens. Disable anonymous blob access at the
account and container levels. Patient and Doctor clients never receive Blob
URLs, SAS tokens or storage credentials; the authenticated backend reads and
decrypts objects.

## AfyaMkononi / HelloDoctor.Asia production account

Both private containers now exist on this account (confirmed by the
infrastructure team; the application has not yet been deployed against
them). No source change was required or made to use them - the container
names are plain operator configuration.

```dotenv
STORAGE_PROVIDER=azure
STORAGE_ENCRYPTION_KEY_B64=<existing-32-byte-application-key>
AZURE_STORAGE_ACCOUNT_NAME=stafyamkononiprod01
AZURE_STORAGE_AUTH_MODE=managed_identity
AZURE_STORAGE_CLINICAL_CONTAINER=clinical-files
AZURE_STORAGE_ARTICLE_CONTAINER=article-media
AZURE_STORAGE_ENDPOINT=https://stafyamkononiprod01.blob.core.windows.net
AZURE_STORAGE_USE_EMULATOR=false
```

`AZURE_STORAGE_ACCOUNT_KEY` and `AZURE_STORAGE_CLIENT_ID` must stay unset:
the production identity is the VM's *system-assigned* managed identity (a
client ID is only needed for a user-assigned identity), and account_key mode
is reserved for the explicitly-acknowledged exception path below.

Container intent:

- `clinical-files`: consultation documents, prescriptions, patient profile
  images and chat attachments - read/written only via `state.object_storage`
  (`DocumentStorageService`).
- `article-media`: Admin CMS article covers, draft images and other article
  media - read/written only via `state.article_media_storage`
  (`article_media_service.rs`'s own `storage()` accessor). Verified by
  inspection that neither service file ever references the other's handle,
  so the two containers cannot be crossed by a typo'd call site; the backend
  opens them as two independent `Arc<dyn ObjectStore>` values
  (`AppState::new` in `services/backend/src/repository/mod.rs`) specifically
  so "article media can never address clinical blobs."

Startup (`services/backend/src/config.rs::validate_storage_configuration`)
requires both container names whenever `STORAGE_PROVIDER=azure`, rejects a
startup where they're equal, and rejects either name if it isn't 3-63
lowercase letters/digits/single-hyphens - a misconfiguration here fails the
process at boot, not silently at request time.

### Required Azure RBAC

| Container | Identity | Role | Scope |
|---|---|---|---|
| `clinical-files` | `vm-afyamkononi-prod-01` (system-assigned) | Storage Blob Data Contributor | Container only |
| `article-media` | `vm-afyamkononi-prod-01` (system-assigned) | Storage Blob Data Contributor | Container only |

No storage-account-level or administrative role is required for either
assignment: the backend's only Blob operations, repo-wide, are per-blob
`put` (conditional create), `get` and `delete` (confirmed by inspecting
every call site under `services/backend/src/services/`) - it never lists,
copies or creates a container, and never calls anything at the account
level.

**Verification status:** container creation for both `clinical-files` and
`article-media` is confirmed. The two RBAC assignments above have **not**
been independently verified from this repository audit - confirm both role
assignments exist, are scoped to the individual container (not the storage
account), and are active, before relying on them at deployment time.

## Configuration

Add these values to `/etc/hellodoctor/hellodoctor.env`:

```dotenv
STORAGE_PROVIDER=azure
STORAGE_ENCRYPTION_KEY_B64=<existing-32-byte-application-key>
AZURE_STORAGE_ACCOUNT_NAME=<lowercase-account-name>
AZURE_STORAGE_AUTH_MODE=managed_identity
AZURE_STORAGE_CLINICAL_CONTAINER=clinical-private
AZURE_STORAGE_ARTICLE_CONTAINER=article-media
AZURE_STORAGE_CLIENT_ID=<optional-user-assigned-identity-client-id>
AZURE_STORAGE_ENDPOINT=https://<account>.blob.core.windows.net
AZURE_STORAGE_USE_EMULATOR=false
```

For deliberate account-key authentication outside development/test, use:

```dotenv
AZURE_STORAGE_AUTH_MODE=account_key
AZURE_STORAGE_ACCOUNT_KEY=<storage-account-key>
AZURE_STORAGE_CLIENT_ID=
AZURE_STORAGE_ACCOUNT_KEY_RISK_ACKNOWLEDGED=true
```

Startup fails closed with `AZURE_STORAGE_AUTH_MODE=account_key` in staging or
production if `AZURE_STORAGE_ACCOUNT_KEY_RISK_ACKNOWLEDGED` is left at its
default (`false`) — see "Account-key mode" below for what the acknowledgment
does and does not cover.

Do not set both an account key and managed-identity mode. The Azure Storage
Account Key authenticates Blob requests; it is not the AES-256-GCM key used to
decrypt existing HelloDoctor objects. Losing or replacing
`STORAGE_ENCRYPTION_KEY_B64` makes previously encrypted objects unreadable.

## Managed identity and RBAC

1. Create or select the VM's system-assigned or approved user-assigned managed
   identity.
2. Grant only the required data-plane role on the storage account or the two
   selected containers. `Storage Blob Data Contributor` is the baseline for
   server-side create/read/delete; use a narrower custom role if the security
   review requires it.
3. If using a user-assigned identity, place its client ID in
   `AZURE_STORAGE_CLIENT_ID`.
4. Do not grant the identity account-management, key-management or public
   anonymous-access permissions.
5. Keep the storage account firewall/private endpoint policy aligned with the
   VM subnet and approved egress path.

## Account-key mode

Managed identity is the required default outside development/test.
`AZURE_STORAGE_AUTH_MODE=account_key` is rejected at startup in staging and
production unless `AZURE_STORAGE_ACCOUNT_KEY_RISK_ACKNOWLEDGED=true` is also
set. The acknowledgment defaults to `false`; it must be set explicitly, and it
only lifts this one gate — it does not relax container-name validation, the
32-byte encryption-key check, endpoint/HTTPS rules, the emulator restriction,
or any other Azure check in `services/backend/src/config.rs`, and it grants no
Azure RBAC permission by itself.

Use account-key mode only when explicitly approved. The operational risk this
exception accepts:

- The shared account key is a long-lived, full-account-scope credential —
  unlike managed identity, it cannot be scoped to a single container or role,
  and possessing it grants whatever data-plane permissions the account key
  carries over both the clinical and article containers.
- It must be rotated manually through the Azure operator process; there is no
  automatic expiry, and a leaked key remains valid until someone rotates it.
- It has to be distributed to and stored by the deployment (VM environment
  file or attached secret manager), which is an additional secret-custody
  surface managed identity does not require.
- Setting `AZURE_STORAGE_ACCOUNT_KEY_RISK_ACKNOWLEDGED=true` is an
  application-level configuration acknowledgment only. It does not configure,
  restrict, or verify any Azure RBAC role, firewall rule, or Conditional
  Access policy — those remain the operator's responsibility regardless of
  this flag.

Store the key in the private VM environment file or an attached secret
manager, rotate it through the Azure operator process, and restart the
backend after rotation. Never place it in source control, Docker image
layers, Next.js `NEXT_PUBLIC_` variables, Flutter build defines or logs.

## Local Azurite tests

Azurite is supported only for development/test fixtures. Keep the emulator on
loopback and never enable it in staging or production:

```dotenv
STORAGE_PROVIDER=azure
STORAGE_ENCRYPTION_KEY_B64=<synthetic-test-key>
AZURE_STORAGE_ACCOUNT_NAME=devstoreaccount1
AZURE_STORAGE_AUTH_MODE=azurite
AZURE_STORAGE_CLINICAL_CONTAINER=clinical-private
AZURE_STORAGE_ARTICLE_CONTAINER=article-media
AZURE_STORAGE_USE_EMULATOR=true
AZURE_STORAGE_ENDPOINT=http://127.0.0.1:10000
# object_store emulator endpoint override when the Azurite port is non-default
AZURITE_BLOB_STORAGE_URL=http://127.0.0.1:10000
```

The repository's isolated test uses the standard Azurite account and an
explicitly owned fixture. Azurite proves adapter behavior only; it does not
prove Azure Entra/RBAC, firewall, retention, backup or managed-identity
behavior.

## Startup and connectivity checks

Run the secret-free preflight before starting the backend:

```sh
HELLODOCTOR_ENV_FILE=/etc/hellodoctor/hellodoctor.env \
  cargo run --release --example config_preflight
```

Preflight validates provider selection, account/container names, auth-mode
compatibility, encryption-key presence, endpoint safety and production
emulator restrictions. It does not claim cloud connectivity. Verify actual
connectivity with a synthetic object in each container through the existing
authenticated backend upload/download paths, then delete those objects and
confirm the audit/cleanup result.

## Encryption, ownership and cleanup

HelloDoctor continues to encrypt object bytes with AES-256-GCM and verify the
stored SHA-256 hash after download. Azure encryption at rest is complementary.
Create-only uploads use the object-store `PutMode::Create` contract, which the
pinned Azure adapter maps to a conditional `If-None-Match: *` request. Draft
article media remains private and is read only after the existing article
version/authorization checks. Abandoned uploads and account-erasure cleanup
continue through the existing durable jobs; missing objects are treated as
already deleted, while other failures remain retryable.

## Backup, rotation and migration

Do not automatically migrate existing objects. Before any cutover, inventory
all metadata rows and object namespaces, preserve a protected manifest of keys,
hashes and encryption-key references, fence concurrent writes, validate
authorized decrypted reads, and rehearse rollback. Retention, soft-delete,
versioning and legal-hold settings must be reviewed with clinical and financial
retention requirements before production activation.

## When Azure is ready: values the operator must supply.

The operator must supply the Storage Account name, two private container names,
the approved managed identity (and optional client ID) or account key, the
private endpoint/firewall policy, and the existing
`STORAGE_ENCRYPTION_KEY_B64`. No source-code edit is required for those values.
