# Phase 11 — Real-Time Communications & Teleconsultation Architecture

> **Document Version:** 1.1.0  
> **Protocols:** Agora RTC SDK (Managed Video/Audio SD-RTN), WebSocket (24h Clinical Chat), FCM/APNS (Push Notifications)  
> **Target Audience:** Claude Code Autonomous Implementation Agent

---

## 1. Real-Time Requirement Analysis & Protocol Selection

To maintain optimal battery life, rapid time-to-market, and network resilience on 2G/3G/4G connections, protocols are selected based on actual clinical interaction requirements:

| Clinical Feature | Real-Time Requirement | Selected Protocol | Architectural Justification |
|---|---|---|---|
| **Live Telehealth Consultation** | Sub-300ms bidirectional video & audio | **Agora RTC SDK** (`agora_rtc_engine`) | Managed SD-RTN automatically traverses carrier NATs and handles packet loss up to 80% without custom TURN/SFU infrastructure. |
| **24-Hour Follow-up Chat** | Instant message delivery, typing indicators | **WebSocket** (with REST history fallback) | Low-overhead bidirectional messaging during active clinical consultation sessions. |
| **Waiting Room Queue Countdown** | Periodic position & status updates | **Server-Sent Events (SSE)** or **Polling** (10s) | Unidirectional server-to-client updates; full duplex WebSockets are unnecessary. |
| **Upcoming Consultation Alerts** | Time-sensitive reminders (30m before) | **Push Notifications (FCM / APNS)** | Must reach user even when application is terminated or phone is locked. |
| **Doctor On-Duty Presence** | Online/offline availability status | **REST Polling** or **SSE** | Status changes infrequently; persistent socket holding drains doctor battery. |
| **Escrow & Ledger Updates** | Financial status transitions | **REST API + Webhook** | High-integrity ACID operations; real-time push not strictly required. |

---

## 2. Agora Video Telehealth Architecture

Live video and audio teleconsultation is powered by **Agora RTC SDK** instead of a custom-built WebRTC server. Media transport, jitter buffering, and mobile telco firewall traversal are handled by Agora's global Software-Defined Real-time Network (SD-RTN).

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       AGORA TELEHEALTH SESSION LIFECYCLE                    │
├──────────────────────────────────────┬──────────────────────────────────────┤
│ 1. CHANNEL SECURITY & TOKEN MINTING  │ 2. MEDIA STREAMING (Agora SD-RTN)    │
│    • Rust backend mints Dynamic Token│    • 720p/1080p Adaptive Video       │
│    • Bound to appointment_id + UID   │    • Hardware Echo Cancellation      │
│    • Short TTL (1 hour expiration)   │    • 80% Packet Loss Tolerance       │
├──────────────────────────────────────┼──────────────────────────────────────┤
│ 3. FLUTTER INTEGRATION               │ 4. WEB WORKSTATION INTEGRATION       │
│    • agora_rtc_engine Plugin         │    • Agora Web SDK (v4.x)            │
│    • AgoraVideoView (Local & Remote) │    • Dual-Pane Telehealth Console    │
│    • Camera / Microphone Pre-Flight  │    • Screen Sharing & Diagnostics    │
├──────────────────────────────────────┴──────────────────────────────────────┤
│ 5. TELEMETRY CAPTURE & DISPUTE EVIDENCE                                     │
│    • Agora RtcEngineEventHandler.onRtcStats Listener                         │
│    • Ingestion of duration, bitrate, packet loss, and RTT                    │
│    • Premature Call Termination Flagging (<30s) -> consultation_telemetry   │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.1 Backend Dynamic Token Minting (Rust Axum)
- **Endpoint:** `POST /api/v1/telehealth/agora-token`
- **Authentication:** Bearer JWT (Patient or Doctor).
- **Request Body:**
  ```json
  { "appointment_id": "apt-94812" }
  ```
- **Business Logic:**
  1. Verifies that the authenticated caller is either the assigned patient or doctor for `appointment_id`.
  2. Verifies that the appointment is in `CONFIRMED` or `IN_CONSULTATION` status.
  3. Mints an Agora RTC Token using:
     - `AGORA_APP_ID` (from environment)
     - `AGORA_APP_CERTIFICATE` (from environment)
     - Channel Name: `apt-94812` (or `room_id`)
     - UID: Numeric user ID hash or string user ID
     - Role: `RtcRole::Publisher`
     - Privilege Expiration: 3600 seconds (1 hour)
- **Response:** `200 OK`
  ```json
  {
    "success": true,
    "data": {
      "channel_name": "apt-94812",
      "token": "007eJxTYGCoM2y+5tW2Z1n380vP4...",
      "uid": 94812,
      "app_id": "a1b2c3d4e5f6..."
    }
  }
  ```

### 2.2 Flutter Mobile Implementation (`agora_rtc_engine`)
```dart
import 'package:agora_rtc_engine/agora_rtc_engine.dart';

class TelehealthCallController {
  late RtcEngine _engine;
  int _callSeconds = 0;
  double _packetLoss = 0.0;
  int _rtt = 0;

  Future<void> initializeAgora({
    required String appId,
    required String channelName,
    required String token,
    required int uid,
  }) async {
    _engine = createAgoraRtcEngine();
    await _engine.initialize(RtcEngineContext(
      appId: appId,
      channelProfile: ChannelProfileType.channelProfileCommunication,
    ));

    _engine.registerEventHandler(RtcEngineEventHandler(
      onJoinChannelSuccess: (RtcConnection connection, int elapsed) {
        // Start duration timer
      },
      onRtcStats: (RtcConnection connection, RtcStats stats) {
        _callSeconds = stats.duration ?? 0;
        _packetLoss = (stats.rxPacketLossRate ?? 0).toDouble();
        _rtt = stats.lastmileDelay ?? 0;
      },
      onUserOffline: (RtcConnection connection, int remoteUid, UserOfflineReasonType reason) {
        // Remote doctor or patient disconnected
      },
      onLeaveChannel: (RtcConnection connection, RtcStats stats) {
        _submitSessionTelemetry();
      },
    ));

    await _engine.enableVideo();
    await _engine.startPreview();
    await _engine.joinChannel(
      token: token,
      channelId: channelName,
      uid: uid,
      options: const ChannelMediaOptions(
        clientRoleType: ClientRoleType.clientRoleBroadcaster,
        autoSubscribeAudio: true,
        autoSubscribeVideo: true,
      ),
    );
  }

  Future<void> _submitSessionTelemetry() async {
    final premature = _callSeconds < 30;
    // Dispatches telemetry to POST /api/v1/consultations/{id}/telemetry
  }
}
```

### 2.3 Forensic Telemetry Ingestion for Medical Grievance Board
Upon call termination, the client submits the final telemetry record derived from Agora's `RtcStats`:
```json
{
  "call_duration_seconds": 642,
  "ice_connection_state": "AGORA_SD_RTN_CONNECTED",
  "packet_loss_percent": 0.42,
  "round_trip_time_ms": 38,
  "premature_end": false,
  "prescription_issued": true
}
```
If `call_duration_seconds < 30`, the backend automatically marks `premature_end: true`, retaining the escrow hold pending grievance review or patient reconnection.

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
