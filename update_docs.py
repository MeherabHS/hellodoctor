import os
import re

base_dir = r"c:\Users\CUBE\Desktop\helodoc old site"

def update_file(filepath, replacements):
    with open(filepath, 'r', encoding='utf-8') as f:
        content = f.read()
    
    for old, new in replacements:
        content = re.sub(old, new, content, flags=re.MULTILINE)
        
    with open(filepath, 'w', encoding='utf-8') as f:
        f.write(content)

# BUSINESS_LOGIC.md
bl_path = os.path.join(base_dir, "BUSINESS_LOGIC.md")
bl_replacements = [
    # 1. Section 4.5 WebRTC -> Agora
    (r"WebRTC Video & 24h", "Agora RTC Video & 24h"),
    (r"WebRTC Video Teleconsultation", "Agora RTC SDK Video Teleconsultation"),
    (r"720p / 1080p WebRTC stream", "720p / 1080p Agora RTC stream"),
    (r"ICE candidate states, packet loss, and actual call duration", "Agora connection states (onRtcStats, onUserOffline), packet loss, and actual call duration"),
    (r"WebRTC PeerConnection", "Agora RTC SDK"),
    (r"WebRTC Peer Connection", "Agora RTC SDK"),
    (r"WebRTC Peer", "Agora RTC SDK"),
    
    # 2. Section 5.1
    (r"Master Database Flush", "Emergency Capacity Surge Override"),
    
    # 3. Section 5.7
    (r"WEBRTC_SIGNALING", "AGORA_RTC"),
    (r"ICE candidate disconnects, STUN/TURN server latency spikes", "Agora RTC connection drops, latency spikes"),
    (r"and Simulators", "(Development/Staging Only)"),
    (r"- \*\*Failure Simulator Modal \(\`#adminSimulateFailureModal\`\)\*\*:\n  - Allows administrators to simulate webhook dropouts, WebRTC timeouts, and gateway failures for system resilience testing\.", "- **Failure Simulator (Dev/Staging Only)**:\n  - Allows developers to simulate webhook dropouts, Agora timeouts, and gateway failures for system resilience testing."),
    
    # 4. Section 5.8
    (r"BMDC Disciplinary Warning", "Internal Platform Compliance Warning"),
    (r"btnAdjudicateWarning\`\): Logs a formal reprimand in the physician's BMDC dossier", "btnAdjudicateWarning`): Issue Internal Platform Compliance Warning (logs a formal reprimand in the physician's dossier"),
    
    # 5. Section 4.6
    (r"Digital Signature Hash", "Integrity Verification Hash"),
    (r"digital signature validity", "integrity verification hash validity"),
    (r"digitally signed prescriptions", "prescriptions with integrity verification hash (Note: HMAC-SHA256 provides integrity authentication, not a legally binding digital signature)"),
    
    # 6. Section 3.2
    (r"IN_CALL --> CONSULTATION_COMPLETED: Doctor Ends Session & Issues Rx", "IN_CALL --> CONSULTATION_COMPLETED: Doctor Ends Session (Records Clinical Outcome)"),
    
    # 7. Section 4.7
    (r"Issue BMDC Disciplinary Warning", "Issue Internal Platform Compliance Warning"),
    (r"WebRTC Video Freeze", "Agora Video Freeze"),
    (r"WebRTC Video / Audio Freeze", "Agora Video / Audio Freeze"),
    (r"server WebRTC session logs", "server Agora session logs"),
    
    # 8. Section 8
    (r"WebRTC session crashes or fails to establish PeerConnection due to strict NAT/Firewall", "Agora session crashes or fails to connect"),
    (r"WebRTC Video Freeze", "Agora Video Freeze"),
    
    # 9. Section 5.9
    (r"Omnichannel Payment & Escrow Master Ledger", "Omnichannel Payment & Escrow Master Ledger (Note: 'escrow' terminology should be confirmed legally; system uses 'payment hold' model)"),
    
    # 10. Telemetry
    (r"ICE Connected", "Agora Connected"),
    (r"ICE Failed", "Agora Disconnected"),
    (r"ICE Connection Drop", "Agora Connection Drop"),
    
    # 11. Country abstraction
    (r"BMDC \(Bangladesh Medical and Dental Council\)", "BMDC (configurable per deployment country)"),
    (r"DGDA-Compliant", "DGDA-Compliant (configurable per deployment country)"),
    (r"Call 16263", "Call 16263 (configurable per deployment country)"),
    (r"৳", "৳ (configurable per deployment country)"),
    (r"bKash", "bKash (configurable per deployment country)"),
    (r"BMDC Number", "Medical License Number (configurable per deployment country)"),
    (r"BMDC Registration Number", "Medical License Number (configurable per deployment country)"),
    (r"BMDC ID", "Medical License ID (configurable per deployment country)")
]
update_file(bl_path, bl_replacements)

