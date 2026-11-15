//! Peer Navigator profiles and support slots (migration
//! `20261222000002_peer_navigators`). Peers are their own role and tables;
//! see docs/peer-navigator/PEER_NAVIGATOR_PLAN.md.

use crate::domain::ids::UserId;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;
use uuid::Uuid;

/// One peer's full profile (own view and Admin view).
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PeerProfile {
    pub id: Uuid,
    pub user_id: UserId,
    pub full_name: String,
    pub county: String,
    pub organization: Option<String>,
    pub bio: Option<String>,
    pub approval_status: String,
    pub approval_note: Option<String>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const PROFILE_COLUMNS: &str = "id, user_id, full_name, county, organization, bio, approval_status, approval_note, reviewed_at, created_at, updated_at";

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PeerSlot {
    pub id: Uuid,
    pub peer_id: Uuid,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub status: String,
}

/// What a patient sees when choosing a peer: display fields only.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PeerDirectoryEntry {
    pub id: Uuid,
    pub full_name: String,
    pub county: String,
    pub organization: Option<String>,
    pub bio: Option<String>,
    pub next_available: Option<DateTime<Utc>>,
}

/// A peer as listed for Admin review.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PeerAdminEntry {
    pub id: Uuid,
    pub full_name: String,
    pub email: Option<String>,
    pub phone_number: String,
    pub county: String,
    pub organization: Option<String>,
    pub approval_status: String,
    pub approval_note: Option<String>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[allow(clippy::too_many_arguments)]
pub async fn insert<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    user_id: UserId,
    full_name: &str,
    county: &str,
    organization: Option<&str>,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO peer_profiles (id, user_id, full_name, county, organization, created_at, updated_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$6)",
    )
    .bind(id)
    .bind(user_id)
    .bind(full_name)
    .bind(county)
    .bind(organization)
    .bind(now)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn find_by_user_id<'e, E: PgExecutor<'e>>(
    executor: E,
    user_id: UserId,
) -> Result<Option<PeerProfile>, sqlx::Error> {
    sqlx::query_as::<_, PeerProfile>(&format!(
        "SELECT {PROFILE_COLUMNS} FROM peer_profiles WHERE user_id = $1"
    ))
    .bind(user_id)
    .fetch_optional(executor)
    .await
}

pub async fn find_by_id<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
) -> Result<Option<PeerProfile>, sqlx::Error> {
    sqlx::query_as::<_, PeerProfile>(&format!(
        "SELECT {PROFILE_COLUMNS} FROM peer_profiles WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(executor)
    .await
}

/// Partial update of the peer's own profile text; `None` keeps the value.
pub async fn update_own<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    full_name: Option<&str>,
    county: Option<&str>,
    organization: Option<&str>,
    bio: Option<&str>,
) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE peer_profiles SET full_name = COALESCE($2, full_name), county = COALESCE($3, county), \
         organization = CASE WHEN $4::text IS NULL THEN organization ELSE NULLIF($4, '') END, \
         bio = CASE WHEN $5::text IS NULL THEN bio ELSE NULLIF($5, '') END, \
         updated_at = clock_timestamp() WHERE id = $1",
    )
    .bind(id)
    .bind(full_name)
    .bind(county)
    .bind(organization)
    .bind(bio)
    .execute(executor)
    .await?
    .rows_affected())
}

pub async fn admin_list<'e, E: PgExecutor<'e>>(
    executor: E,
    status: Option<&str>,
) -> Result<Vec<PeerAdminEntry>, sqlx::Error> {
    sqlx::query_as::<_, PeerAdminEntry>(
        "SELECT p.id, p.full_name, u.email, u.phone_number, p.county, p.organization, \
                p.approval_status, p.approval_note, p.reviewed_at, p.created_at \
         FROM peer_profiles p JOIN users u ON u.id = p.user_id \
         WHERE ($1::text IS NULL OR p.approval_status = $1) \
         ORDER BY (p.approval_status = 'PENDING') DESC, p.created_at DESC LIMIT 500",
    )
    .bind(status)
    .fetch_all(executor)
    .await
}

