# ARM64 GHCR publication and digest handoff

This is a publication plan only. It does not log in to GHCR, push images, trigger Actions, or change package visibility.

## Package names

Use private packages under the repository owner’s GHCR namespace:

```text
ghcr.io/<owner>/hellodoctor-backend
ghcr.io/<owner>/hellodoctor-admin-web
ghcr.io/<owner>/hellodoctor-doctor-web
ghcr.io/<owner>/hellodoctor-postgres-pgbackrest
```

Use an immutable source revision tag such as `arm64-<full-git-sha>`. Do not use `latest` for the integration handoff. The package visibility must remain **Private** in the GitHub package settings.

## Required permissions

The publication job needs only:

```yaml
permissions:
  contents: read
  packages: write
```

It must use the repository-scoped `GITHUB_TOKEN` for GHCR authentication. No Azure credential, database password, JWT key, FCM key, payment credential, Blob key, or production `.env` file belongs in this job. No `id-token: write` permission is needed for the initial GHCR handoff.

## Authorized publication sequence

After the ARM64 build workflow has passed on the exact source revision:

1. Record the full `SOURCE_REVISION` printed by every ARM64 job.
2. Build each image on a native ARM64 runner with `--platform linux/arm64`.
3. Log in to `ghcr.io` with the scoped `GITHUB_TOKEN`.
4. Push the four images under the immutable `arm64-<full-git-sha>` tag.
5. Inspect each pushed reference with `docker buildx imagetools inspect`.
6. Record the top-level manifest digest and the ARM64 child digest. The integration workflow should receive the top-level digest-pinned reference, for example:

```text
ghcr.io/<owner>/hellodoctor-backend@sha256:<manifest-digest>
ghcr.io/<owner>/hellodoctor-admin-web@sha256:<manifest-digest>
ghcr.io/<owner>/hellodoctor-doctor-web@sha256:<manifest-digest>
```

7. Confirm the package is private and that the digest resolves from the ARM64 integration runner.
8. Dispatch `.github/workflows/arm64-disposable-integration.yml` with those three application image references.

The pgBackRest image is built and checked by CI. Publish it separately only when the backup/PITR validation job is ready to consume it; the six-service workflow does not need to pull it.

## Digest handoff record

Store the following as CI run evidence or an access-controlled release note, not in a public repository file:

```text
SOURCE_REVISION=<full git SHA>
BACKEND_RUNTIME_IMAGE=ghcr.io/<owner>/hellodoctor-backend@sha256:<digest>
ADMIN_WEB_IMAGE=ghcr.io/<owner>/hellodoctor-admin-web@sha256:<digest>
DOCTOR_WEB_IMAGE=ghcr.io/<owner>/hellodoctor-doctor-web@sha256:<digest>
PUBLISHED_BY=<workflow run URL>
PUBLISHED_AT=<UTC timestamp>
PACKAGE_VISIBILITY=private
ARM64_CHILD_DIGESTS=<record each child digest>
```

## Expected usage and charges

The four native ARM64 build jobs and the disposable integration job consume GitHub Actions minutes. Private repositories charge according to the repository’s Actions plan and runner rates; public repositories may have different included/free treatment. GHCR storage and image transfer may incur account-specific costs or quota effects. The initial handoff should use one source revision, one immutable tag, and disposable images only. No Azure compute, registry push, or workflow charge has been incurred by this preparation.
