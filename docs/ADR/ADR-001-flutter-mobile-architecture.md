# ADR-001: Flutter for Cross-Platform Mobile Client Architecture

> **Status:** APPROVED  
> **Date:** 2026-10-01  
> **Deciders:** Principal Systems Architect, Mobile Lead

---

## 1. Context
HelloDoctor requires an iOS and Android mobile healthcare client that provides high-fidelity UI rendering, consistent 60fps animations, native Agora RTC SDK integration, and offline capabilities. The reference prototype is built with HTML/CSS/JavaScript, but a web wrapper (Cordova, Capacitor) would suffer from performance bottlenecks, device fragmentation, and poor offline resilience on low-end Android smartphones prevalent in Bangladesh and Kenya.

## 2. Decision
We choose **Flutter (Dart)** as the primary cross-platform mobile technology for both the Patient Application and Doctor Mobile Application.

## 3. Alternatives Considered
1. **React Native:** Good ecosystem, but relies on a JavaScript bridge that introduces garbage collection pauses and frame drops during live video and canvas rendering.
2. **Native iOS (Swift) & Native Android (Kotlin):** Highest possible performance, but requires maintaining two distinct codebases, doubling development effort and risking feature desynchronization.
3. **PWA / Web Wrapper:** Unacceptable hardware device integration, lack of reliable background push notifications, and higher memory footprints on low-spec devices.

## 4. Rationale
- **Direct Skia/Impeller Rendering:** Flutter bypasses native OEM widgets and renders directly to the canvas, guaranteeing 100% pixel-fidelity across all Android and iOS device models.
- **Single Unified Codebase:** Patient and Doctor mobile viewports can share domain models, API clients, and design token libraries.
- **Robust Hardware Ecosystem:** `agora_rtc_engine` provides battle-tested bindings for native video/audio.

## 5. Consequences
- **Positive:** Single codebase, deterministic pixel fidelity, fast development cycle, strong typing with Dart. `agora_rtc_engine` integration allows for resilient telehealth sessions on unreliable networks.
- **Trade-off:** Initial app binary size is slightly larger (~25-30 MB) compared to a pure native app; mitigated via asset compression and ProGuard/R8 obfuscation.
