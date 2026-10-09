> Current canonical ownership / Doctor Q&A correction (2026-10-08): [identity enforcement, audited Compliance trace and Q&A rendering evidence](USER_ISOLATION_AND_QNA.md); [all-table ownership inventory](USER_OWNERSHIP_INVENTORY.md). Migration 20261026000001 adds the database guards. Q&A real-device inspection remains pending until a device is connected; widget/golden renders are separate evidence. Earlier verification counts below are historical.

> Current public-exposure/session hardening and direct Doctor Articles (2026-10-08): [security controls and deployment requirements](PUBLIC_EXPOSURE_HARDENING.md), [Doctor Articles without Admin review](DOCTOR_ARTICLES.md). Earlier dated results below are historical snapshots. Slot approvals are explicitly excluded by product policy.

# Admin professional-registration mapping — Kenya

Current authority: Kenya Medical Practitioners and Dentists Council (KMPDC), KE.
This is internal manual HelloDoctor review, not external registry confirmation.

| Admin surface | Backend API under `/api/v1` | Authorization / contract |
| --- | --- | --- |
| Doctor Verification | `GET /admin/doctors/pending-verification` | ClinicalAdmin/PlatformAdmin; PENDING only |
| Verify Registration | `POST /admin/doctors/{id}/verify` | PENDING → VERIFIED; optional note |
| Reject Application | `POST /admin/doctors/{id}/reject` | PENDING → REJECTED; optional note |
| Doctor directory/dossier | `GET /admin/doctors`, `GET /admin/doctors/{id}/dossier` | Existing role access; explicit review state |
| Overview pending count | `GET /admin/overview` | `pending_professional_verification` |
| Create Doctor | `POST /admin/doctors` | Existing ClinicalAdmin/PlatformAdmin gate; creates PENDING/off-duty |

The browser uses the existing same-origin HttpOnly-cookie BFF; tokens are not
exposed to browser JavaScript. Review conflicts remain failures (409). The UI
refreshes the actual pending queue after a successful response, so verified and
rejected applications both leave it. No unsupported clarification/reapplication
action is presented. Ordinary Doctors cannot review themselves or other Doctors.

See [migration, semantics and evidence](KENYA_PROFESSIONAL_VERIFICATION.md).
