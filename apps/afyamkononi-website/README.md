# AfyaMkononi — Product Website (Kenya)

AfyaMkononi is a focused, public-facing digital healthcare product website designed for Kenya under the Sauti V4 editorial design language.

## 1. Locked Website Structure (6 Pages)

| Page | Route | File | Key Contents |
| :--- | :--- | :--- | :--- |
| **Home** | `/` | `index.html` | Brand identity, Sauti V4 editorial hero, How it works, **For Patients** section with real Flutter App carousel, **For Doctors** section with real Doctor Flutter App carousel, Trust & Safety, About preview, and final download CTA. |
| **About** | `/about` | `about.html` | Mission, bilingual dignity (English & Kiswahili), 4-party governance model (GHI, Circle Technology Ltd, AfyaMkononi.app, RMCL), and platform technology. |
| **Support & Safety** | `/support` | `support.html` | Emergency Casualty Warning, technical support, account services & Google Play web deletion procedure, patient grievances, doctor support, M-Pesa billing, privacy inquiries, and ODPC/KMPDC escalation. |
| **Privacy Policy** | `/privacy` | `privacy.html` | Full 19-section Kenya Data Protection Act 2019 policy, 5-day temporary trace rule, retention principles, and Google Play health data disclosures. |
| **Terms of Service** | `/terms` | `terms.html` | Full 29-section Terms of Service, medical device disclaimer, emergency limitations, consultation booking, payments, cancellations, and Kenyan governing law. |
| **Patient Consent** | `/patient-consent` | `patient-consent.html` | Patient Telemedicine and Health Data Consent under the Kenya Health Act 2017 & Digital Health Act 2023. The old `/consent` address (a duplicate copy) 301-redirects here. |
| **Peer Navigator Privacy Notice** | `/peer-navigator-privacy` | `peer-navigator-privacy.html` | Version 1.0 of the notice Peer Navigators accept at sign-up in the Care app (same text as the backend's `PEER_PRIVACY_NOTICE`): data held, purposes and lawful bases, who sees it, patient information peers see, retention, rights, contacts. |

*Note: Homepage navigation links directly to `/#patients` and `/#doctors`. No separate `/patients` or `/doctors` pages exist.*

---

## 2. Interactive App Screenshot Carousels

- **Patient Showcase (`#patients`)**:
  - Refined device shell with titanium bezel, speaker grille, camera notch, and shadow.
  - 7 genuine Flutter Patient App screens: Dashboard, Doctor Discovery, Clinician Credentials, Appointment Booking, Encrypted Clinical Chat, M-Pesa Confirmation, and Care Continuity.
  - Interactive touch/swipe, keyboard navigation (Left/Right/Home/End), previous/next controls, pill indicators, and ARIA live announcements.
- **Doctor Showcase (`#doctors`)**:
  - Distinct midnight-forest art direction (`var(--v4-deep)` and luminous ivory with ochre signals).
  - 7 genuine Doctor Flutter App screens: Patient Queue & Triage, Sovereign Schedule Controls, Centralized Inbox, Clinical Encounter Interface, Smart Rx Digital Prescriptions, Community Q&A Advice, and Practice Earnings Wallet.
  - Full keyboard accessibility, touch/swipe gestures, and synchronized editorial captions.

---

## 3. Local Preview

Start the local preview server from the repository root (it serves clean URLs like production):

```powershell
python apps/afyamkononi-website/tools/serve.py 4173
```

Then open http://127.0.0.1:4173/ (`/about`, `/support`, `/privacy`, ...).

### Clean URLs (no `.html`)

Internal links use clean, root-relative routes (`/about`, `/#patients`), so the site must be served from the domain root by a host that maps `/about` to `about.html`:

- **Azure Static Web Apps (host):** `staticwebapp.config.json` sets `"trailingSlash": "never"`, which serves `/about` from `about.html` and 301-redirects `/about.html` to `/about`.
- **Azure App Service (Windows), if used instead:** `web.config` does the same with IIS URL Rewrite.
- **Azure Storage static website:** cannot rewrite URLs or set headers; do not use it.

Opening the files directly (`file://`) no longer navigates between pages; use the preview server.

### Motion

`site.css` (Motion section) and `site.js` add cross-fades between pages (View Transitions), a header entrance, staggered scroll reveals, floating app phones, hover/press feedback and a reading-progress line. All of it switches off when the visitor's system asks for reduced motion.

Open in your browser:
- Home: [http://127.0.0.1:4173/](http://127.0.0.1:4173/)
- Patients Section: [http://127.0.0.1:4173/#patients](http://127.0.0.1:4173/#patients)
- Doctors Section: [http://127.0.0.1:4173/#doctors](http://127.0.0.1:4173/#doctors)
- About: [http://127.0.0.1:4173/about.html](http://127.0.0.1:4173/about.html)
- Support & Safety: [http://127.0.0.1:4173/support.html](http://127.0.0.1:4173/support.html)
- Privacy Policy: [http://127.0.0.1:4173/privacy.html](http://127.0.0.1:4173/privacy.html)
- Terms of Service: [http://127.0.0.1:4173/terms.html](http://127.0.0.1:4173/terms.html)
- Patient Consent: [http://127.0.0.1:4173/patient-consent](http://127.0.0.1:4173/patient-consent)

---

## 4. Assets & Previews

- Complete screenshot registry and provenance: [`ASSET-MANIFEST.md`](ASSET-MANIFEST.md)
- Production checklist: [`PRODUCTION-CHECKLIST.md`](PRODUCTION-CHECKLIST.md)
- Previews directory: `afyamkononi-website/previews/`

## 4. Security

**Deploy only the built folder.** `python apps/afyamkononi-website/tools/build.py` copies the published pages and only the CSS, JS and images they reference into `apps/afyamkononi-website/dist/`. Point Azure Static Web Apps' `app_location` at that folder (no build step). Drafts (`concept-*`), older design versions (`sauti-v2/v3`, `styles.css`), notes (`*.md`), screenshots (`previews/`, 66 MB) and helper scripts (`scratch/`) never reach the server.

**Defence in depth if the raw folder is ever deployed:** `staticwebapp.config.json` returns 404 for those paths, for dot-files and config files, and for Azure's built-in sign-in endpoints (`/.auth/*`; the site has no sign-in). Write methods (POST/PUT/PATCH/DELETE) get 405.

**Security headers on every response:**

| Header | Value / purpose |
| :--- | :--- |
| Content-Security-Policy | Scripts, fonts, images and requests from this site only; no inline script, no plugins, no `<base>` tag, no form posts, cannot be framed. Styles allow inline `style=""` attributes used by the legal pages. |
| Strict-Transport-Security | HTTPS only for 2 years, including subdomains. |
| X-Frame-Options: DENY | No clickjacking through iframes (older browsers). |
| X-Content-Type-Options: nosniff | Files are only run as their declared type. |
| Referrer-Policy | Other sites see only `https://afyamkononi.app`, never the page path. |
| Permissions-Policy | Camera, microphone, location, payment, USB, sensors and ad-topic tracking switched off. |
| Cross-Origin-Opener/Resource-Policy | Other sites cannot script this window or embed its files. |

**Code rules that keep the policy working:** no inline `<script>` blocks and no `onclick=""` attributes. Page behaviour lives in `site.js` (and `account-deletion.js`); buttons use `data-am-action="print" | "top" | "toggle-toc"`. Text is inserted with `textContent`, never `innerHTML`. No third-party scripts, fonts or trackers are loaded.

**Account deletion form abuse protection** (`account-deletion.js`, loaded only on that page): a Zod schema (Zod 4 mini, self-hosted under `assets/vendor/zod/`, English messages only, `jitless` so the CSP can keep forbidding eval) checks every field: Kenyan mobile format, email, a letters-only name capped at 100 characters, and only the listed account types and reasons. Control characters and line breaks are stripped so nothing can be smuggled into the email. A hidden honeypot field and a 3-second time trap catch bots, which are shown the normal success message but no email opens. One request a minute and five a day per browser. The form never sends data to a server: a valid request opens in the visitor's own email app.

**Rate limiting, IP jail, bot and attack blocking** (`deploy/afyamkononi-website/frontdoor.bicep`): Azure Front Door Premium with a Web Application Firewall in front of the Static Web App. This is the Azure equivalent of fail2ban, which needs a server this site does not have.
- Per-IP rate limits (default 1,500 requests a minute and 4,000 per 5 minutes; one page view is about 30 requests). An IP over the limit is blocked for the rest of that window. Limits are generous because Kenyan mobile networks share one IP among many people.
- Manual IP jail list (`blockedIpRanges`).
- Scanner trap: WordPress, PHP, `.env`, `.git`, `/admin` and similar probes are blocked.
- Only GET, HEAD and OPTIONS.
- Microsoft Default Rule Set 2.1 (OWASP-style attacks) and Bot Manager 1.1.

Deploy steps:
1. `az deployment group create -g <rg> -f deploy/afyamkononi-website/frontdoor.bicep -p staticWebAppHostname=<name>.azurestaticapps.net` (consider `wafMode=Detection` for the first days, then `Prevention`).
2. Add `afyamkononi.app` and `www.afyamkononi.app` as custom domains on the Front Door endpoint (not on the Static Web App), and add them to the `waf` security policy.
3. **Only Front Door may reach the site:** build with the deployment's `frontDoorId` output, `AFD_ID=<frontDoorId> python apps/afyamkononi-website/tools/build.py`. The Static Web App then rejects any request that did not come through this Front Door, so the firewall cannot be bypassed via `*.azurestaticapps.net`. This needs the Static Web Apps **Standard** plan.

**Sessions and tokens:** the website has no sign-in, sets no cookies and stores no tokens, so there is no session to steal. Azure's built-in sign-in (`/.auth/*`) is switched off. If an account area is ever added, the CSP (no injected script), HSTS, `frame-ancestors 'none'` and `Cross-Origin-Opener-Policy` are already in place. The app accounts and their sessions live in the HelloDoctor backend, not here.

**Contact for researchers:** `/.well-known/security.txt` (renew its `Expires` date yearly).

**Azure portal settings to check** (not configurable from files): turn off or restrict *staging environments* for pull requests (they are public by default), keep the free `*.azurestaticapps.net` hostname redirected or unadvertised, and add the custom domain with Azure's managed certificate.

**Local check:** `python apps/afyamkononi-website/tools/serve.py 4174 --dist` serves the built folder with the same headers and blocked paths (HSTS left out on local http).
