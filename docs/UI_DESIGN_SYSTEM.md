# Phase 12 — UI Design System & Flutter Theme Specification

> **Document Version:** 1.0.0  
> **Source Baseline:** `css/tokens.css`, `css/components.css`, `index.html`  
> **Design Philosophy:** "Warm Dignity" — Clinical authority paired with human warmth  
> **Target Audience:** Claude Code Autonomous Implementation Agent

---

## 1. Color Palette & Token Definitions

All values are `OBSERVED` directly from `css/tokens.css` and verified in `index.html`.

### 1.1 Brand & Clinical Colors
| Token Name | Hex Code | Purpose & Usage | Flutter Color |
|---|---|---|---|
| `--color-primary` | `#0D5C52` | Deep Forest Teal: Primary buttons, active tabs, clinical authority | `Color(0xFF0D5C52)` |
| `--color-primary-dark` | `#073B34` | Pressed button states, deep header backgrounds | `Color(0xFF073B34)` |
| `--color-primary-light` | `#14B8A6` | Active Mint: Positive feedback, adherence streaks, toggles | `Color(0xFF14B8A6)` |
| `--color-primary-surface`| `#E6F4F1` | Soft teal tint: Selected chip fills, highlight containers | `Color(0xFFE6F4F1)` |
| `--color-accent-terracotta`| `#D95D39` | Warm Terracotta: Human empathy, secondary CTAs, health vault | `Color(0xFFD95D39)` |
| `--color-gold` | `#D97706` | Acacia Gold: Milestone celebrations, appointment alerts | `Color(0xFFD97706)` |
| `--color-gold-surface` | `#FEF3C7` | Soft gold background for reminder cards | `Color(0xFFFEF3C7)` |
| `--color-emergency` | `#DC2626` | Urgent Red: Exclusively for Emergency Contact (`16263`) and clinical red flags | `Color(0xFFDC2626)` |
| `--color-emergency-surface`| `#FEE2E2` | Light red surface for emergency helpline strip | `Color(0xFFFEE2E2)` |
| `--color-success` | `#059669` | Emerald Green: Completed consultations, settled fees, on-duty | `Color(0xFF059669)` |
| `--color-success-surface`| `#D1FAE5` | Soft green background for verified badges | `Color(0xFFD1FAE5)` |

### 1.2 Neutral & Surface Colors
| Token Name | Hex Code | Purpose & Usage | Flutter Color |
|---|---|---|---|
| `--color-bg-app` | `#F8F6F0` | Warm linen / alabaster: Screen background eliminating eye glare | `Color(0xFFF8F6F0)` |
| `--color-surface-card` | `#FFFFFF` | Crisp white cards, modal sheets, elevated surfaces | `Color(0xFFFFFFFF)` |
| `--color-surface-subtle`| `#F1EFEA` | Input fills, pill containers, neutral tags | `Color(0xFFF1EFEA)` |
| `--color-border` | `#E5E1D8` | Hairline dividers, card borders (1px solid) | `Color(0xFFE5E1D8)` |
| `--color-text-main` | `#182220` | Midnight slate ink: Primary typography, maximum contrast | `Color(0xFF182220)` |
| `--color-text-secondary`| `#526360` | Humanized slate: Subtitles, metadata, secondary labels | `Color(0xFF526360)` |
| `--color-text-muted` | `#7E908D` | Hints, placeholders, captions | `Color(0xFF7E908D)` |

---

## 2. Typography Hierarchy

Primary Font Family: **Plus Jakarta Sans** (Fallback: **Inter**, System Sans-serif).

| Style Name | Font Size | Line Height | Font Weight | Usage |
|---|---|---|---|---|
| **Display** | `28px` | `34px` | `800` (Extrabold) | Onboarding hero titles, revenue summary totals |
| **H1** | `22px` | `28px` | `700` (Bold) | Screen headers, top greetings ("Rafiq Ahmed 👋") |
| **H2** | `18px` | `24px` | `700` (Bold) | Section headers ("Upcoming Consultation", "Core Services") |
| **H3** | `16px` | `22px` | `600` (Semibold) | Doctor card names, modal titles, card headers |
| **Body Large** | `16px` | `24px` | `400` (Regular) | Clinical descriptions, blog text |
| **Body** | `14px` | `20px` | `400` (Regular) | General text, inputs, transaction list items |
| **Body Small** | `13px` | `18px` | `500` (Medium) | Hospital affiliation, slot time labels |
| **Caption** | `11px` | `15px` | `600` (Semibold) | BMDC badges, status pills, calculation basis note |

