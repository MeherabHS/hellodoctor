// Dependency-free so Node's test runner can import it directly.
/** Same plain-address rule as the backend's validate_invite_email. */
export function isPlainEmail(value: string): boolean {
  const email = value.trim().toLowerCase();
  const at = email.indexOf('@');
  if (at < 1 || at !== email.lastIndexOf('@') || email.length > 254) return false;
  const local = email.slice(0, at);
  const domain = email.slice(at + 1);
  if (local.length > 64 || !/^[a-z0-9._%+-]+$/.test(local) || local.startsWith('.') || local.endsWith('.') || local.includes('..')) return false;
  const labels = domain.split('.');
  return labels.length >= 2
    && labels.every(l => /^[a-z0-9-]+$/.test(l) && !l.startsWith('-') && !l.endsWith('-'))
    && /^[a-z]{2,}$/.test(labels[labels.length - 1]);
}
