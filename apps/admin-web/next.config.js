/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
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
        ],
      },
    ];
  },
};

module.exports = nextConfig;
