use crate::{
    abac::{self, Action},
    auth_context::AuthContext,
    domain::models::UserRole,
    error::AppError,
    repository::{
        appointment_repo, chat_repo, email_outbox_repo, grievance_repo, notification_repo,
        AppState, DbRole,
    },
    services::consultation_trace::{self, Event},
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Acknowledgement {
    pub event: String,
    pub conversation_id: Option<Uuid>,
    #[serde(default)]
    pub message_ids: Vec<Uuid>,
    pub client_occurred_at: Option<chrono::DateTime<chrono::Utc>>,
}
pub async fn acknowledge(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(appointment): Path<Uuid>,
    Json(body): Json<Acknowledgement>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    if body.message_ids.len() > 50
        || ![
            "CONSULTATION_OPENED",
            "CHAT_OPENED",
            "CHAT_READ_ACKNOWLEDGED",
            "CLIENT_REPORTED_OFFLINE",
            "CLIENT_RECONNECTED",
            "CHAT_REFRESH_FAILED",
            "CHAT_REFRESH_SUCCEEDED",
        ]
        .contains(&body.event.as_str())
    {
        return Err(AppError::Validation("Unsupported acknowledgement.".into()));
    }
    crate::rate_limit::check_and_increment(
        &state.redis,
        &format!("trace-ack:{}", ctx.user_id),
        120,
        60,
    )
    .await?;
    let mut tx = state.begin_authed(&ctx).await?;
    appointment_repo::find_by_id(&mut *tx, appointment)
        .await?
        .ok_or_else(|| AppError::NotFound("Consultation not found.".into()))?;
    let allowed: bool = sqlx::query_scalar("SELECT consultation_participant_allowed($1)")
        .bind(appointment)
        .fetch_one(&mut *tx)
        .await?;
    if !allowed {
        return Err(AppError::NotFound("Consultation not found.".into()));
    }
    if let Some(id) = body.conversation_id {
        let conv = chat_repo::find_conversation(&mut *tx, id)
            .await?
            .ok_or_else(|| AppError::NotFound("Conversation not found.".into()))?;
        if conv.appointment_id != appointment {
            return Err(AppError::NotFound("Conversation not found.".into()));
        }
    }
    let event = if body.event == "CONSULTATION_OPENED" {
        match ctx.role {
            UserRole::Patient => "PATIENT_CONSULTATION_OPENED",
            UserRole::Doctor => "DOCTOR_CONSULTATION_OPENED",
            UserRole::Admin => "ADMIN_RESOURCE_VIEWED",
            _ => return Err(AppError::Forbidden("Role unavailable.".into())),
        }
    } else {
        &body.event
    };
    if event == "CHAT_READ_ACKNOWLEDGED" {
        let conversation = body
            .conversation_id
            .ok_or_else(|| AppError::Validation("Conversation is required.".into()))?;
        for message in body.message_ids {
            let msg = chat_repo::find_message(&mut *tx, conversation, message)
                .await?
                .ok_or_else(|| AppError::NotFound("Message not found.".into()))?;
            if msg.sender_id == ctx.user_id {
                continue;
            }
            let inserted=sqlx::query("INSERT INTO chat_read_acknowledgements(conversation_id,user_id,message_id) VALUES($1,$2,$3) ON CONFLICT DO NOTHING").bind(conversation).bind(ctx.user_id).bind(message).execute(&mut *tx).await?.rows_affected();
            if inserted == 1 {
                let mut record = Event::accepted(event, Some(("MESSAGE", message)));
                record.metadata = json!({"client_reported":true});
                consultation_trace::record(&mut tx, appointment, Some(conversation), record)
                    .await?;
            }
        }
    } else {
        let mut record = Event::accepted(event, Some(("APPOINTMENT", appointment)));
        record.metadata =
            json!({"client_reported":true,"client_occurred_at":body.client_occurred_at});
        consultation_trace::record(&mut tx, appointment, body.conversation_id, record).await?;
    }
    tx.commit().await?;
    Ok(Json(
        json!({"success":true,"data":{"acknowledged":true,"semantics":"Client acknowledgement; not proof of human comprehension"}}),
    ))
}

#[derive(Deserialize, Default)]
pub struct TimelinePage {
    pub event_limit: Option<i64>,
    pub after_time: Option<chrono::DateTime<chrono::Utc>>,
    pub after_id: Option<Uuid>,
    pub after_source: Option<String>,
    pub snapshot_at: Option<chrono::DateTime<chrono::Utc>>,
    pub include_events: Option<bool>,
    pub include_records: Option<bool>,
    pub record_limit: Option<i64>,
    pub record_offset: Option<i64>,
}
pub async fn grievance_timeline(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(grievance): Path<Uuid>,
    Query(page): Query<TimelinePage>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ViewGrievanceQueue)?;
    let valid_cursor = matches!(
        (page.after_time, page.after_id, page.after_source.as_deref()),
        (None, None, None) | (Some(_), Some(_), Some("TRACE" | "AUDIT"))
    );
    if !valid_cursor {
        return Err(AppError::Validation(
            "Use a complete timeline cursor.".into(),
        ));
    }
    let mut tx = state.begin_authed(&ctx).await?;
    let case = grievance_repo::find_by_id(&mut *tx, grievance)
        .await?
        .ok_or_else(|| AppError::NotFound("Grievance not found.".into()))?;
    let appointment: Uuid =
        sqlx::query_scalar("SELECT appointment_id FROM consultation_sessions WHERE id=$1")
            .bind(case.consultation_id)
            .fetch_one(&mut *tx)
            .await?;
    consultation_trace::admin_read(
        &mut tx,
        &ctx,
        "ADMIN_GRIEVANCE_OPENED",
        "GRIEVANCE_REPORT",
        grievance,
    )
    .await?;
    consultation_trace::admin_read(
        &mut tx,
        &ctx,
        "ADMIN_CONSULTATION_HISTORY_VIEWED",
        "APPOINTMENT",
        appointment,
    )
    .await?;
    let facts:Value=sqlx::query_scalar("SELECT jsonb_build_object('appointment_id',a.id,'appointment_number',a.appointment_number,'consultation_id',cs.id,'doctor_id',a.doctor_id,'doctor_name',d.full_name,'patient_id',a.patient_id,'patient_name',p.full_name,'modality',a.modality,'status',a.status,'consultation_session_status',cs.status,'recovery_state',cs.recovery_state,'interruption_reason',CASE WHEN cs.recovery_state='NEEDS_REVIEW' THEN 'EMPTY_ROOM_GRACE_EXPIRED' ELSE NULL END,'empty_room_deadline',cs.empty_room_deadline,'patient_present',cs.patient_present,'doctor_present',cs.doctor_present,'patient_presence_sequence',cs.patient_presence_sequence,'doctor_presence_sequence',cs.doctor_presence_sequence,'start_time',s.start_time,'end_time',s.end_time,'transaction_id',t.id,'payment_status',t.payment_status,'conversation_id',c.id,'chat_expires_at',c.expires_at,'chat_locked',c.is_locked) FROM appointments a JOIN consultation_sessions cs ON cs.appointment_id=a.id JOIN doctor_profiles d ON d.id=a.doctor_id JOIN patient_profiles p ON p.id=a.patient_id JOIN doctor_schedule_slots s ON s.id=a.slot_id LEFT JOIN LATERAL (SELECT tx.* FROM transactions tx WHERE tx.appointment_id=a.id ORDER BY (tx.payment_status<>'FAILED') DESC,tx.created_at DESC,tx.id DESC LIMIT 1) t ON TRUE LEFT JOIN chat_conversations c ON c.appointment_id=a.id WHERE a.id=$1").bind(appointment).fetch_one(&mut *tx).await?;
    let snapshot: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT LEAST(COALESCE($1,clock_timestamp()),clock_timestamp())")
            .bind(page.snapshot_at)
            .fetch_one(&mut *tx)
            .await?;
    let limit = page.event_limit.unwrap_or(50).clamp(1, 100);
    let evidence=concat!("WITH evidence AS (SELECT e.id,e.event_time_server AS at,'TRACE'::text AS source,to_jsonb(e) AS payload FROM consultation_trace_events e WHERE e.appointment_id=$1 UNION ALL SELECT e.id,e.created_at AS at,'AUDIT'::text AS source,","(to_jsonb(e)-'before_data'-'after_data'-'reason')||jsonb_build_object('actor_user_id',e.actor_id,'auth_session_id',e.session_id,'trace_id',e.request_id)"," AS payload FROM audit_events e WHERE ","e.resource_id=$1 OR e.resource_id=$2 OR e.resource_id IN(SELECT id FROM prescriptions WHERE appointment_id=$1) OR e.resource_id IN(SELECT id FROM prescription_intake_documents WHERE appointment_id=$1) OR e.resource_id IN(SELECT id FROM chat_conversations WHERE appointment_id=$1) OR e.resource_id IN(SELECT m.id FROM chat_messages m JOIN chat_conversations c ON c.id=m.conversation_id WHERE c.appointment_id=$1) OR (e.action IN ('SESSION_REVOKED','AUTH_ROLE_MISMATCH') AND session_related_to_consultation(e.session_id,$1))",") ");
    let total_query = format!("{evidence}SELECT COUNT(*) FROM evidence WHERE at<=$3");
    let total: i64 = sqlx::query_scalar(&total_query)
        .bind(appointment)
        .bind(grievance)
        .bind(snapshot)
        .fetch_one(&mut *tx)
        .await?;
    let rows_query=format!("{evidence}SELECT jsonb_build_object('id',id,'at',at,'source',source,'payload',payload) FROM evidence WHERE at<=$3 AND ($4::TIMESTAMPTZ IS NULL OR (at,id,source)>($4,$5::UUID,$6::TEXT)) ORDER BY at,id,source LIMIT $7");
    let mut rows: Vec<Value> = if page.include_events.unwrap_or(true) {
        sqlx::query_scalar(&rows_query)
            .bind(appointment)
            .bind(grievance)
            .bind(snapshot)
            .bind(page.after_time)
            .bind(page.after_id)
            .bind(page.after_source.as_deref())
            .bind(limit + 1)
            .fetch_all(&mut *tx)
            .await?
    } else {
        Vec::new()
    };
    let has_more = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let next = rows
        .last()
        .map(|row| json!({"time":row["at"],"id":row["id"],"source":row["source"]}));
    let events: Vec<Value> = rows
        .iter()
        .filter(|row| row["source"] == "TRACE")
        .map(|row| row["payload"].clone())
        .collect();
    let audit: Vec<Value> = rows
        .iter()
        .filter(|row| row["source"] == "AUDIT")
        .map(|row| row["payload"].clone())
        .collect();
    let event_page = json!({"snapshot_at":snapshot,"total":total,"has_more":has_more,"next":next});
    let record_limit = page.record_limit.unwrap_or(50).clamp(1, 100);
    let record_offset = page.record_offset.unwrap_or(0).max(0);
    let mut records = json!({});
    let mut record_totals = json!({});
    if page.include_records.unwrap_or(true) {
        let sources=[
       ("messages","SELECT jsonb_build_object('id',m.id,'sender_id',m.sender_id,'sent_at',m.sent_at,'attachment',m.attachment_object_key IS NOT NULL) AS payload,m.sent_at AS at,m.id AS id FROM chat_messages m JOIN chat_conversations c ON c.id=m.conversation_id WHERE c.appointment_id=$1"),
       ("prescriptions","SELECT jsonb_build_object('id',r.id,'doctor_id',r.doctor_id,'created_at',r.created_at,'page_number',r.page_number) AS payload,r.created_at AS at,r.id AS id FROM prescriptions r WHERE r.appointment_id=$1"),
       ("documents","SELECT jsonb_build_object('id',d.id,'created_at',d.created_at,'patient_id',d.patient_id) AS payload,d.created_at AS at,d.id AS id FROM prescription_intake_documents d WHERE d.appointment_id=$1"),
       ("payments","SELECT to_jsonb(e)-'metadata' AS payload,e.created_at AS at,e.id AS id FROM payment_events e JOIN transactions t ON t.id=e.transaction_id WHERE t.appointment_id=$1"),
       ("reschedules","SELECT to_jsonb(r) AS payload,r.created_at AS at,r.id AS id FROM appointment_reschedules r WHERE r.appointment_id=$1"),
       ("adjudications","SELECT jsonb_build_object('id',g.id,'actor_user_id',g.adjudicated_by_admin_id,'action',g.action_type,'adjudicated_at',g.adjudicated_at,'board_remedy',g.board_remedy,'audit_notes',g.audit_notes) AS payload,g.adjudicated_at AS at,g.id AS id FROM grievance_adjudications g WHERE g.grievance_id=$2"),
       ("recovery_resolutions","SELECT jsonb_build_object('id',r.id,'session_id',r.session_id,'outcome',r.outcome,'reason',r.reason,'admin_user_id',r.admin_user_id,'created_at',r.created_at) AS payload,r.created_at AS at,r.id AS id FROM consultation_recovery_resolutions r WHERE r.session_id=(SELECT id FROM consultation_sessions WHERE appointment_id=$1)"),
       ("notifications","SELECT jsonb_build_object('id',n.id,'recipient_user_id',n.user_id,'notification_type',n.type,'created_at',n.created_at,'read_at',n.read_at,'related_entity_id',n.related_entity_id) AS payload,n.created_at AS at,n.id AS id FROM notifications n WHERE n.related_entity_id=$1 OR n.related_entity_id IN(SELECT id FROM chat_conversations WHERE appointment_id=$1)"),
       ("read_acknowledgements","SELECT to_jsonb(r) AS payload,r.first_acknowledged_at AS at,r.message_id AS id FROM chat_read_acknowledgements r JOIN chat_conversations c ON c.id=r.conversation_id WHERE c.appointment_id=$1"),
       ("correlated_error_references","SELECT jsonb_build_object('id',s.id,'trace_id',s.trace_id,'subsystem',s.subsystem,'component',s.component,'severity',s.severity,'logged_at',s.logged_at) AS payload,s.logged_at AS at,s.id AS id FROM system_error_logs s WHERE s.trace_id IN(SELECT request_id FROM consultation_trace_events WHERE appointment_id=$1) OR s.id IN(SELECT resource_id FROM consultation_trace_events WHERE appointment_id=$1 AND resource_type='SYSTEM_ERROR')"),
      ];
        for (kind, source) in sources {
            // All SQL fragments are compile-time literals; identities/cursors stay bound.
            let count_query=format!("WITH input AS(SELECT $1::UUID AS appointment,$2::UUID AS grievance), refs AS ({source}) SELECT COUNT(*) FROM refs WHERE at<=$3");
            let count: i64 = sqlx::query_scalar(&count_query)
                .bind(appointment)
                .bind(grievance)
                .bind(snapshot)
                .fetch_one(&mut *tx)
                .await?;
            let refs_query=format!("WITH input AS(SELECT $1::UUID AS appointment,$2::UUID AS grievance), refs AS ({source}) SELECT payload FROM refs WHERE at<=$3 ORDER BY at,id,payload::text LIMIT $4 OFFSET $5");
            let values: Vec<Value> = sqlx::query_scalar(&refs_query)
                .bind(appointment)
                .bind(grievance)
                .bind(snapshot)
                .bind(record_limit)
                .bind(record_offset)
                .fetch_all(&mut *tx)
                .await?;
            records[kind] = json!(values);
            record_totals[kind] = json!(count);
        }
    }
    let record_page = json!({"offset":record_offset,"limit":record_limit,"totals":record_totals});
    let holds: Vec<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(h) FROM consultation_trace_holds h WHERE appointment_id=$1",
    )
    .bind(appointment)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    // Existing telemetry/errors remain labelled by provenance; do not invent media proof.
    let telemetry = if page.include_records.unwrap_or(true) && record_offset == 0 {
        crate::services::ConsultationService::get_session_timeline(
            &state,
            &ctx,
            case.consultation_id,
        )
        .await?
    } else {
        Value::Null
    };
    Ok(Json(
        json!({"success":true,"data":{"grievance_id":grievance,"facts":facts,"trace_events":events,"audit_events":audit,"event_page":event_page,"record_page":record_page,"records":records,"preservation_holds":holds,"telemetry":telemetry,"semantics":"Server timestamps order accepted actions. Client acknowledgements do not prove human comprehension. Client telemetry is not provider media evidence."}}),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reschedule {
    pub slot_id: Uuid,
    pub reason: String,
    pub grievance_id: Option<Uuid>,
}
pub async fn reschedule(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(body): Json<Reschedule>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ManageSlots)?;
    let reason = crate::domain::validation::validate_text_field(&body.reason, "Reason", 300)?;
    if reason.chars().count() < 8 {
        return Err(AppError::Validation(
            "A reschedule reason of at least 8 characters is required.".into(),
        ));
    }
    let mut tx = state.begin_authed_as(&ctx, DbRole::PaymentWorker).await?;
    let old: (Uuid, Uuid, String) = sqlx::query_as(
        "SELECT slot_id,doctor_id,status::text FROM appointments WHERE id=$1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;
    if !["CONFIRMED", "WAITING"].contains(&old.2.as_str()) || old.0 == body.slot_id {
        return Err(AppError::Conflict(
            "This consultation cannot be rescheduled.".into(),
        ));
    }
    if let Some(grievance) = body.grievance_id {
        // This lookup is security-definer and constrained to this appointment.
        let valid: bool = sqlx::query_scalar("SELECT grievance_belongs_to_appointment($1,$2)")
            .bind(grievance)
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        if !valid {
            return Err(AppError::NotFound("Linked grievance not found.".into()));
        }
    }
    // INV-BOOK-02 on reschedule: the Patient must not end up with two
    // overlapping active appointments. The exclusion constraint is the
    // authority (changing slot_id re-syncs appointments.slot_period); these
    // steps keep it race-free. Serialize this Patient's booking activity first
    // (same key as booking), then free any *due* abandoned hold on the target
    // slot or overlapping this Patient's time, in appointment -> slot lock
    // order. A live hold still blocks, as it must.
    let patient: Uuid = sqlx::query_scalar("SELECT patient_id FROM appointments WHERE id=$1")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("patient-booking:{patient}"))
        .execute(&mut *tx)
        .await?;
    let window = state.config.payment_reservation_window_secs;
    appointment_repo::expire_abandoned_hold_for_slot(&mut tx, body.slot_id, window).await?;
    appointment_repo::expire_due_holds_overlapping_for_patient(
        &mut tx,
        patient,
        body.slot_id,
        window,
    )
    .await?;
    if !crate::repository::doctor_repo::try_lock_slot(&mut tx, body.slot_id, old.1.into()).await? {
        return Err(AppError::Conflict(
            "The replacement slot is no longer available.".into(),
        ));
    }
    let original: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT start_time FROM doctor_schedule_slots WHERE id=$1")
            .bind(old.0)
            .fetch_one(&mut *tx)
            .await?;
    let new_start: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT start_time FROM doctor_schedule_slots WHERE id=$1")
            .bind(body.slot_id)
            .fetch_one(&mut *tx)
            .await?;
    sqlx::query("UPDATE appointments SET slot_id=$2,status='CONFIRMED',updated_at=clock_timestamp() WHERE id=$1").bind(id).bind(body.slot_id).execute(&mut *tx).await?;
    sqlx::query("UPDATE doctor_schedule_slots SET status=CASE WHEN id=$1 THEN 'AVAILABLE'::slot_status_enum ELSE 'BOOKED'::slot_status_enum END WHERE id IN($1,$2)").bind(old.0).bind(body.slot_id).execute(&mut *tx).await?;
    // The T-15 reminder scheduler's dedup key embeds the slot's start_time,
    // so a reschedule naturally gets a fresh reminder created at the new time
    // on the next scheduler tick — but a reminder already created for the
    // *old* time before this reschedule would otherwise sit untouched and
    // still fire at the wrong instant. Cancel any such stale pending
    // reminder for both the Doctor and the Patient/guardian recipient now.
    notification_repo::cancel_pending_reminders_for_appointment(&mut *tx, id).await?;
    let change = Uuid::new_v4();
    crate::services::NotificationService::notify_booking_rescheduled(&mut tx, id, change).await?;
    sqlx::query("INSERT INTO appointment_reschedules(id,appointment_id,original_slot_id,new_slot_id,original_start,new_start,actor_user_id,auth_session_id,request_id,reason,grievance_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)").bind(change).bind(id).bind(old.0).bind(body.slot_id).bind(original).bind(new_start).bind(ctx.user_id).bind(ctx.session_id).bind(&ctx.request_id).bind(reason).bind(body.grievance_id).execute(&mut *tx).await?;
    consultation_trace::record(
        &mut tx,
        id,
        None,
        Event::accepted("RESCHEDULE_PERFORMED", Some(("RESCHEDULE", change))),
    )
    .await?;
    consultation_trace::admin_read(
        &mut tx,
        &ctx,
        "ADMIN_RESCHEDULED_APPOINTMENT",
        "APPOINTMENT",
        id,
    )
    .await?;

    // Phase 1B: atomic creation of the email intent alongside the business
    // event, on this same (payment-worker) transaction — see
    // `email_outbox_repo::enqueue_transactional`'s doc comment. A second
    // reschedule before the first one's email ever sent must not leave both
    // queued: `supersede_pending` marks any still-pending
    // CONFIRMED/RESCHEDULED row for this same appointment stale first, in
    // the same transaction as the new row's own insert.
    let recipient_email: Option<String> = sqlx::query_scalar::<_, Option<String>>(
        "SELECT COALESCE(pu.email, gu.email) \
           FROM appointments a \
           JOIN patient_profiles p ON p.id = a.patient_id \
           LEFT JOIN patient_profiles g ON g.id = p.guardian_patient_id \
           LEFT JOIN users pu ON pu.id = p.user_id \
           LEFT JOIN users gu ON gu.id = g.user_id \
          WHERE a.id = $1",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    let rescheduled_outbox_id =
        if let Some(recipient_email) = recipient_email.filter(|e| !e.is_empty()) {
            let outbox_id = Uuid::new_v4();
            let dedup_key = format!("consultation-rescheduled:{change}");
            email_outbox_repo::supersede_pending(
                &mut *tx,
                &["CONSULTATION_CONFIRMED", "CONSULTATION_RESCHEDULED"],
                "APPOINTMENT",
                id,
                &dedup_key,
            )
            .await?;
            email_outbox_repo::enqueue_transactional(
                &mut *tx,
                outbox_id,
                "CONSULTATION_RESCHEDULED",
                &recipient_email,
                &dedup_key,
                "APPOINTMENT_RESCHEDULE",
                change,
                email_outbox_repo::transactional_email_expiry(),
            )
            .await?;
            Some(outbox_id)
        } else {
            None
        };
    tx.commit().await?;

    // Fire-and-forget, after commit (never awaited — must not delay this
    // already-committed response). The durable fallback is the periodic
    // scheduler pass.
    if let Some(outbox_id) = rescheduled_outbox_id {
        let state = state.clone();
        tokio::spawn(async move {
            crate::services::email_dispatch_worker::dispatch_now(&state, outbox_id).await;
        });
    }

    Ok(Json(
        json!({"success":true,"data":{"appointment_id":id,"reschedule_id":change,"start_time":new_start}}),
    ))
}
