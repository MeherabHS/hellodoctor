# Consultation device test guide

Use the latest [Patient ARM64 correction APK](hellodoctor-patient-arm64-pre-e2e-corrections-20261016.apk) (**1.0.1+5**) and [Doctor ARM64 correction APK](hellodoctor-doctor-arm64-pre-e2e-corrections-20261016.apk) (**1.0.1+6**). These retain the previous OTP-navigation, system-safe-area, Doctor startup and notification release fixes. The latest APKs are built and signature-checked but **not installed on the phone**; on 2026-10-16 the user explicitly deferred physical-device verification. See [queue/availability](../docs/PRE_E2E_QUEUE_AVAILABILITY_CORRECTION.md), [payment UX](../docs/PATIENT_PAYMENT_UX_VERIFICATION.md), and [chat attachments](../docs/DOCTOR_CHAT_ATTACHMENT_VERIFICATION.md).

These development builds use `http://192.168.0.105:8080/api/v1`. Keep the phone and this computer on the same Wi-Fi and the local backend running. These are signed development test builds with development OTP explicitly enabled, rather than production-distribution artifacts.

| Account | Email | Password |
| --- | --- | --- |
| Patient | `patient.test3@hellodoctor.local` | `PatientTest!2026` |
| Assigned synthetic Doctor | `sabrina.akter@hellodoctor.health` | `Password123!` |

Patient phone on registration/profile: `+254700009999`. Login uses **email + password + the one-time code displayed by the development build**. The code changes and can be used only once. Azure mail delivery is not configured locally. Security rate limits remain enabled; login attempts share a five-attempt/15-minute IP window.

To test an already activated chat, open Patient **Services → Chat with Doctor → Active → Open consultation**. Use the consultation with reference `APT-20261015-5CC0D508`; it contains the same Patient message, Doctor Web reply and Doctor Flutter reply used for live verification. All are synthetic test messages. Other development verification bookings may also appear, each with a separate appointment/conversation.

To test a new booking:

1. Choose Chat from the Doctor's Consult choices, review the actual fee, and select an available slot. The current backend requires a slot for Chat as well as Video.
2. Confirm the booking. Verify **Payment required**, **Proceed to payment**, and that consultation entry is absent. Tapping Proceed calls the owned backend payment action; current live checkout is unavailable and must never show fake success.
3. The local environment has no live M-Pesa checkout. For this synthetic test account only, run the existing-webhook development helper from the repository root using the reference displayed in the app:

   ```powershell
   python docs/audit_evidence/patient_consultation/confirm_dev_booking.py APT-YYYYMMDD-XXXXXXXX
   ```

4. Use the secondary Refresh status action. Once the backend confirms payment, open the conversation. Send a message and check the same conversation on Doctor Flutter and Doctor Web at `http://127.0.0.1:3001` on this computer.
5. Reply from Doctor Web or Doctor Flutter, then reload/reopen Patient. Confirm that history is preserved and no second consultation/charge is created by opening it.

The helper refuses production and other accounts, uses the existing signed development callback, and never writes a paid state directly to the database. It does not simulate real money or provider acceptance. An already confirmed reference does not send another callback.

Video bookings now display the correct Patient and video action on Doctor clients. Server participant/status/time gates remain enforced. Real Agora audio/video transmission is still deferred in the existing implementation; do not interpret an authorized session as a completed media call.

No physical device was connected during the original consultation verification. On 2026-10-15 the Doctor launch fix passed three cold starts on the connected phone; broader device checks of consultations, keyboard behavior, reconnects, notifications and background behavior remain open. See the [engineering verification report](../docs/PATIENT_CHAT_CONSULTATION_VERIFICATION.md) for screenshots, exact test counts and limitations.


Latest correction limits: live payment collection has no configured adapter, and private document object storage is unconfigured. Clinical-image attachment authorization/deduplication/storage behavior passes backend tests using the established test-only storage fixture; successful live upload is not claimed. No full physical ecosystem simulation or video/Agora media verification was performed in this correction pass. The synthetic Doctor has two new future availability slots created through the actual Doctor schedule API; all historical slots are retained.
