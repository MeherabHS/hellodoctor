# Doctor onboarding (4 screens)

Reference: "AfyaMkononi Doctor Onboarding Screens" board, supplied by the
product owner on 2026-11-20 (four phone screens in a 2 x 2 grid).

## When it shows

The onboarding shows once per approved doctor account on a phone, inside
`DoctorShell`, after the verification gate. That means the doctor's first
login, or their first launch after the account is approved (product-owner
decision, 2026-11-20). It never shows to Peer Navigators or to doctors
awaiting verification.

- The flag is `doctor_onboarding_seen_v1:<user id>`, or the licence number
  when the profile has no user id. A second doctor on the same phone still
  sees it.
- `SecureTokenStore.clear` keeps every key with that prefix, so signing out
  does not show it again.
- Until the profile and flag are known, the dashboard shows, so a returning
  doctor never sees a blank screen. A storage error counts as "seen".
- Skip, or the last button, finishes it.

## Board text changed for signed-in doctors

The board's last screen invites a doctor to "Create your account now…" with
a **Create Doctor Account** button. Because the onboarding shows after login,
that screen reads instead:

- "Start providing trusted, confidential and adolescent-friendly care on
  AfyaMkononi."
- A **Go to Dashboard** button, which finishes onboarding.

Every other word is the board's own. Swahili strings are in the Care app ARB
files (`doctorOnboarding*`, `brandTagline`).

## How it is built

- **Artwork:** the illustrations and the bottom wave are the board's own
  pixels, cropped inside each phone frame and stored losslessly in
  `apps/mobile/assets/onboarding/`. Each screen keeps its own size (about
  405 x 941), so nothing was resampled.
- **Logo:** the board's logo is reference only (product-owner instruction,
  2026-11-20). The app's own AfyaMkononi logo (`assets/brand/afya_logo.webp`,
  the same file in every app) is drawn in the board's logo box.
- **Native parts:** the wordmark, tagline, Skip, titles, body text, page dots
  and buttons are Flutter widgets. They are placed at positions, sizes and
  colours measured from the board.
- **Fit:**
  - The whole slide always shows, centred on a white page.
  - A full-width copy of the slide's bottom wave spans phones wider than the
    board.
  - With a tall navigation bar, the slide is lifted so the buttons stay
    clear of it.
- **Accuracy:** compared at board size, every text line, Skip and the header
  land within 1–3 px of the board.

## Known differences

- **Typeface:** the board's lettering is not a bundled font. Plus Jakarta
  Sans is used, sized and weighted to the board's line widths.
- **Resolution:** the artwork is as sharp as the board (about 405 px wide).
  On a 1080 px phone it is enlarged about 2.4x, so it is softer than the
  native text. It was not upscaled.
- **Slide 1 dot:** the board draws slide 1's current dot in pale green
  (#83CCB1), unlike slides 2–4 (#027C4B). This is kept as drawn.
