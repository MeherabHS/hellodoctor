# Sauti V4 creative director pass

Sauti V4 keeps the Sauti V3 grid, typography, section order, palette, and content model. The finishing pass focuses on art direction and launch honesty.

## Specific improvements

- The hero now uses a cleaner campaign-style image slot with a restrained approval-pending label instead of visible CSS placeholder copy. The Patient App capture sits in a larger, more upright device frame with a clearer internal-build caption.
- The Patient section is now warm sand with an ochre edge signal. Yellow supports the composition rather than becoming the entire visual field.
- The Doctor section uses an outlined clinician signal composition with `Review context / Respond clearly / Continue care`, which communicates a real workflow without fabricating a dashboard.
- The app showcase uses larger, more legible screens with consistent frames and calmer perspective angles.
- How It Works now has a continuous editorial rule through the numbered journey, while Trust and About transitions use shorter, more intentional spacing.
- Body copy and app captions retain the V3 readability improvements, with slightly larger product notes and stronger contrast on green and sand surfaces.
- The marquee remains a 42-second loop with a static screen-reader sentence, hover/focus pause, reduced-motion fallback, and no horizontal page overflow.

## Preserved comparison

The V3 homepage, stylesheet, and script are preserved as `concept-sauti-v3.html`, `sauti-v3.css`, and `sauti-v3.js`.

## Remaining launch blockers

1. Replace the labelled hero image slot with approved licensed Kenyan healthcare photography.
2. Replace the current Patient App review captures with clean production exports that contain no test accounts or synthetic clinician names.
3. Confirm a production clinician screenshot or photography if a real clinician workflow visual is approved.
4. Confirm the support email and published Privacy, Terms, and Consent URLs.
5. Approve the GHI, Circle Technology Ltd, AfyaMkononi.app, and RMCL attribution wording.

## Accessibility and performance

The homepage keeps semantic landmarks, descriptive app-screen alt text, explicit keyboard focus styling, reduced-motion handling, a skip link, and a minimal JavaScript bundle. The hero slot is CSS-only until the licensed photograph is available; production photography and clean app exports should be converted to responsive WebP/AVIF assets with intrinsic dimensions before launch.

## Production-prep structure

The static workspace now includes Patients, Doctors, About, Support, Privacy, Terms, and Consent pages. Their public support/legal destinations remain visibly pending until approved. See `PRODUCTION-CHECKLIST.md` for the release gate.
