/* AfyaMkononi: account deletion request form.
 *
 * The site has no server: a valid request opens in the visitor's own email
 * app, addressed to the privacy office, and nothing is sent or stored here.
 * Abuse protection on this page:
 *   - Zod schema: every field is checked, trimmed and length-capped, and
 *     only the listed options are accepted.
 *   - Honeypot: a hidden "website" field people never see; bots fill it.
 *   - Time trap: a form completed within 3 seconds of loading is a bot.
 *   - Cooldown: one request a minute and five a day from this browser.
 * Bots that trip the honeypot or time trap get the normal success message
 * so they learn nothing, but no email is opened.
 */
import { z } from './assets/vendor/zod/v4/mini/index.js';

// No generated code (`new Function`), so the Content Security Policy can
// keep forbidding eval.
z.config({ jitless: true });

const TO = 'info@rm-consultants.asia';
const MIN_FILL_MS = 3000;
const COOLDOWN_MS = 60 * 1000;
const DAILY_MAX = 5;
const STORE_KEY = 'am-deletion-requests';

// Strip control characters (including line breaks) so nothing can be smuggled
// into the email subject, then collapse runs of spaces.
const clean = (max) => z.pipe(
  z.string(),
  z.transform((v) => v.replace(/[\u0000-\u001f\u007f]/g, ' ').replace(/\s+/g, ' ').trim().slice(0, max + 1)),
);

const DeletionRequest = z.object({
  phone: z.pipe(
    z.pipe(z.string(), z.transform((v) => v.replace(/[\s()-]/g, '').slice(0, 20))),
    z.string().check(z.regex(/^(?:\+254|254|0)(?:7|1)\d{8}$/, 'Please enter a valid Kenyan mobile number, e.g. 0712 345 678 or +254 712 345 678.')),
  ),
  email: z.pipe(
    clean(254),
    z.string().check(
      z.maxLength(254, 'That email address is too long.'),
      z.regex(/^[^\s@]+@[^\s@]+\.[^\s@]{2,}$/, 'Please enter a valid email address.'),
    ),
  ),
  name: z.pipe(
    clean(100),
    z.string().check(
      z.minLength(2, 'Please enter your full registered name.'),
      z.maxLength(100, 'Please shorten the name to 100 characters.'),
      z.regex(/^[\p{L}\p{M}][\p{L}\p{M} .'’-]*$/u, 'The name can only contain letters, spaces, apostrophes, dots and hyphens.'),
    ),
  ),
  account_type: z.enum(['patient', 'doctor', 'peer', 'guardian'], 'Please choose an account type.'),
  reason: z.enum(['', 'no-longer-needed', 'privacy-concerns', 'switching-providers', 'technical-issues', 'other']),
  confirm: z.literal(true, 'Please tick the confirmation box.'),
  website: z.string().check(z.maxLength(0)), // honeypot: must stay empty
});

const FIELD_IDS = {
  phone: 'del-phone', email: 'del-email', name: 'del-name',
  account_type: 'del-type', reason: 'del-reason', confirm: 'del-confirm',
};

const readLog = () => {
  try {
    const log = JSON.parse(localStorage.getItem(STORE_KEY) || '[]');
    return Array.isArray(log) ? log.filter((t) => Number.isFinite(t)) : [];
  } catch { return []; }
};
const writeLog = (log) => {
  try { localStorage.setItem(STORE_KEY, JSON.stringify(log)); } catch { /* private mode */ }
};

const form = document.getElementById('deletion-request-form');
if (form) {
  const status = document.getElementById('deletion-status');
  const loadedAt = performance.now();
  const show = (msg, ok) => {
    status.hidden = false;
    status.textContent = msg;
    status.classList.toggle('is-error', !ok);
  };
  const fakeSuccess = () => show(`Your email app should now open with the request addressed to ${TO}. Send it to complete your request.`, true);

  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const data = new FormData(form);
    const input = {
      phone: String(data.get('phone') ?? ''),
      email: String(data.get('email') ?? ''),
      name: String(data.get('name') ?? ''),
      account_type: String(data.get('account_type') ?? ''),
      reason: String(data.get('reason') ?? ''),
      confirm: data.get('confirm') === 'on',
      website: String(data.get('website') ?? ''),
    };

    // Bots: look successful, do nothing.
    if (input.website || performance.now() - loadedAt < MIN_FILL_MS) {
      fakeSuccess();
      return;
    }

    const now = Date.now();
    const log = readLog().filter((t) => now - t < 24 * 60 * 60 * 1000);
    if (log.length && now - log[log.length - 1] < COOLDOWN_MS) {
      show('A request was just prepared. Please wait a minute before trying again.', false);
      return;
    }
    if (log.length >= DAILY_MAX) {
      show(`Too many requests from this browser today. Please email ${TO} directly.`, false);
      return;
    }

    const result = DeletionRequest.safeParse(input);
    Object.values(FIELD_IDS).forEach((id) => document.getElementById(id)?.setAttribute('aria-invalid', 'false'));
    if (!result.success) {
      const issue = result.error.issues[0];
      const field = document.getElementById(FIELD_IDS[issue.path[0]]);
      field?.setAttribute('aria-invalid', 'true');
      show(issue.message || 'Please check the highlighted field.', false);
      field?.focus();
      return;
    }

    const v = result.data;
    const label = (id, value) => {
      const opt = Array.from(document.getElementById(id).options).find((o) => o.value === value);
      return opt ? opt.text : value;
    };
    const body = [
      'Please delete my AfyaMkononi account.',
      '',
      `Full name: ${v.name}`,
      `Registered mobile number: ${v.phone}`,
      `Registered email: ${v.email}`,
      `Account type: ${label('del-type', v.account_type)}`,
      `Reason: ${v.reason ? label('del-reason', v.reason) : 'Not given'}`,
      '',
      'I confirm that I am the authorised holder of this account.',
    ].join('\n');
    const subject = `Account Deletion Request - ${v.name} / ${v.phone}`;

    log.push(now);
    writeLog(log);
    window.location.href = `mailto:${TO}?subject=${encodeURIComponent(subject)}&body=${encodeURIComponent(body)}`;
    show(`Your email app should now open with the request addressed to ${TO}. Send it to complete your request. If nothing opens, email ${TO} with the details above.`, true);
  });
}