# USER_FLOWS.md
uf_path = os.path.join(base_dir, "docs", "USER_FLOWS.md")
uf_replacements = [
    (r"WebRTC", "Agora SDK"),
    (r"PeerConnection", "Agora SDK"),
    (r"digital signature", "integrity verification hash"),
    (r"BMDC warning", "Internal Platform Compliance Warning"),
    (r"BMDC Disciplinary Warning", "Internal Platform Compliance Warning"),
    (r"ICE", "Agora"),
    (r"CONFIRMED", "PENDING_PAYMENT"),
    (r"/api/v1/consultations/\{appointment_id\}/rtc-token", "POST /api/v1/consultations/{appointment_id}/rtc-token")
]
update_file(uf_path, uf_replacements)

# INTERACTION_SPEC.md
is_path = os.path.join(base_dir, "docs", "INTERACTION_SPEC.md")
is_replacements = [
    (r"WebRTC", "Agora"),
    (r"ICE", "Agora"),
    (r"digital_signature_hash", "integrity_verification_hash"),
    (r"/prescriptions/upload", "/api/v1/appointments/{id}/prescriptions"),
    (r"URLs", "document_id")
]
update_file(is_path, is_replacements)

# ERROR_HANDLING.md
eh_path = os.path.join(base_dir, "docs", "ERROR_HANDLING.md")
eh_replacements = [
    (r"Payment gateway did not respond. No funds were debited.", "We could not confirm the payment status. Please do not pay again while we verify the transaction."),
    (r"WebRTC", "Agora"),
    (r"WEBRTC_SIGNALING", "AGORA_RTC")
]
update_file(eh_path, eh_replacements)
with open(eh_path, 'a', encoding='utf-8') as f:
    f.write("\n## 4. Logging Rules\nNever dump entire request objects, JWTs, authorization headers, prescription content, uploaded filenames, or PHI. Use allow-list approach for logged fields.\n")
    f.write("\n*Note: System error logs are NOT an adequate security/medical audit log. Reference the new audit_events table for healthcare audit trail.*\n")

# VALIDATION_RULES.md
vr_path = os.path.join(base_dir, "docs", "VALIDATION_RULES.md")
vr_replacements = [
    (r"HMAC-SHA256 hash verified against doctor private secret", "HMAC-SHA256 integrity verification hash computed with server-managed key"),
    (r"Digital Signature", "Integrity Verification Hash"),
    (r"BMDC Number", "Medical License Number"),
    (r"৳ \d+", "configurable amount"),
    (r"fee >= 100\.00 AND fee <= 50000\.00", "fee >= min_fee AND fee <= max_fee")
]
update_file(vr_path, vr_replacements)
with open(vr_path, 'a', encoding='utf-8') as f:
    f.write("\n## 4. File Upload Security Rules\n")
    f.write("- Magic-byte validation (server verifies content matches MIME)\n")
    f.write("- Image re-encoding (strip EXIF, re-encode to safe format)\n")
    f.write("- PDF structural validation\n")
    f.write("- No executable/SVG/HTML content\n")
    f.write("- Per-user storage quotas\n")
    f.write("- Server-generated object names\n")

print("Done")
