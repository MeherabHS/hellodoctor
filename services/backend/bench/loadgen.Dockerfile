# Load generator image for the performance-assurance runs
# (deploy/single-vm/compose.perf-baseline.yml). Builds the existing harnesses
# (examples/capacity_test.rs, examples/web_workload.rs) from the same source
# and toolchain digest as the backend image. Test tooling only; never deployed.
FROM rust:1-bookworm@sha256:c9155318d07341301a78b45ecdd81af2e43b8b297d64e8fd4c5e80d7735607c3 AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
COPY examples ./examples
RUN cargo build --release --example capacity_test --example web_workload --example auth_stress_test

# Root on purpose (reviewed 2026-11-24): test tooling that is never deployed,
# writing results into a host bind mount (../../docs/assurance/evidence),
# which a fixed non-root uid cannot write on every Linux perf host.
FROM debian:bookworm-slim@sha256:99c0b25c46d0214d5a2091092e7908907dcaecd6a962cde0ac700acee222b6f3
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/examples/capacity_test /usr/local/bin/capacity_test
COPY --from=builder /app/target/release/examples/web_workload /usr/local/bin/web_workload
COPY --from=builder /app/target/release/examples/auth_stress_test /usr/local/bin/auth_stress_test
WORKDIR /evidence
