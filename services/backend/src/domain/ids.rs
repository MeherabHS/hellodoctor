//! Newtype wrappers for the three ID spaces that have actually been confused
//! with each other in this codebase's real history:
//! `consultation_service.rs::mint_agora_token` once compared a
//! `patient_profiles.id` (`PatientProfileId`) directly against a `users.id`
//! (`UserId`) — two different ID spaces that are never equal for a real
//! distinct record, but both were plain `Uuid` and the comparison compiled
//! fine while being silently wrong. Wrapping these three ID families turns
//! that exact class of mistake into a compile error.
//!
//! Deliberately scoped to only these three — `appointment_id`/`transaction_id`/
//! `slot_id`/etc. have never actually been confused with anything else in this
//! project's history, so wrapping them would be pure churn with no matching
//! safety win.
//!
//! Each type is a transparent wrapper at the wire/DB boundary: it binds/
//! decodes exactly like a bare `Uuid` (`#[sqlx(transparent)]`, no schema or
//! migration change needed) and serializes to/from JSON as the same plain
//! UUID string clients already send and receive — this is a Rust-type-system-
//! only change, never a wire-format change.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! uuid_newtype {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
        #[serde(transparent)]
        #[sqlx(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            /// Generates a fresh random (v4) id — the same scheme every
            /// bare-`Uuid` id in this codebase already used.
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// Returns the raw `Uuid` for the few genuine boundary points
            /// that need it (constructing a new id, a generic audit-log call
            /// that takes a bare `Uuid`, etc.).
            pub fn into_inner(self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl From<Uuid> for $name {
            fn from(id: Uuid) -> Self {
                Self(id)
            }
        }

        impl From<$name> for Uuid {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        impl AsRef<Uuid> for $name {
            fn as_ref(&self) -> &Uuid {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }
    };
}

uuid_newtype!(UserId, "Primary key of the `users` table.");
uuid_newtype!(
    PatientProfileId,
    "Primary key of the `patient_profiles` table — distinct from the owning `UserId`."
);
uuid_newtype!(
    DoctorProfileId,
    "Primary key of the `doctor_profiles` table — distinct from the owning `UserId`."
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn different_id_families_are_not_interchangeable_even_with_the_same_raw_uuid() {
        // This is the actual point of these types: the same raw Uuid wrapped
        // in two different families must not be comparable at all (a compile
        // error), which this test can't literally exercise (that's a
        // compile-time property, not a runtime one) — but it does confirm
        // equality within one family still works via the raw value.
        let raw = Uuid::new_v4();
        let a = UserId::from(raw);
        let b = UserId::from(raw);
        assert_eq!(a, b);
        assert_eq!(a.into_inner(), raw);
    }

    #[test]
    fn display_matches_the_raw_uuid_string() {
        let raw = Uuid::new_v4();
        let id = DoctorProfileId::from(raw);
        assert_eq!(id.to_string(), raw.to_string());
    }

    #[test]
    fn serializes_as_a_plain_uuid_string_not_a_wrapped_object() {
        let raw = Uuid::new_v4();
        let id = PatientProfileId::from(raw);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"{raw}\""));
        let back: PatientProfileId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }
}
