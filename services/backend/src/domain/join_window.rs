//! Authoritative consultation join-window policy: the ONE place that decides
//! whether a participant may enter a video consultation room.
//!
//! Used by the RTC-token mint path (the real gate) and by the read-only
//! eligibility exposed to Doctor Web and the Patient app, so clients display
//! exactly what the server will enforce and never carry their own copy of the
//! window rules.
//!
//! Policy (identical for Doctor and Patient):
//! * opens `before_mins` before the slot start (default 15), inclusive;
//! * closes `after_mins` after the slot END (default 60), inclusive — it does
//!   NOT close at the scheduled end;
//! * only `CONFIRMED` and `IN_CONSULTATION` appointments are joinable;
//! * only VIDEO appointments have a room.
//!
//! All instants are UTC. Display (EAT) is a client concern.

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;

pub const REASON_OK: &str = "OK";
pub const REASON_NOT_YET_OPEN: &str = "NOT_YET_OPEN";
pub const REASON_WINDOW_CLOSED: &str = "WINDOW_CLOSED";
pub const REASON_INVALID_STATE: &str = "INVALID_STATE";
pub const REASON_NOT_VIDEO: &str = "NOT_VIDEO";
/// Session-level denial (set by the service, which can see the session row).
pub const REASON_NEEDS_REVIEW: &str = "NEEDS_REVIEW";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JoinEligibility {
    pub eligible: bool,
    /// Stable machine-readable reason; one of the `REASON_*` constants.
    pub reason: &'static str,
    pub join_open_at: DateTime<Utc>,
    pub join_close_at: DateTime<Utc>,
    /// Server clock at evaluation, so clients can correct their own skew.
    pub server_time: DateTime<Utc>,
}

pub fn evaluate(
    status: &str,
    modality: &str,
    slot_start: DateTime<Utc>,
    slot_end: DateTime<Utc>,
    now: DateTime<Utc>,
    before_mins: i64,
    after_mins: i64,
) -> JoinEligibility {
    let join_open_at = slot_start - Duration::minutes(before_mins);
    let join_close_at = slot_end + Duration::minutes(after_mins);
    let reason = if modality != "VIDEO" {
        REASON_NOT_VIDEO
    } else if status != "CONFIRMED" && status != "IN_CONSULTATION" {
        REASON_INVALID_STATE
    } else if now < join_open_at {
        REASON_NOT_YET_OPEN
    } else if now > join_close_at {
        REASON_WINDOW_CLOSED
    } else {
        REASON_OK
    };
    JoinEligibility {
        eligible: reason == REASON_OK,
        reason,
        join_open_at,
        join_close_at,
        server_time: now,
    }
}

