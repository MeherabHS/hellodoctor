# Production deployment checklist

This repository contains development and capacity compose files only. Before
production deployment, create a separate deployment configuration with:

- Internet -> firewall -> TLS reverse proxy -> Rust backend;
- PostgreSQL and Redis on a private network, never public ports;
- managed secrets rather than committed `.env` files;
- migrations/role provisioning as a separate deployment step;
- restricted runtime DB role credentials;
- encrypted off-host PostgreSQL backups and a restore drill;
- health, error, payment-reconciliation, CPU/memory, disk, and backup alerts;
- explicit UTC/NTP configuration and an incident/on-call procedure.

Do not reuse `docker-compose.capacity.yml` as a production manifest.

## PUBLIC INTERNET EXPOSURE REQUIREMENTS

Development must not be internet-exposed. Production requires HTTPS termination, explicit safe environment configuration, no development OTP/seed fixtures, protected metrics, blocked simulation endpoints, fail-closed rate limiting and immediate session invalidation. The implemented application guards and exact proxy/client configuration are documented in [Public exposure hardening](PUBLIC_EXPOSURE_HARDENING.md). External TLS infrastructure is required and was not installed. Sensitive Admin step-up authentication remains open; these checks do not declare general production readiness.
