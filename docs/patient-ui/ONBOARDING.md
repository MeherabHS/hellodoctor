# Patient onboarding (4 screens)

Reference: "AfyaMkononi Youth Health Onboarding" board, supplied by the product
owner on 2026-11-20 (four phone screens, 1774 x 887 px in all). It replaces the
earlier six-slide onboarding.

## How it is built

- **Artwork is the board's own pixels.** Each screen is cropped inside the phone
  frame (430 x 882 px) and stored as lossless WebP in
  `apps/mobile_patient/assets/onboarding/onboarding_1..4.webp`. Everything in
  the artwork keeps its exact colours: the characters, the Kenyan/African
  pattern bands, plants and icon circles. The waving hand on screen 1
  is a separate transparent sprite (`onboarding_wave.webp`) so it can follow
  the title in either language.
- **Logo:** the board's logo is reference only (product-owner instruction,
  2026-11-20). It is erased from the artwork and the app's own AfyaMkononi
  logo (`assets/brand/afya_logo.webp`, the same file in every app) is drawn
  in the board's logo box.
- **Native parts:** the title, body, Skip, page dots and button are Flutter
  widgets, as is the wordmark and tagline on screen 1. They sit at the
  positions, sizes and colours measured from the board. Their baked-in
  originals were blanked from the artwork. The button is drawn over the
  board's button, at the same rectangle.
- **Layout:** the code works in the board's 430 x 882 coordinates and scales
  them to the screen. If the screen is taller than the board, the artwork
  grows until at most 3% is cropped from each side; any height left over goes
  above it, in the artwork's top colour. A tablet shows the whole artwork,
  centred. Skip keeps the board's margin from the screen's right edge. Text
  never runs past the visible artwork.
- **Copy:** English and Swahili strings live in the patient ARB files
  (`onboarding1Title` … `onboarding4Body`, `onboardingNext`,
  `onboardingGetStarted`, `onboardingSkip`). The English line breaks follow
  the board. A longer translation shrinks to fit its box instead of
  overflowing.

## Verified against the board

The board-shaped render (430 x 882 at 2x) was compared with the board pixel by
pixel. Every text line lands within 1–4 px of the board. The dots and Skip
pill match, and the button label sits within 2 px.

## Known differences

- **Typeface:** the board's lettering is not a font this app bundles. Native
  text uses Plus Jakarta Sans at sizes and weights matched to the board's line
  widths. Screen 1's title on the board is a rounded display face; the app
  uses Plus Jakarta Sans ExtraBold.
- **Resolution:** the artwork is only as sharp as the board (430 px wide per
  screen). On a 1080 px phone it is shown about 2.5x enlarged, so it is softer
  than the native text. It was not upscaled or redrawn. A sharper copy would
  need a higher-resolution source from the designer.

## Behaviour

Onboarding shows once per install, before sign-in. Skip, or the last button
("Get Started" / "Anza"), finishes it. The device flag `onboarding_seen_v1` is
kept when sign-out clears the session (`SecureTokenStore.clear`), so signing
out does not show onboarding again.
