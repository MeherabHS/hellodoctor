# ADR-008: WebRTC Telehealth Video Infrastructure & Network Traversal

> **Status:** APPROVED  
> **Date:** 2026-10-01  
> **Deciders:** Principal Systems Architect, Network Systems Engineer

---

## 1. Context
Patient consultations frequently occur over mobile cellular networks (2G/3G/4G) in Bangladesh and Kenya, characterized by carrier-grade symmetric NATs, packet jitter, and firewall restrictions. Standard peer-to-peer WebRTC connections often fail without robust network traversal relays.

## 2. Decision
We implement a **Dual-Mode Media Architecture**:
1. **Direct P2P via STUN/TURN:** For primary 1-on-1 consultations using an open-source `coturn` cluster deployed in regional data centers (Dhaka & Nairobi).
2. **Selective Forwarding Unit (SFU) Fallback:** Preparedness for **LiveKit** integration if multi-party consultations (e.g., patient, family member, and specialist) or server-side clinical recording are required.

## 3. Alternatives Considered
1. **Pure Peer-to-Peer without TURN:** Fails in over 35% of mobile cellular connections in target markets due to symmetric NAT and mobile firewall blocking.
2. **Proprietary Video SDKs (Twilio Video / Agora):** Expensive per-minute pricing that undermines unit economics for affordable teleconsultation services.

## 4. Rationale
- **High Connection Success Rate:** Dedicated TURN relays guarantee near-100% connection success even behind restrictive mobile telco firewalls.
- **Adaptive Bitrate Control:** Enforces automated degradation to audio-only mode when cellular throughput falls below 128 kbps, preserving clinical dialogue over frozen video.
- **Client Telemetry Sampling:** Native `getStats()` hooks capture jitter and packet loss, storing objective metrics for grievance arbitration.

## 5. Consequences
- **Positive:** Maximum connection reliability, cost-effective bandwidth pricing, full control over media streams.
- **Trade-off:** Operating regional TURN servers requires monitoring relay bandwidth capacity.
