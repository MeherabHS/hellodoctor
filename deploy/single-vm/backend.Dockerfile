ARG BACKEND_IMAGE
FROM ${BACKEND_IMAGE}
USER root
RUN apt-get update && apt-get install -y --no-install-recommends curl && rm -rf /var/lib/apt/lists/*
USER hellodoctor:hellodoctor
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 CMD curl --fail --silent http://127.0.0.1:8080/healthz > /dev/null || exit 1