/// Admin decision. Returns the previous status, or `None` if no such peer.
pub async fn set_approval<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    status: &str,
    note: Option<&str>,
    reviewer: UserId,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        "WITH prev AS (SELECT approval_status FROM peer_profiles WHERE id = $1 FOR UPDATE) \
         UPDATE peer_profiles p SET approval_status = $2, approval_note = $3, reviewed_by = $4, \
                reviewed_at = clock_timestamp(), updated_at = clock_timestamp() \
         FROM prev WHERE p.id = $1 RETURNING prev.approval_status",
    )
    .bind(id)
    .bind(status)
    .bind(note)
    .bind(reviewer)
    .fetch_optional(executor)
    .await
}

pub async fn list_own_slots<'e, E: PgExecutor<'e>>(
    executor: E,
    peer_id: Uuid,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<PeerSlot>, sqlx::Error> {
    sqlx::query_as::<_, PeerSlot>(
        "SELECT id, peer_id, start_time, end_time, status FROM peer_slots \
         WHERE peer_id = $1 AND status <> 'CANCELLED' AND start_time >= $2 AND start_time < $3 \
         ORDER BY start_time",
    )
    .bind(peer_id)
    .bind(from)
    .bind(to)
    .fetch_all(executor)
    .await
}

pub async fn slot_overlaps<'e, E: PgExecutor<'e>>(
    executor: E,
    peer_id: Uuid,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM peer_slots WHERE peer_id = $1 AND status <> 'CANCELLED' \
         AND start_time < $3 AND end_time > $2)",
    )
    .bind(peer_id)
    .bind(start)
    .bind(end)
    .fetch_one(executor)
    .await
}

pub async fn insert_slot<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    peer_id: Uuid,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO peer_slots (id, peer_id, start_time, end_time, status) VALUES ($1,$2,$3,$4,'AVAILABLE')",
    )
    .bind(id)
    .bind(peer_id)
    .bind(start)
    .bind(end)
    .execute(executor)
    .await?;
    Ok(())
}

/// Cancels one of the peer's own slots; only an open (AVAILABLE) slot can be
/// removed. Returns false when there is no such open slot.
pub async fn cancel_open_slot<'e, E: PgExecutor<'e>>(
    executor: E,
    peer_id: Uuid,
    slot_id: Uuid,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE peer_slots SET status = 'CANCELLED' WHERE id = $1 AND peer_id = $2 AND status = 'AVAILABLE'",
    )
    .bind(slot_id)
    .bind(peer_id)
    .execute(executor)
    .await?
    .rows_affected()
        == 1)
}

/// Approved peers with their next open slot, for patients.
pub async fn directory<'e, E: PgExecutor<'e>>(
    executor: E,
) -> Result<Vec<PeerDirectoryEntry>, sqlx::Error> {
    sqlx::query_as::<_, PeerDirectoryEntry>(
        "SELECT p.id, p.full_name, p.county, p.organization, p.bio, \
                (SELECT min(s.start_time) FROM peer_slots s \
                  WHERE s.peer_id = p.id AND s.status = 'AVAILABLE' AND s.start_time > clock_timestamp()) AS next_available \
         FROM peer_profiles p WHERE p.approval_status = 'APPROVED' \
         ORDER BY next_available NULLS LAST, p.full_name LIMIT 200",
    )
    .fetch_all(executor)
    .await
}

/// A peer's open, future slots, for patients.
pub async fn open_slots<'e, E: PgExecutor<'e>>(
    executor: E,
    peer_id: Uuid,
) -> Result<Vec<PeerSlot>, sqlx::Error> {
    sqlx::query_as::<_, PeerSlot>(
        "SELECT s.id, s.peer_id, s.start_time, s.end_time, s.status FROM peer_slots s \
         JOIN peer_profiles p ON p.id = s.peer_id AND p.approval_status = 'APPROVED' \
         WHERE s.peer_id = $1 AND s.status = 'AVAILABLE' AND s.start_time > clock_timestamp() \
         ORDER BY s.start_time LIMIT 300",
    )
    .bind(peer_id)
    .fetch_all(executor)
    .await
}

