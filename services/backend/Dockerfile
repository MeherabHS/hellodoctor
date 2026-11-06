# Capacity-testing image for the HelloDoctor backend — lets Docker enforce
# real CPU/RAM limits (see docker-compose.capacity.yml) that a bare .exe
# running directly on the host cannot be constrained by. Not (yet) the
# production deployment image — just what docs/CONSTRAINED_CAPACITY_TEST.md
# needs to make "simulated 2-vCPU VPS" an enforced fact, not a claim.
#
# Base images are pinned to reviewed OCI manifest-list digests. The manifest
# digest is deliberately used here (rather than an amd64 child digest), so
# Buildx resolves the native image for the requested target platform.
# Verify a replacement with:
#   docker buildx imagetools inspect rust:1-bookworm
#   docker buildx imagetools inspect debian:bookworm-slim

FROM rust:1-bookworm@sha256:c9155318d07341301a78b45ecdd81af2e43b8b297d64e8fd4c5e80d7735607c3 AS builder
ARG TARGETARCH
WORKDIR /app

# Fail the build if an ARM64 build accidentally uses a host/toolchain for a
# different architecture. The amd64 branch keeps the existing multi-platform
# build contract intact.
RUN set -eux; host="$(rustc -vV | sed -n 's/^host: //p')"; \
    case "$TARGETARCH:$host" in \
      arm64:aarch64-unknown-linux-gnu|amd64:x86_64-unknown-linux-gnu) ;; \
      *) echo "unexpected Rust host $host for TARGETARCH=$TARGETARCH" >&2; exit 1 ;; \
    esac

# Cache the (large) dependency set separately from our own source so an
# iterative rebuild only recompiles our own crate, not every dependency.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    echo "" > src/lib.rs && \
    cargo build --release && \
    rm -rf src

COPY src ./src
COPY migrations ./migrations
RUN touch src/main.rs src/lib.rs && cargo build --release --bin hellodoctor-backend

FROM debian:bookworm-slim@sha256:99c0b25c46d0214d5a2091092e7908907dcaecd6a962cde0ac700acee222b6f3 AS runtime
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/* && \
    groupadd --system --gid 10001 hellodoctor && \
    useradd --system --uid 10001 --gid hellodoctor \
        --no-create-home --shell /usr/sbin/nologin hellodoctor
WORKDIR /app
COPY --from=builder --chown=hellodoctor:hellodoctor /app/target/release/hellodoctor-backend ./hellodoctor-backend
COPY --chown=hellodoctor:hellodoctor migrations ./migrations
USER hellodoctor:hellodoctor
EXPOSE 8080
CMD ["./hellodoctor-backend"]
