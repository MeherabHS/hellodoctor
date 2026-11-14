/** @type {import('next').NextConfig} */
const nextConfig = {
  output: 'standalone',
  reactStrictMode: true,
  // Dev-server-only (ignored in production builds): Next 16 blocks
  // cross-origin HMR/dev-resource requests by default. ADMIN_WEB_ORIGIN
  // (the CSRF origin check in middleware.ts) is deliberately 127.0.0.1, not
  // localhost, so local dev access needs this explicit allow — without it,
  // the login POST itself succeeds but the client-side app silently breaks
  // right after (HMR/dev-resource requests get rejected), which is what
  // looked like the dashboard going blank post-login.
  allowedDevOrigins: ['127.0.0.1'],
  // No rewrites to the backend anymore — the previous rule here transparently
  // passed browser requests straight through to the Rust backend with no
  // authentication attached at all. Every backend call now goes through
  // src/app/api/proxy/[...path]/route.ts instead, which attaches the admin's
  // session server-side from an HttpOnly cookie.
  async headers() {
    // Matches the CSP already specified for this admin web shell in
    // docs/AUTH_SECURITY.md §7.2, plus the standard adjacent headers that
    // section's surrounding anti-injection/clickjacking guidance implies.
    // Script policy is supplied with a per-request nonce by middleware.

    return [
      {
        source: '/:path*',
        headers: [
          { key: 'X-Frame-Options', value: 'DENY' },
          { key: 'X-Content-Type-Options', value: 'nosniff' },
          { key: 'Referrer-Policy', value: 'no-referrer' },
          { key: 'Permissions-Policy', value: 'camera=(), microphone=(), geolocation=(), payment=()' },
          // HTTPS only for two years, production builds only (local dev is plain http).
          ...(process.env.NODE_ENV === 'production' ? [{ key: 'Strict-Transport-Security', value: 'max-age=63072000; includeSubDomains' }] : []),
        ],
      },
    ];
  },
};

module.exports = nextConfig;
