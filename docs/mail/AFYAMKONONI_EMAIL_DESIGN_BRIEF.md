# AfyaMkononi — Email Design System
**Version:** 1.0  
**Status:** Design specification for implementation  
**Platform:** AfyaMkononi / HelloDoctor.Asia

## 1. Design Objective

Create a distinctive, professional, youth-centered email communication system for AfyaMkononi, a digital healthcare platform serving young people in Kenya, including young people living with HIV and communities experiencing barriers to healthcare.

The email experience must combine:

- Human-centered, contemporary African illustrations.
- Modern digital-health visual language.
- Confident, welcoming and empowering communication.
- Strong commitments to privacy, dignity and non-discrimination.
- Professional clinical credibility.

The visual identity should feel like a modern youth-focused health and wellness platform rather than a conventional hospital, government institution or generic automated email service.

## 2. Brand Identity

**Primary brand:** AfyaMkononi

**Tagline:** Your Health. Your Space. Your Way.

**Operating organization:** Global Health Initiatives (GHI)

**Technology partner:** Research and Management Consultants Ltd. (RMCL), through HelloDoctor.Asia.

Routine email footers should identify GHI and RMCL only. Do not include Circle Technology Ltd. in these templates.

Use the approved AfyaMkononi logo and brand assets where available. Do not invent an official organizational logo.

## 3. Audience

**Primary audience:** Young people and young adults in Kenya, particularly individuals seeking accessible, confidential, youth-friendly healthcare.

**Secondary audience:** Healthcare professionals participating in the AfyaMkononi network.

The communication style should be:

- Warm but not overly emotional.
- Youthful without excessive slang.
- Respectful of different identities and circumstances.
- Accessible and easy to understand.
- Positive, stigma-free and non-judgmental.
- Clinically responsible and trustworthy.

Communications for doctors should retain greater professional formality than patient onboarding emails.

## 4. Visual Identity

### Color Palette

Proposed design palette, subject to existing official brand guidelines:

| Color | Hex | Application |
|---|---|---|
| Deep Teal | #164E59 | Headers, prominent headings |
| Vibrant Teal | #087F8C | Primary actions |
| Mint | #DDF5EE | Soft backgrounds |
| Warm Coral | #F3A58E | Illustration accents |
| Soft Lavender | #EAE7FA | Decorative elements |
| Warm Ivory | #FAFAF6 | Main background |
| Charcoal | #24343B | Body text |

Use color sparingly and maintain accessible contrast.

### Illustration Direction

Combine contemporary editorial illustration with modern geometric digital-health elements.

Illustrations should feature:

- Diverse Kenyan and East African young adults.
- Friendly healthcare professionals.
- Natural, confident expressions.
- Inclusive representation without stereotypes.
- Healthcare interactions, digital communication, friendship, trust and personal agency.
- Rounded shapes, subtle gradients and contemporary editorial compositions.

Avoid:

- Graphic depictions of illness or suffering.
- HIV ribbons or visible HIV-status references in patient-facing artwork.
- Stereotypical portrayals of vulnerable populations.
- Generic stock photography and excessive medical symbolism.
- Infantile cartoon characters.
- Overly decorative graphics that reduce readability.

Illustrations must not imply that a particular depicted individual has HIV.

### Layout

Use a responsive HTML email structure.

- Maximum desktop content width: approximately 600 px.
- Mobile-responsive layout.
- Clean header with AfyaMkononi branding.
- Optional illustration-led hero section.
- One strong headline.
- Short paragraphs with generous spacing.
- One prominent CTA when appropriate.
- Consistent footer.
- Readable plain-text alternative.

Use inline CSS and email-client-compatible HTML.

All important information must remain readable when images are blocked.

## 5. Master Email Structure

**Header**

AfyaMkononi logo and tagline.

**Hero**

Original illustration appropriate to the message category.

**Headline**

Short, memorable, positive and relevant.

**Personal greeting**

Hello {{first_name}}, or Dear Dr. {{doctor_name}}.

**Main message**

Two or three concise paragraphs communicating the purpose.

**Action**

A clearly labeled button when an approved action URL exists.

**Closing**

A brief human-sounding sign-off.

**Footer**

AfyaMkononi  
Your Health. Your Space. Your Way.

Operated by Global Health Initiatives (GHI)  
Technology Partner: Research and Management Consultants Ltd. (RMCL)

Support: support@hellodoctor.asia

## 6. Template-Specific Visual Treatment

### Patient Welcome

Use a warm illustration of confident young adults accessing digital healthcare, with modern rounded shapes and a bright but restrained palette.

Mood: welcoming, hopeful, independent.

### Doctor Application Acknowledgment

Use a healthcare-professional illustration combined with a small application-received status graphic.

Mood: purposeful, respectful, encouraging.

### Doctor Approval

Use a subtle celebratory illustration featuring a healthcare professional and contemporary graphic accents.

Mood: accomplishment, responsibility, inclusion.

### Doctor Rejection

Use a restrained layout, minimal decoration and a neutral illustration or abstract graphic.

Mood: respectful, calm, professional.

### Appointment Confirmation

Use a compact confirmation layout with a date/time card.

Avoid clinical details or diagnosis references.

### Payments and Security

Use minimal illustrations, clear transactional details and strong readability.

OTP codes must be readable without images.

## 7. Privacy and Safety Requirements

Never expose an individual's HIV status, diagnosis, medication, consultation reason or sensitive identity information in ordinary email subjects, preview text or decorative images.

Do not include clinical notes or consultation transcripts.

Use privacy-preserving appointment references rather than detailed medical descriptions.

Doctor-facing organizational communications may refer to the HIV-response mission, stigma reduction and inclusive care without identifying individual patients.

Avoid external tracking pixels.

Never include OTP values, account tokens or clinical information in externally hosted image URLs.

## 8. Implementation Requirements

Build reusable HTML and plain-text email templates within the existing Rust email architecture.

Use the existing EmailTransport and durable email outbox.

Templates must support:

- Safe HTML escaping.
- Missing-name fallbacks.
- Mobile rendering.
- Image-blocked rendering.
- Tested links.
- Consistent footer.
- Future English and Kiswahili localization.
- Accessibility and email-client compatibility.

Store illustration assets in an approved, controlled hosting location.

Do not introduce external newsletter platforms or marketing-tracking services.

## 9. Scope Control

This specification covers email presentation and content, not authorization to activate sending.

Consultation reminders remain exclusively in existing FCM/in-app notification workflows.

Password recovery remains disabled until its implementation and security testing are separately approved.

## 10. Acceptance Criteria

The design is ready for integration when:

1. All four core onboarding templates use the same branded structure.
2. Patient and doctor communications are appropriate to their audiences.
3. HTML and plain-text variants convey equivalent essential information.
4. Layouts render correctly on mobile and common email clients.
5. Illustrations are visually consistent and optimized.
6. Important information remains accessible with images disabled.
7. GHI and RMCL appear consistently in the footer.
8. No patient-sensitive information is disclosed.
9. No existing authentication or notification behavior is unintentionally changed.
