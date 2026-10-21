//! Attribute-Based Access Control.
//!
//! Role-level gating uses the canonical human ADMIN product role. Historical
//! legacy role values remain parseable but do not grant active capabilities.
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
    SubmitHealthQuestion,
    PatientRead,
    PatientManageProfile,
    ManagePatientPaymentMethod,

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

    /// The platform's own merchant/collection configuration — distinct from
    /// `ManagePatientPaymentMethod` (how a patient pays) and `ViewOwnWallet`
    /// (where a doctor is paid): this is the platform's own identity with
    /// its payment provider(s). Admin-only, same shape as `SimulateLog`.
    ManagePlatformPaymentMethod,
    ExecuteDataRightsAction,
}

pub fn authorize(ctx: &AuthContext, action: Action) -> Result<(), AppError> {
    use Action::*;
    use UserRole::*;

    let allowed = match action {
        AnyAuthenticated => true,

        BookAppointment
        | UploadPatientIntake
        | SubmitGrievance
        | SubmitHealthQuestion
        | PatientRead
        | PatientManageProfile
        | ManagePatientPaymentMethod => {
            matches!(ctx.role, Patient)
        }

        CompleteConsultation | UploadDoctorPrescription | ViewOwnWallet => {
            matches!(ctx.role, Doctor)
        }

        DoctorRead | DoctorManageOwnSchedule | DoctorManageProfile | DoctorQna | DoctorArticles => {
            matches!(ctx.role, Doctor)
        }

        ViewGrievanceQueue
        | AdjudicateGrievance
        | ViewAdminOverview
        | ManageDoctorCredentialing
        | ManageProfessionalVerification
        | ViewDoctorDossier
        | ViewLegalAcceptances
        | InitiateDisbursement
        | ViewFinanceSummary
        | ViewTransactions
        | ViewCommandTelemetry
        | ManageSlots
        | ViewPatientDirectory
        | ViewPatientDossier
        | ViewComplianceAlerts
        | InvestigateIdentity
        | ViewLogs
        | SimulateLog
        | ViewSystemErrors
        | AcknowledgeSystemError
        | ResolveSystemError
        | ViewConsultationTimeline
        | ManagePlatformPaymentMethod
        | ExecuteDataRightsAction => matches!(ctx.role, Admin),
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
