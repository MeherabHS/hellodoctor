# HelloDoctor VM Configuration Guide

This guide prepares a future Azure VM without provisioning it. It uses one private server-side file and explicit service mappings.

## 1. Create the private environment file

On the VM, create the file as root and keep it out of the repository:

```sh
sudo install -d -m 0750 -o root -g root /etc/hellodoctor
sudo install -m 0600 -o root -g root /dev/null /etc/hellodoctor/hellodoctor.env
sudoedit /etc/hellodoctor/hellodoctor.env
```

Start from the repository [`.env.example`](../.env.example). Replace placeholders with approved values. Never paste credentials into source, Flutter `--dart-define`, browser `NEXT_PUBLIC_` variables or a GitHub workflow.

## 2. Validate before starting services

Run the backend preflight without printing values:

```sh
cd /path/to/hellodoctor/services/backend
HELLODOCTOR_ENV_FILE=/etc/hellodoctor/hellodoctor.env \
  cargo run --release --example config_preflight
```

The process validates runtime mode, database-role settings, Redis, HTTPS/proxy requirements, FCM completeness, monitoring credentials, storage requirements and the password-reset fail-closed policy. A validation error must be fixed before startup.

For a container image, run the equivalent command with the same private file mounted read-only and `HELLODOCTOR_ENV_FILE` set inside the backend container. Do not mount the private file into browser containers.

## 3. Start the approved images

The checked-in Compose design is intentionally an example because image registry names and ingress topology are operator choices:

```sh
docker compose --env-file /etc/hellodoctor/hellodoctor.env \
  -f deploy/docker-compose.vm.example.yml config
docker compose --env-file /etc/hellodoctor/hellodoctor.env \
  -f deploy/docker-compose.vm.example.yml up -d
```

Set `BACKEND_IMAGE`, `ADMIN_WEB_IMAGE` and `DOCTOR_WEB_IMAGE` in the private file. The backend receives the private file as a server-side environment source. Admin Web and Doctor Web receive only their allowlisted BFF settings. Put TLS, public DNS and rate-limiting at the approved reverse proxy; bind application ports to loopback as shown.

## 4. Password recovery policy

Keep `PASSWORD_RESET_ENABLED=false`. The current source has no password-reset endpoint or UI. Setting it to true fails closed at backend startup, even if email credentials exist, until a separately reviewed and tested recovery implementation is delivered. Existing MFA, session revocation, logout and authenticated password-change paths are unaffected.

## 5. Provider-specific values

- **Firebase FCM:** supply the project ID, client email and base64 service-account private key together for staging/production. The backend sends FCM; mobile apps receive only their platform Firebase configuration.
- **Agora:** supply the backend app ID, certificate and optional provider callback HMAC. Never put the certificate in Flutter.
- **Email:** supply the existing Azure Communication Services endpoint, access key and verified sender only when the corresponding server-side email feature is enabled.
- **Azure Blob:** set `STORAGE_PROVIDER=azure` only after completing the setup checklist in [AZURE_BLOB_SETUP.md](AZURE_BLOB_SETUP.md). The backend requires distinct private clinical/article containers and the existing application encryption key; it never falls back to S3.

## 6. Health and acceptance checks

After startup, verify the backend health endpoint through the trusted ingress, web BFF login flows, database migration state, Redis connectivity and role-separated access. Use synthetic accounts and disposable test data. Do not run acceptance checks against real clinical records or send real notifications during preparation.

## 7. Rotation and restart

Runtime secrets require a service restart. Rotate database role passwords together with the backend bootstrap process, rotate JWT/TOTP/HMAC/FCM keys under the existing revocation policy, and remove old values from the VM environment after the new processes are healthy. Browser/mobile build-time values require rebuilding the affected artifact; server secrets never do.

## When Azure is ready: values the operator must supply.

The operator will later supply the Azure Storage Account name, authentication mode (managed identity preferred), private clinical and article-media container names, optional user-assigned identity client ID or authorized account credential, and the existing application encryption key. The Azure adapter milestone must validate and consume these values before any cloud upload is accepted.
