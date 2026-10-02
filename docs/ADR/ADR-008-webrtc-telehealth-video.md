# ADR-008: Agora RTC SDK for Telehealth Video Infrastructure

> **Status:** APPROVED (Updated per Product Requirement)  
> **Date:** 2026-10-01  
> **Deciders:** Principal Systems Architect, Mobile Lead, Product Owner

---

## 1. Context
Patient consultations frequently occur over mobile cellular networks (2G/3G/4G) in Bangladesh and Kenya, characterized by carrier-grade symmetric NATs, high packet jitter, and fluctuating bandwidth. Building, maintaining, and scaling custom WebRTC signaling servers, STUN/TURN relays, and custom media SFUs introduces heavy DevOps overhead, infrastructure maintenance costs, and risks of dropped connections under challenging telco conditions.

## 2. Decision
We choose **Agora RTC SDK** as the managed real-time video and audio infrastructure for HelloDoctor:
1. **Client Mobile Application:** Implemented via **`agora_rtc_engine`** (Flutter plugin for iOS & Android) and Agora Web SDK for desktop clinical workstations.
2. **Backend Authentication & Channel Security:** The **Rust Axum backend** generates short-lived, cryptographically signed Agora Dynamic RTC Tokens (`RTC_TOKEN_BUILDER`) using our secure Agora App ID and App Certificate.
3. **Telemetry & QoS Capture:** Agora's client event callbacks (`onRtcStats`, `onNetworkQuality`, `onUserOffline`) provide direct telemetry metrics that are ingested into HelloDoctor's `consultation_telemetry` database for premature drop detection (<30s) and clinical grievance arbitration.

## 3. Alternatives Considered
1. **Custom WebRTC P2P with Self-Hosted Coturn:** Requires deploying, clustering, and monitoring regional TURN servers across Dhaka and Nairobi; prone to connection drops under mobile telco firewalls.
2. **Custom SFU Cluster (LiveKit / Mediasoup):** Requires dedicated media server compute instances, auto-scaling configuration, and constant infrastructure maintenance.
3. **Twilio Video / Vonage:** Deprecated or less optimized for high-jitter, high-packet-loss emerging market mobile telco networks.

## 4. Rationale
- **Software-Defined Real-Time Network (SD-RTN):** Agora's global routing infrastructure automatically traverses mobile carrier firewalls and maintains audio/video stability even under up to 80% packet loss.
- **Dynamic RTC Token Security:** Patients and doctors cannot access a consultation room without a server-minted token tied to their specific `appointment_id`, user ID (`uid`), and active consultation time window.
- **Hardware-Accelerated Codecs:** Native integration with device hardware encoders/decoders (H.264, VP8) maximizes battery efficiency on budget smartphones.
- **Direct QoS Telemetry:** Agora's `RtcStats` (duration, tx/rx audio/video bitrate, packet loss rate, round-trip time) directly populates HelloDoctor's clinical governance and grievance audit trail.

## 5. Consequences
- **Positive:** No custom TURN-server maintenance, vendor-managed network traversal and built-in echo cancellation, with reliability measured through Agora telemetry and tested across supported mobile networks.
- **Trade-off:** External vendor dependency; requires provisioning Agora App ID and App Certificate in environment configuration (`AGORA_APP_ID`, `AGORA_APP_CERTIFICATE`).