// ---------------------------------------------------------------------------
// Requests (Phase 2, migration 20261222000005_peer_requests)
// ---------------------------------------------------------------------------

/// The parties and state of one request, for authorization and transitions.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PeerRequestCore {
    pub id: Uuid,
    pub patient_user_id: UserId,
    pub peer_user_id: UserId,
    pub peer_id: Uuid,
    pub slot_id: Uuid,
    pub status: String,
}

/// A request as the patient sees it.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PatientPeerRequest {
    pub id: Uuid,
    pub peer_id: Uuid,
    pub peer_name: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub support_type: String,
    pub topic: String,
    pub status: String,
    pub decline_note: Option<String>,
    pub created_at: DateTime<Utc>,
    pub decided_at: Option<DateTime<Utc>>,
    /// Set once the peer accepts: the chat opens.
    pub conversation_id: Option<Uuid>,
}

/// A request as the peer sees it: no patient identity beyond the snapshot.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PeerInboxRequest {
    pub id: Uuid,
    pub patient_display_name: String,
    pub patient_age_range: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub support_type: String,
    pub topic: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub decided_at: Option<DateTime<Utc>>,
    pub conversation_id: Option<Uuid>,
}

/// The peer behind a slot that is still open and in the future, locking it.
pub async fn open_slot_peer(
    conn: &mut sqlx::PgConnection,
    slot_id: Uuid,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT peer_id FROM peer_slots \
         WHERE id = $1 AND status = 'AVAILABLE' AND start_time > clock_timestamp() FOR UPDATE",
    )
    .bind(slot_id)
    .fetch_optional(conn)
    .await
}

/// Holds the slot for a request just inserted (AVAILABLE -> REQUESTED). Run
/// after the request row exists: a patient can only see a REQUESTED slot
/// through their own request, and PostgreSQL checks the updated row against
/// the read policy.
pub async fn hold_slot(conn: &mut sqlx::PgConnection, slot_id: Uuid) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE peer_slots SET status = 'REQUESTED' WHERE id = $1 AND status = 'AVAILABLE'",
    )
    .bind(slot_id)
    .execute(conn)
    .await?
    .rows_affected()
        == 1)
}

pub async fn set_slot_status<'e, E: PgExecutor<'e>>(
    executor: E,
    slot_id: Uuid,
    from: &str,
    to: &str,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("UPDATE peer_slots SET status = $3 WHERE id = $1 AND status = $2")
            .bind(slot_id)
            .bind(from)
            .bind(to)
            .execute(executor)
            .await?
            .rows_affected()
            == 1,
    )
}

/// The approved peer's user id, as visible to the requesting patient.
pub async fn approved_peer_user<'e, E: PgExecutor<'e>>(
    executor: E,
    peer_id: Uuid,
) -> Result<Option<UserId>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT user_id FROM peer_profiles WHERE id = $1 AND approval_status = 'APPROVED'",
    )
    .bind(peer_id)
    .fetch_optional(executor)
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_request<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    patient_id: crate::domain::ids::PatientProfileId,
    patient_user_id: UserId,
    peer_id: Uuid,
    peer_user_id: UserId,
    slot_id: Uuid,
    support_type: &str,
    topic: &str,
    patient_display_name: &str,
    patient_age_range: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO peer_requests (id, patient_id, patient_user_id, peer_id, peer_user_id, slot_id, \
         support_type, topic, patient_display_name, patient_age_range) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
    )
    .bind(id)
    .bind(patient_id)
    .bind(patient_user_id)
    .bind(peer_id)
    .bind(peer_user_id)
    .bind(slot_id)
    .bind(support_type)
    .bind(topic)
    .bind(patient_display_name)
    .bind(patient_age_range)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn pending_count_for_patient<'e, E: PgExecutor<'e>>(
    executor: E,
    patient_user_id: UserId,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT count(*) FROM peer_requests WHERE patient_user_id = $1 AND status = 'PENDING'",
    )
    .bind(patient_user_id)
    .fetch_one(executor)
    .await
}

