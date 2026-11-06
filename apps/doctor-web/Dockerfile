# Build without runtime secrets. This is the reviewed multi-platform
# node:22-bookworm-slim manifest digest; Buildx selects its ARM64 child.
ARG NODE_IMAGE=node@sha256:4b150b48b4116e2a22f2c39cc27025fbf4f153e2078fd3b14d159fdb5c7279da
FROM ${NODE_IMAGE} AS build
WORKDIR /app
ENV NEXT_TELEMETRY_DISABLED=1
COPY package.json package-lock.json ./
# Keep platform-specific optional packages such as Next SWC and Sharp.
RUN npm ci --include=optional --no-audit --no-fund
COPY . .
RUN mkdir -p public && npm run build

FROM ${NODE_IMAGE} AS runtime
WORKDIR /app
ENV NODE_ENV=production NEXT_TELEMETRY_DISABLED=1 HOSTNAME=0.0.0.0 PORT=3000
COPY --from=build --chown=node:node /app/.next/standalone ./
COPY --from=build --chown=node:node /app/.next/static ./.next/static
COPY --from=build --chown=node:node /app/public ./public
USER node
EXPOSE 3000
# Full ingress/auth checks are performed by the verification harness.
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
  CMD node -e "const s=require('net').connect(3000,'127.0.0.1');s.on('connect',()=>{s.end();process.exit(0)});s.on('error',()=>process.exit(1));s.setTimeout(3000,()=>process.exit(1))"
CMD ["node","server.js"]
