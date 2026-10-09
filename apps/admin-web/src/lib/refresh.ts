type Tokens = { access_token: string; refresh_token: string };
// Concurrent dashboard reads carry the same HttpOnly refresh cookie. Rotate it
// once, rather than treating normal parallel reads as token-reuse attacks.
export function refreshCoordinator(graceMs = 5000) {
  const rotations = new Map<string, Promise<Tokens | null>>();
  return async (refresh: string, rotate: () => Promise<Tokens | null>) => {
    let pending = rotations.get(refresh);
    if (!pending) { pending = rotate(); rotations.set(refresh, pending); }
    try { return await pending; }
    finally { setTimeout(() => rotations.delete(refresh), graceMs).unref(); }
  };
}
export const coordinatedRefresh = refreshCoordinator();