---

## 3. Spacing Grid, Geometry & Radii

### 3.1 8-Point Spacing Scale
- `2px` (`--space-2`): Hairline icon offsets.
- `4px` (`--space-4`): Tight chip padding, badge internal margins.
- `8px` (`--space-8`): Standard inter-element spacing, button vertical padding.
- `12px` (`--space-12`): Compact card padding, input internal padding.
- `16px` (`--space-16`): Standard screen margins, standard card padding.
- `20px` (`--space-20`): Section vertical spacing.
- `24px` (`--space-24`): Major section dividers, hero card padding.
- `32px` (`--space-32`): Bottom sheet top clearance, modal padding.
- `40px` (`--space-40`): Large hero offsets.

### 3.2 Corner Radii
- `radius-sm` (`8px`): Small chips, status badges, secondary buttons.
- `radius-md` (`14px`): Input fields, thumbnail cards, small dialogs.
- `radius-lg` (`20px`): Main service cards, doctor profile cards, hero banners.
- `radius-xl` (`28px`): Bottom modal sheets, top header card wrappers.
- `radius-full` (`9999px`): Circular avatars, rounded filter pills.

### 3.3 Elevation & Shadows
- **Card Shadow:** `0 4px 16px rgba(18, 38, 34, 0.06), 0 1px 3px rgba(18, 38, 34, 0.04)` (`OBSERVED`).
- **Floating Shadow:** `0 10px 28px rgba(13, 92, 82, 0.18), 0 2px 6px rgba(13, 92, 82, 0.08)` (`OBSERVED`).

---

## 4. Reusable Component Specifications

### 4.1 Emergency Contact Helpline Strip (`#patientEmergencyHelplineStrip`)
- **Visual Spec:**
  - Background: `#FEE2E2` (Soft red).
  - Border: `1.5px solid #FCA5A5`.
  - Border Radius: `16px`.
  - Padding: `12px 16px`.
  - Margin: `16px 0` (Positioned directly above Upcoming Consultation).
  - Layout: Horizontal row. Left: Ambulance icon in circular red badge (`#DC2626`). Center: Label "Emergency Contact" (Semibold 14px) and subtext "National Health Emergency Triage". Right: Pill button "Call 16263" (`#DC2626` background, white text).
  - Rule: Must strictly omit any *"24/7"* prefix.

### 4.2 Doctor Card (Directory & Discovery)
- **Visual Spec:**
  - Background: `#FFFFFF`.
  - Border: `1px solid #E5E1D8`.
  - Border Radius: `16px`.
  - Padding: `16px`.
  - Avatar: `60x60px` rounded image (`radius: 12px`).
  - Layout: Row with avatar on left, content in center, booking CTA on right.
  - Badges: BMDC badge (`#E6F4F1` surface, `#0D5C52` text), Experience badge (`#F1EFEA` surface).
  - Rule: No star rating elements, review stars, or thumbs up icons.

### 4.3 Multi-Prescription Upload Dropzone & Thumbnail Strip
- **Visual Spec:**
  - Dropzone Border: `2px dashed #0D5C52` with `#E6F4F1` background when active.
  - Border Radius: `14px`.
  - Thumbnails: `72x72px` square cards with rounded corners (`8px`).
  - Delete Icon: Circular red badge (`20x20px`) at top-right corner.
  - Page Badge: `#182220` surface with white text (`Caption 11px`), positioned at bottom of thumbnail (`"Page 1"`).

### 4.4 Shimmer Loading Card (`.sk-shimmer`)
- **Animation:** `@keyframes skShimmer` with linear gradient sweep from `rgba(240, 238, 233, 0.5)` to `rgba(255, 255, 255, 0.8)` to `rgba(240, 238, 233, 0.5)` across 1.5 seconds infinite loop.