impl JoinEligibility {
    /// A session already marked `NEEDS_REVIEW` cannot be rejoined, whatever
    /// the clock says. Only downgrades an otherwise-eligible result.
    pub fn with_recovery_state(mut self, recovery_state: Option<&str>) -> Self {
        if self.eligible && recovery_state == Some("NEEDS_REVIEW") {
            self.eligible = false;
            self.reason = REASON_NEEDS_REVIEW;
        }
        self
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

/// Human-readable message for a denial reason (client text is derived from the
/// stable `reason`, never string-matched from this).
pub fn denial_message(reason: &str) -> &'static str {
    match reason {
        REASON_NOT_YET_OPEN => "The consultation room is not open yet.",
        REASON_WINDOW_CLOSED => "This consultation's joining window has closed.",
        REASON_INVALID_STATE => "This appointment is not in a state that allows joining.",
        REASON_NOT_VIDEO => "Video access is only available for video consultations.",
        REASON_NEEDS_REVIEW => "This consultation needs review before it can be rejoined.",
        _ => "This consultation cannot be joined.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn t(h: u32, m: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 11, h, m, s).unwrap()
    }
    // slot 12:00–12:10 UTC (15:00–15:10 EAT); window 11:45:00 ..= 13:10:00 UTC
    fn at(now: DateTime<Utc>, status: &str) -> JoinEligibility {
        evaluate(status, "VIDEO", t(12, 0, 0), t(12, 10, 0), now, 15, 60)
    }

    #[test]
    fn sixteen_minutes_before_start_is_too_early() {
        let e = at(t(11, 44, 0), "CONFIRMED");
        assert!(!e.eligible);
        assert_eq!(e.reason, REASON_NOT_YET_OPEN);
    }
    #[test]
    fn exactly_fifteen_minutes_before_start_is_open() {
        assert!(at(t(11, 45, 0), "CONFIRMED").eligible);
        assert_eq!(at(t(11, 44, 59), "CONFIRMED").reason, REASON_NOT_YET_OPEN);
    }
    #[test]
    fn start_and_end_are_inside_the_window() {
        assert!(at(t(12, 0, 0), "CONFIRMED").eligible);
        assert!(at(t(12, 10, 0), "IN_CONSULTATION").eligible);
    }
    #[test]
    fn grace_after_end_is_permitted_until_exactly_sixty_minutes() {
        assert!(at(t(12, 40, 0), "CONFIRMED").eligible);
        assert!(at(t(13, 10, 0), "CONFIRMED").eligible);
        let late = at(t(13, 10, 1), "CONFIRMED");
        assert!(!late.eligible);
        assert_eq!(late.reason, REASON_WINDOW_CLOSED);
    }
    #[test]
    fn only_confirmed_and_in_consultation_may_join() {
        for s in [
            "COMPLETED",
            "CANCELLED",
            "PENDING_PAYMENT",
            "WAITING",
            "REFUNDED",
            "EXPIRED",
        ] {
            let e = at(t(12, 5, 0), s);
            assert!(!e.eligible, "{s}");
            assert_eq!(e.reason, REASON_INVALID_STATE, "{s}");
        }
    }
    #[test]
    fn chat_appointments_have_no_video_room() {
        let e = evaluate(
            "CONFIRMED",
            "CHAT",
            t(12, 0, 0),
            t(12, 10, 0),
            t(12, 5, 0),
            15,
            60,
        );
        assert_eq!(e.reason, REASON_NOT_VIDEO);
    }
    #[test]
    fn window_is_utc_and_independent_of_any_display_timezone() {
        // 15:00 EAT == 12:00 UTC: a client whose device is on UTC+6 (or any
        // zone) gets the same open/close instants — the policy never sees a zone.
        let e = at(t(11, 45, 0), "CONFIRMED");
        assert_eq!(e.join_open_at, t(11, 45, 0));
        assert_eq!(e.join_close_at, t(13, 10, 0));
    }
    #[test]
    fn interrupted_session_needing_review_is_not_joinable() {
        let e = at(t(12, 5, 0), "IN_CONSULTATION").with_recovery_state(Some("NEEDS_REVIEW"));
        assert!(!e.eligible);
        assert_eq!(e.reason, REASON_NEEDS_REVIEW);
        // other recovery states and no session leave a valid window untouched
        assert!(
            at(t(12, 5, 0), "IN_CONSULTATION")
                .with_recovery_state(Some("GRACE_PERIOD"))
                .eligible
        );
        assert!(
            at(t(12, 5, 0), "CONFIRMED")
                .with_recovery_state(None)
                .eligible
        );
        // never upgrades an already-denied result
        let early = at(t(11, 0, 0), "CONFIRMED").with_recovery_state(Some("NEEDS_REVIEW"));
        assert_eq!(early.reason, REASON_NOT_YET_OPEN);
    }
    #[test]
    fn configurable_window_is_honoured() {
        let e = evaluate(
            "CONFIRMED",
            "VIDEO",
            t(12, 0, 0),
            t(12, 10, 0),
            t(11, 54, 0),
            5,
            0,
        );
        assert_eq!(e.reason, REASON_NOT_YET_OPEN);
        assert!(
            evaluate(
                "CONFIRMED",
                "VIDEO",
                t(12, 0, 0),
                t(12, 10, 0),
                t(11, 55, 0),
                5,
                0
            )
            .eligible
        );
        assert_eq!(
            evaluate(
                "CONFIRMED",
                "VIDEO",
                t(12, 0, 0),
                t(12, 10, 0),
                t(12, 10, 1),
                5,
                0
            )
            .reason,
            REASON_WINDOW_CLOSED
        );
    }
}
