# Capacity-testing image for the HelloDoctor backend — lets Docker enforce
# real CPU/RAM limits (see docker-compose.capacity.yml) that a bare .exe
# running directly on the host cannot be constrained by. Not (yet) the
# production deployment image — just what docs/CONSTRAINED_CAPACITY_TEST.md
# needs to make "simulated 2-vCPU VPS" an enforced fact, not a claim.

FROM rust:1-bookworm AS builder
WORKDIR /app

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

FROM debian:bookworm-slim AS runtime
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/hellodoctor-backend ./hellodoctor-backend
COPY migrations ./migrations
EXPOSE 8080
CMD ["./hellodoctor-backend"]
