# Phase 11 — Real-Time Communications & Teleconsultation Architecture

> **Document Version:** 1.0.0  
> **Protocols:** WebRTC (P2P / SFU Media Transport), WebSocket (Signaling & 24h Chat), FCM/APNS (Push Notifications)  
> **Target Audience:** Claude Code Autonomous Implementation Agent

---

## 1. Real-Time Requirement Analysis & Protocol Selection

To maintain optimal battery life and network efficiency on 2G/3G/4G connections, protocols are selected based on actual clinical interaction requirements:

| Clinical Feature | Real-Time Requirement | Selected Protocol | Architectural Justification |
|---|---|---|---|
| **Live Telehealth Consultation** | Sub-300ms bidirectional video & audio | **WebRTC** (Media) + **WebSocket** (Signaling) | Essential for real-time diagnostic examination; REST is incapable of streaming media. |
| **24-Hour Follow-up Chat** | Instant message delivery, typing indicators | **WebSocket** (with REST history fallback) | Low-overhead bidirectional messaging during active clinical consultation sessions. |
| **Waiting Room Queue Countdown** | Periodic position & status updates | **Server-Sent Events (SSE)** or **Polling** (10s) | Unidirectional server-to-client updates; full duplex WebSockets are unnecessary. |
| **Upcoming Consultation Alerts** | Time-sensitive reminders (30m before) | **Push Notifications (FCM / APNS)** | Must reach user even when application is terminated or phone is locked. |
| **Doctor On-Duty Presence** | Online/offline availability status | **REST Polling** or **SSE** | Status changes infrequently; persistent socket holding drains doctor battery. |
| **Escrow & Ledger Updates** | Financial status transitions | **REST API + Webhook** | High-integrity ACID operations; real-time push not strictly required. |

---

## 2. WebRTC Video Teleconsultation Architecture

WebRTC is structured into five distinct subsystems to avoid conflating signaling with media transport:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                             TELEHEALTH SESSION                              │
├──────────────────────────────────────┬──────────────────────────────────────┤
│ 1. SIGNALING (Rust WebSocket Server)  │ 2. MEDIA TRANSPORT (WebRTC / SFU)    │
│    • SDP Offer / Answer Exchange      │    • Direct P2P via STUN/TURN (P2P)  │
│    • ICE Candidate Routing            │    • Adaptive Bitrate (128kbps-2Mbps)│
│    • Connection Heartbeats            │    • VP8 / H.264 Video, Opus Audio   │
├──────────────────────────────────────┼──────────────────────────────────────┤
│ 3. SESSION MANAGEMENT (Rust Backend) │ 4. DEVICE HARDWARE (Flutter Client)  │
│    • Room Creation & Authorization    │    • Camera / Microphone Access      │
│    • Call Duration Clock              │    • Pre-Flight Permission Checks    │
│    • Escrow State Locking             │    • Hardware Echo Cancellation      │
├──────────────────────────────────────┴──────────────────────────────────────┤
│ 5. TELEMETRY CAPTURE & CONNECTION RECOVERY                                  │
│    • Packet Loss & RTT Sampling                                             │
│    • Premature Call Termination Flagging (<30s)                             │
│    • Silent Evidence Binding for Medical Grievance Board                     │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.1 Signaling Protocol (Rust WebSocket Endpoint)
- **Endpoint:** `GET /api/v1/telehealth/signal/{room_id}` (Upgraded to WebSocket).
- **Authentication:** Query param `?token=<JWT_ACCESS_TOKEN>`.
- **Signaling Message Types:**
  - `join_room`: Client enters signaling channel.
  - `sdp_offer`: Peer sends Session Description Protocol offer.
  - `sdp_answer`: Peer responds with SDP answer.
  - `ice_candidate`: Peer exchanges network routing candidates.
  - `leave_room`: Peer intentionally disconnects.

### 2.2 External Media Infrastructure Recommendation
- **STUN/TURN Servers:** Production requires a dedicated TURN server (e.g., open-source `coturn` deployed in-region or managed Twilio Network Traversal) to traverse symmetric NATs and mobile telco firewalls.
- **Selective Forwarding Unit (SFU):** For 1-on-1 calls, direct P2P with TURN relay is recommended. If multi-party consultations (e.g., patient, family member, and specialist) are introduced, **LiveKit** (open-source Go/Rust SFU) is recommended.

### 2.3 Forensic Telemetry Recording
During the call, the Flutter client samples `PeerConnection.getStats()` every 5 seconds. Upon call termination, the client submits the final telemetry record:
```json
{
  "call_duration_seconds": 642,
  "ice_connection_state": "COMPLETED",
  "packet_loss_percent": 0.42,
  "round_trip_time_ms": 38,
  "premature_end": false,
  "prescription_issued": true
}
```
If `call_duration_seconds < 30`, the backend automatically flags `premature_end: true`, retaining the escrow hold pending grievance review or patient reconnection.

---

## 3. 24-Hour Clinical Chat Specification

### 3.1 Session Lifecycle & Auto-Locking
- The chat conversation (`chat_conversations`) is instantiated upon appointment confirmation.
- The chat window remains active for **exactly 24 hours** after the video consultation completes (`expires_at = consultation_ended_at + 24 hours`).
- Once expired, the backend marks `is_locked = TRUE`. Further message submissions return `403 Forbidden` (`CHAT_SESSION_EXPIRED`).

### 3.2 WebSocket Chat Schema
- **Endpoint:** `GET /api/v1/chat/ws/{conversation_id}`
- **Client -> Server (Message Send):**
  ```json
  {
    "type": "send_message",
    "content": "Should I continue taking Azithromycin if fever subsides?",
    "attachment_url": null
  }
  ```
- **Server -> Client (Message Ingested & Broadcast):**
  ```json
  {
    "type": "message_received",
    "message_id": "msg-84192",
    "sender_id": "u-9481",
    "sender_role": "PATIENT",
    "content": "Should I continue taking Azithromycin if fever subsides?",
    "sent_at": "2026-10-01T10:45:00Z"
  }
  ```
- **Typing Indicator:**
  ```json
  { "type": "typing", "sender_id": "u-doctor-sabrina", "is_typing": true }
  ```
