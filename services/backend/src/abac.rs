//! Attribute-Based Access Control.
//!
//! Role-level gating lives here, matching `docs/AUTH_SECURITY.md` §3's matrix (extended
//! to cover every admin route the matrix doesn't name individually, using the same
//! role-grouping logic — e.g. subsystem diagnostics stays with SECURITY_ADMIN/PLATFORM_ADMIN).
//! Resource-level ownership (a doctor reading only their own wallet, a patient reading
//! only their own appointment) is NOT expressed here — it is checked in the handler
//! after the resource is loaded, since it depends on data this function doesn't have.
//! Call `authorize()` as the first line of every protected handler.

use crate::auth_context::AuthContext;
use crate::domain::models::UserRole;
use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Any authenticated role (patient, doctor, or any admin) may browse/read — these
    /// still require a valid token, just not a specific role. Narrower per-resource
    /// ownership (e.g. "this appointment belongs to you") is enforced in the handler.
    AnyAuthenticated,

    BookAppointment,
    UploadPatientIntake,
    SubmitGrievance,

    CompleteConsultation,
    UploadDoctorPrescription,
    ViewOwnWallet,
    DoctorRead,
    DoctorManageOwnSchedule,
    DoctorManageProfile,
    DoctorQna,
    DoctorArticles,

    ViewGrievanceQueue,
    AdjudicateGrievance,

    ViewAdminOverview,
    ManageDoctorCredentialing,
    ViewDoctorDossier,
    InitiateDisbursement,
    ViewFinanceSummary,
    ViewTransactions,
    ViewCommandTelemetry,
    ManageSlots,
    ViewPatientDirectory,
    ViewPatientDossier,
    ManageProfessionalVerification,
    /// Compliance evidence: who accepted which Doctor Terms/Privacy/
    /// Professional Verification/Telemedicine document version, and when.
    /// A Doctor reading their own acceptance history is not this action —
    /// that's gated by `ctx.role == Doctor` plus ownership in the handler,
    /// the same shape as `ViewOwnWallet`.
    ViewLegalAcceptances,
    ViewComplianceAlerts,
    InvestigateIdentity,
    ViewLogs,
    SimulateLog,

    /// Operational System Errors surface — a distinct area from `ViewLogs`
    /// (the existing generic `audit_events`-backed "Logs" screen): genuine
    /// backend technical failures, never clinical/service grievances. See
    /// docs/CHECKPOINT.md §8.
    ViewSystemErrors,
    AcknowledgeSystemError,
    ResolveSystemError,

    /// Detailed per-session call event timeline (reconnect/disconnect
    /// reconstruction, correlated grievances/system errors) — same role set
    /// as the grievance queue it's built to support investigating. See
    /// docs/CHECKPOINT.md §8.2.
    ViewConsultationTimeline,
}

pub fn authorize(ctx: &AuthContext, action: Action) -> Result<(), AppError> {
    use Action::*;
    use UserRole::*;

    let allowed = match action {
        AnyAuthenticated => true,

        BookAppointment | UploadPatientIntake | SubmitGrievance => matches!(ctx.role, Patient),

        CompleteConsultation | UploadDoctorPrescription | ViewOwnWallet => {
            matches!(ctx.role, Doctor)
        }

        DoctorRead | DoctorManageOwnSchedule | DoctorManageProfile | DoctorQna | DoctorArticles => {
            matches!(ctx.role, Doctor)
        }

        ViewGrievanceQueue | AdjudicateGrievance => {
            matches!(ctx.role, Compliance | ClinicalAdmin | PlatformAdmin)
        }

        ViewAdminOverview => ctx.role.is_admin(),

        ManageDoctorCredentialing | ManageProfessionalVerification => {
            matches!(ctx.role, ClinicalAdmin | PlatformAdmin)
        }

        ViewDoctorDossier => matches!(
            ctx.role,
            ClinicalAdmin | PlatformAdmin | FinanceAdmin | Compliance
        ),

        ViewLegalAcceptances => matches!(ctx.role, Compliance | PlatformAdmin | ClinicalAdmin),

        InitiateDisbursement | ViewFinanceSummary => {
            matches!(ctx.role, FinanceAdmin | PlatformAdmin)
        }

        ViewTransactions => matches!(ctx.role, FinanceAdmin | PlatformAdmin | Compliance),

        ViewCommandTelemetry => matches!(ctx.role, PlatformAdmin | SecurityAdmin | Support),

        ManageSlots => matches!(ctx.role, PlatformAdmin | Support),

        ViewPatientDirectory | ViewPatientDossier => {
            matches!(
                ctx.role,
                Compliance | PlatformAdmin | ClinicalAdmin | Support
            )
        }

        ViewComplianceAlerts => matches!(ctx.role, Compliance | PlatformAdmin | SecurityAdmin),
        InvestigateIdentity => matches!(ctx.role, Compliance | PlatformAdmin),

        // Narrowed to the single designated "Super Admin" authority —
        // SECURITY_ADMIN — matching the RLS policy tightened in
        // migrations/20261004000003_admin_audit_snapshots (see that
        // migration's own comment for why this role was chosen and why it's
        // a deliberately reversible judgment call). `SimulateLog` (a test/
        // demo utility that only ever inserts a row, never reads the trail)
        // deliberately keeps the wider PLATFORM_ADMIN access it already had.
        ViewLogs => matches!(ctx.role, SecurityAdmin),
        SimulateLog => matches!(ctx.role, SecurityAdmin | PlatformAdmin),

        ViewSystemErrors | AcknowledgeSystemError | ResolveSystemError => {
            matches!(ctx.role, SecurityAdmin | PlatformAdmin)
        }

        ViewConsultationTimeline => matches!(ctx.role, Compliance | ClinicalAdmin | PlatformAdmin),
    };

    if allowed {
        Ok(())
    } else {
        Err(AppError::Forbidden(format!(
            "Role {} is not permitted to perform this action.",
            ctx.role.as_str()
        )))
    }
}