pub async fn find_request_for_update<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
) -> Result<Option<PeerRequestCore>, sqlx::Error> {
    sqlx::query_as::<_, PeerRequestCore>(
        "SELECT id, patient_user_id, peer_user_id, peer_id, slot_id, status \
         FROM peer_requests WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(executor)
    .await
}

pub async fn set_request_status<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    to: &str,
    decline_note: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE peer_requests SET status = $2, decline_note = COALESCE($3, decline_note), \
         decided_at = CASE WHEN $2 IN ('ACCEPTED', 'DECLINED') THEN clock_timestamp() ELSE decided_at END, \
         cancelled_at = CASE WHEN $2 = 'CANCELLED' THEN clock_timestamp() ELSE cancelled_at END, \
         completed_at = CASE WHEN $2 = 'COMPLETED' THEN clock_timestamp() ELSE completed_at END \
         WHERE id = $1",
    )
    .bind(id)
    .bind(to)
    .bind(decline_note)
    .execute(executor)
    .await?;
    Ok(())
}

const PATIENT_REQUEST_SELECT: &str =
    "SELECT r.id, r.peer_id, p.full_name AS peer_name, s.start_time, s.end_time, \
     r.support_type, r.topic, r.status, r.decline_note, r.created_at, r.decided_at, \
     cv.id AS conversation_id \
     FROM peer_requests r JOIN peer_slots s ON s.id = r.slot_id \
     LEFT JOIN peer_profiles p ON p.id = r.peer_id \
     LEFT JOIN peer_conversations cv ON cv.request_id = r.id";

pub async fn patient_requests<'e, E: PgExecutor<'e>>(
    executor: E,
    patient_user_id: UserId,
) -> Result<Vec<PatientPeerRequest>, sqlx::Error> {
    sqlx::query_as::<_, PatientPeerRequest>(&format!(
        "{PATIENT_REQUEST_SELECT} WHERE r.patient_user_id = $1 ORDER BY r.created_at DESC LIMIT 200"
    ))
    .bind(patient_user_id)
    .fetch_all(executor)
    .await
}

pub async fn patient_request<'e, E: PgExecutor<'e>>(
    executor: E,
    patient_user_id: UserId,
    id: Uuid,
) -> Result<Option<PatientPeerRequest>, sqlx::Error> {
    sqlx::query_as::<_, PatientPeerRequest>(&format!(
        "{PATIENT_REQUEST_SELECT} WHERE r.patient_user_id = $1 AND r.id = $2"
    ))
    .bind(patient_user_id)
    .bind(id)
    .fetch_optional(executor)
    .await
}

const PEER_REQUEST_SELECT: &str =
    "SELECT r.id, r.patient_display_name, r.patient_age_range, s.start_time, s.end_time, \
     r.support_type, r.topic, r.status, r.created_at, r.decided_at, cv.id AS conversation_id \
     FROM peer_requests r JOIN peer_slots s ON s.id = r.slot_id \
     LEFT JOIN peer_conversations cv ON cv.request_id = r.id";

pub async fn peer_inbox<'e, E: PgExecutor<'e>>(
    executor: E,
    peer_user_id: UserId,
    status: Option<&str>,
) -> Result<Vec<PeerInboxRequest>, sqlx::Error> {
    sqlx::query_as::<_, PeerInboxRequest>(&format!(
        "{PEER_REQUEST_SELECT} WHERE r.peer_user_id = $1 AND ($2::text IS NULL OR r.status = $2) \
         ORDER BY (r.status = 'PENDING') DESC, s.start_time LIMIT 300"
    ))
    .bind(peer_user_id)
    .bind(status)
    .fetch_all(executor)
    .await
}

pub async fn peer_request<'e, E: PgExecutor<'e>>(
    executor: E,
    peer_user_id: UserId,
    id: Uuid,
) -> Result<Option<PeerInboxRequest>, sqlx::Error> {
    sqlx::query_as::<_, PeerInboxRequest>(&format!(
        "{PEER_REQUEST_SELECT} WHERE r.peer_user_id = $1 AND r.id = $2"
    ))
    .bind(peer_user_id)
    .bind(id)
    .fetch_optional(executor)
    .await
}
