# Test integrity audit

Modified existing test files: 8. Intentional assertion replacements: 23.

No removed existing test functions or added skip/ignore markers were detected. The private-profile cutoff remains exact zero.

## Existing test files

- `services/backend/tests/api_tests.rs`
- `apps/mobile/test/doctor_models_test.dart`
- `apps/mobile/test/doctor_navigation_safe_area_test.dart`
- `apps/mobile/test/doctor_widgets_test.dart`
- `apps/mobile/test/pre_e2e_queue_chat_test.dart`
- `apps/mobile_patient/test/consultation_booking_test.dart`
- `apps/mobile_patient/test/consultation_flow_test.dart`
- `apps/mobile_patient/test/pre_e2e_availability_payment_test.dart`

## Intentional assertion replacements

### 1. test_identity_doctor_private_financial_and_treatment_context_rls

File: `services/backend/tests/api_tests.rs` (frozen baseline line 1787).

Old:
```
assert_eq!(
        count, 0,
        "assigned Doctor outside approved window must not read PHI"
    )
```

New:
```
assert_eq!(
        count, 1,
        "assigned Doctor retains their own encounter intake without broader profile access"
    )
```

Approved retained own-encounter intake reads. The original private-profile cutoff and foreign/private/financial isolation assertions remain unchanged.

### 2. test_identity_compliance_trace_scoped_authorized_and_audited

File: `services/backend/tests/api_tests.rs` (frozen baseline line 2071).

Old:
```
assert_eq!(
            post(
                &f.app,
                "/api/v1/admin/compliance/trace",
                Some(&token),
                &request
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        )
```

New:
```
assert_eq!(
            post(
                &f.app,
                "/api/v1/admin/compliance/trace",
                Some(&token),
                &request
            )
            .await
            .status(),
            if ["PATIENT", "DOCTOR"].contains(
                &hellodoctor_backend::crypto::verify_access_token(&token, TEST_JWT_PUBLIC_KEY_B64)
                    .unwrap()
                    .role
                    .as_str()
            ) {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::UNAUTHORIZED
            }
        )
```

Approved canonical ADMIN role: exact active role/audit identity becomes ADMIN, retired claims are rejected with exact 401, and Patient/Doctor remain exact 403. All-Admin privileges are intentional; durable denial/account-attribution checks are preserved or strengthened.

### 3. test_identity_compliance_trace_scoped_authorized_and_audited

File: `services/backend/tests/api_tests.rs` (frozen baseline line 2118).

Old:
```
assert_eq!(row.get::<String, _>("role"), "COMPLIANCE")
```

New:
```
assert_eq!(row.get::<String, _>("role"), "ADMIN")
```

Approved canonical ADMIN role: exact active role/audit identity becomes ADMIN, retired claims are rejected with exact 401, and Patient/Doctor remain exact 403. All-Admin privileges are intentional; durable denial/account-attribution checks are preserved or strengthened.

### 4. test_doctor_rx_pages_encrypted_download_retry_and_expired_grant

File: `services/backend/tests/api_tests.rs` (frozen baseline line 3260).

Old:
```
assert_eq!(
        get(
            &app,
            &document_uri,
            Some(&token_for_real_role(&pool, "FINANCE_ADMIN").await)
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    )
```

New:
```
assert_eq!(
        get(
            &app,
            &document_uri,
            Some(&token_for_real_role(&pool, "ADMIN").await)
        )
        .await
        .status(),
        StatusCode::OK
    )
assert_eq!(
        send(&app, "GET", download_url, None, None, Some(&download_key))
            .await
            .status(),
        StatusCode::OK,
        "a valid grant preserves authorized own-encounter reads after chat lock"
    )
assert_eq!(
        get(&app, &document_uri, Some(&doctor)).await.status(),
        StatusCode::OK
    )
assert_eq!(expired, 1)
assert_eq!(
        send(&app, "GET", download_url, None, None, Some(&download_key))
            .await
            .status(),
        StatusCode::NOT_FOUND
    )
assert!(legacy["session_id"].is_string())
assert_eq!(
        send(
            &app,
            "GET",
            &format!("/api/v1/documents/download?token={legacy_token}"),
            None,
            None,
            Some(&Uuid::new_v4().to_string())
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    )
assert_eq!(
        send(
            &app,
            "GET",
            fresh_url,
            None,
            None,
            Some(&Uuid::new_v4().to_string())
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    )
```

Approved ADMIN access and retained own-encounter Rx reads. Independent download expiry, legacy no-session denial and session-revocation checks were added; foreign Doctor denial remains.

### 5. test_doctor_rx_pages_encrypted_download_retry_and_expired_grant

File: `services/backend/tests/api_tests.rs` (frozen baseline line 3277).

Old:
```
assert_eq!(
        send(&app, "GET", download_url, None, None, Some(&download_key))
            .await
            .status(),
        StatusCode::NOT_FOUND,
        "an unexpired grant cannot bypass an expired clinical window"
    )
```

New:
```
assert_eq!(
        get(
            &app,
            &document_uri,
            Some(&token_for_real_role(&pool, "ADMIN").await)
        )
        .await
        .status(),
        StatusCode::OK
    )
assert_eq!(
        send(&app, "GET", download_url, None, None, Some(&download_key))
            .await
            .status(),
        StatusCode::OK,
        "a valid grant preserves authorized own-encounter reads after chat lock"
    )
assert_eq!(
        get(&app, &document_uri, Some(&doctor)).await.status(),
        StatusCode::OK
    )
assert_eq!(expired, 1)
assert_eq!(
        send(&app, "GET", download_url, None, None, Some(&download_key))
            .await
            .status(),
        StatusCode::NOT_FOUND
    )
assert!(legacy["session_id"].is_string())
assert_eq!(
        send(
            &app,
            "GET",
            &format!("/api/v1/documents/download?token={legacy_token}"),
            None,
            None,
            Some(&Uuid::new_v4().to_string())
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    )
assert_eq!(
        send(
            &app,
            "GET",
            fresh_url,
            None,
            None,
            Some(&Uuid::new_v4().to_string())
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    )
```

Approved ADMIN access and retained own-encounter Rx reads. Independent download expiry, legacy no-session denial and session-revocation checks were added; foreign Doctor denial remains.

### 6. test_doctor_rx_pages_encrypted_download_retry_and_expired_grant

File: `services/backend/tests/api_tests.rs` (frozen baseline line 3284).

Old:
```
assert_eq!(
        get(&app, &document_uri, Some(&doctor)).await.status(),
        StatusCode::NOT_FOUND
    )
```

New:
```
assert_eq!(
        get(
            &app,
            &document_uri,
            Some(&token_for_real_role(&pool, "ADMIN").await)
        )
        .await
        .status(),
        StatusCode::OK
    )
assert_eq!(
        send(&app, "GET", download_url, None, None, Some(&download_key))
            .await
            .status(),
        StatusCode::OK,
        "a valid grant preserves authorized own-encounter reads after chat lock"
    )
assert_eq!(
        get(&app, &document_uri, Some(&doctor)).await.status(),
        StatusCode::OK
    )
assert_eq!(expired, 1)
assert_eq!(
        send(&app, "GET", download_url, None, None, Some(&download_key))
            .await
            .status(),
        StatusCode::NOT_FOUND
    )
assert!(legacy["session_id"].is_string())
assert_eq!(
        send(
            &app,
            "GET",
            &format!("/api/v1/documents/download?token={legacy_token}"),
            None,
            None,
            Some(&Uuid::new_v4().to_string())
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    )
assert_eq!(
        send(
            &app,
            "GET",
            fresh_url,
            None,
            None,
            Some(&Uuid::new_v4().to_string())
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    )
```

Approved ADMIN access and retained own-encounter Rx reads. Independent download expiry, legacy no-session denial and session-revocation checks were added; foreign Doctor denial remains.

### 7. test_admin_login_with_real_totp

File: `services/backend/tests/api_tests.rs` (frozen baseline line 4171).

Old:
```
assert_eq!(
        json["data"]["user"]["role"].as_str().unwrap(),
        "PLATFORM_ADMIN"
    )
```

New:
```
assert_eq!(json["data"]["user"]["role"].as_str().unwrap(), "ADMIN")
```

Approved canonical ADMIN role: exact active role/audit identity becomes ADMIN, retired claims are rejected with exact 401, and Patient/Doctor remain exact 403. All-Admin privileges are intentional; durable denial/account-attribution checks are preserved or strengthened.

### 8. test_admin_mutation_creates_audit_event

File: `services/backend/tests/api_tests.rs` (frozen baseline line 5802).

Old:
```
assert_eq!(row.get::<String, _>("actor_role"), "PLATFORM_ADMIN")
```

New:
```
assert_eq!(row.get::<String, _>("actor_role"), "ADMIN")
```

Approved canonical ADMIN role: exact active role/audit identity becomes ADMIN, retired claims are rejected with exact 401, and Patient/Doctor remain exact 403. All-Admin privileges are intentional; durable denial/account-attribution checks are preserved or strengthened.

### 9. test_audit_before_after_snapshots_and_super_admin_only_log_access

File: `services/backend/tests/api_tests.rs` (frozen baseline line 6334).

Old:
```
assert_eq!(res_denied.status(), StatusCode::FORBIDDEN)
```

New:
```
assert_eq!(res_denied.status(), StatusCode::OK)
assert_eq!(
        get(
            &app,
            "/api/v1/admin/logs",
            Some(&make_token(uid(PATIENT_USER_ID), "PATIENT"))
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    )
```

Approved canonical ADMIN role: exact active role/audit identity becomes ADMIN, retired claims are rejected with exact 401, and Patient/Doctor remain exact 403. All-Admin privileges are intentional; durable denial/account-attribution checks are preserved or strengthened.

### 10. test_doctor_history_window_pagination_and_cutoff

File: `services/backend/tests/api_tests.rs` (frozen baseline line 9141).

Old:
```
assert_eq!(
        get(&app, &path, Some(&doctor)).await.status(),
        StatusCode::NOT_FOUND
    )
```

New:
```
assert_eq!(
            get(&app, &path, Some(&other)).await.status(),
            StatusCode::NOT_FOUND
        )
assert_eq!(
            get(
                &app,
                &format!("/api/v1/doctor/consultations/{apt}/summary"),
                Some(&doctor)
            )
            .await
            .status(),
            StatusCode::OK
        )
assert_eq!(
        get(&app, &path, Some(&doctor)).await.status(),
        StatusCode::OK
    )
```

Approved retained own-encounter history after 24 hours, with unrelated-Doctor denials retained and summary reads added.

### 11. test_admin_system_error_simulation_environment_abac_and_persistence

File: `services/backend/tests/api_tests.rs` (frozen baseline line 9970).

Old:
```
assert_eq!(
            post(&app, uri, Some(&token), &payload).await.status(),
            StatusCode::FORBIDDEN,
            "{role}"
        )
```

New:
```
assert_eq!(
            post(&app, uri, Some(&token), &payload).await.status(),
            if ["PATIENT", "DOCTOR"].contains(&role) {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::UNAUTHORIZED
            },
            "{role}"
        )
```

Approved canonical ADMIN role: exact active role/audit identity becomes ADMIN, retired claims are rejected with exact 401, and Patient/Doctor remain exact 403. All-Admin privileges are intentional; durable denial/account-attribution checks are preserved or strengthened.

### 12. test_admin_compliance_alerts_abac_and_real_prescription_data

File: `services/backend/tests/api_tests.rs` (frozen baseline line 10089).

Old:
```
assert_eq!(
            get(&app, uri, Some(token)).await.status(),
            StatusCode::FORBIDDEN
        )
```

New:
```
assert_eq!(
            get(&app, uri, Some(token)).await.status(),
            if ["PATIENT", "DOCTOR"].contains(
                &hellodoctor_backend::crypto::verify_access_token(token, TEST_JWT_PUBLIC_KEY_B64)
                    .unwrap()
                    .role
                    .as_str()
            ) {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::UNAUTHORIZED
            }
        )
```

Approved canonical ADMIN role: exact active role/audit identity becomes ADMIN, retired claims are rejected with exact 401, and Patient/Doctor remain exact 403. All-Admin privileges are intentional; durable denial/account-attribution checks are preserved or strengthened.

### 13. test_admin_audit_log_simulation_abac_and_persistence

File: `services/backend/tests/api_tests.rs` (frozen baseline line 10175).

Old:
```
assert_eq!(
            post(&app, uri, Some(&token), &payload).await.status(),
            StatusCode::FORBIDDEN,
            "{role}"
        )
```

New:
```
assert_eq!(
            post(&app, uri, Some(&token), &payload).await.status(),
            if ["PATIENT", "DOCTOR"].contains(&role) {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::UNAUTHORIZED
            },
            "{role}"
        )
```

Approved canonical ADMIN role: exact active role/audit identity becomes ADMIN, retired claims are rejected with exact 401, and Patient/Doctor remain exact 403. All-Admin privileges are intentional; durable denial/account-attribution checks are preserved or strengthened.

### 14. test_admin_professional_verification_queue_pending_only_and_abac

File: `services/backend/tests/api_tests.rs` (frozen baseline line 10261).

Old:
```
assert_eq!(
            get(&app, uri, Some(&token)).await.status(),
            StatusCode::FORBIDDEN,
            "{role}"
        )
```

New:
```
assert_eq!(
            get(&app, uri, Some(&token)).await.status(),
            if ["PATIENT", "DOCTOR"].contains(&role) {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::UNAUTHORIZED
            },
            "{role}"
        )
```

Approved default Queue contains current/today only. Explicit All retains exact aggregate row count and ordering; Schedule and History receive exact separate assertions.

### 15. test_professional_review_denies_self_cross_doctor_patient_and_other_admin_roles

File: `services/backend/tests/api_tests.rs` (frozen baseline line 11488).

Old:
```
assert_eq!(
                    post(&app, &uri, Some(&token), &serde_json::json!({}))
                        .await
                        .status(),
                    StatusCode::FORBIDDEN,
                    "{role} {id} {action}"
                )
```

New:
```
assert_eq!(
                    post(&app, &uri, Some(&token), &serde_json::json!({}))
                        .await
                        .status(),
                    if ["PATIENT", "DOCTOR"].contains(&role) {
                        StatusCode::FORBIDDEN
                    } else {
                        StatusCode::UNAUTHORIZED
                    },
                    "{role} {id} {action}"
                )
```

Approved canonical ADMIN role: exact active role/audit identity becomes ADMIN, retired claims are rejected with exact 401, and Patient/Doctor remain exact 403. All-Admin privileges are intentional; durable denial/account-attribution checks are preserved or strengthened.

### 16. test_exposure_production_metrics_and_simulation_have_independent_backend_guards

File: `services/backend/tests/api_tests.rs` (frozen baseline line 12276).

Old:
```
assert_eq!(
        through_tls_proxy(
            &app,
            "GET",
            "/api/v1/admin/overview",
            Some(&make_token(uid(ADMIN_USER_ID), "PLATFORM_ADMIN")),
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    )
```

New:
```
assert_eq!(
        through_tls_proxy(
            &app,
            "GET",
            "/api/v1/admin/overview",
            Some(&make_token(uid(ADMIN_USER_ID), "ADMIN")),
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    )
```

Approved canonical ADMIN role: exact active role/audit identity becomes ADMIN, retired claims are rejected with exact 401, and Patient/Doctor remain exact 403. All-Admin privileges are intentional; durable denial/account-attribution checks are preserved or strengthened.

### 17. test_pre_e2e_queue_excludes_stale_and_terminal_but_preserves_history

File: `services/backend/tests/api_tests.rs` (frozen baseline line 14944).

Old:
```
assert_eq!(
        ids,
        vec![near.to_string(), later.to_string(), tomorrow.to_string()]
    )
```

New:
```
assert_eq!(ids, vec![near.to_string(), later.to_string()])
```

Approved default Queue contains current/today only. Explicit All retains exact aggregate row count and ordering; Schedule and History receive exact separate assertions.

### 18. test_pre_e2e_queue_excludes_stale_and_terminal_but_preserves_history

File: `services/backend/tests/api_tests.rs` (frozen baseline line 14985).

Old:
```
assert_eq!(queue["data"].as_array().unwrap().len(), 4)
```

New:
```
assert_eq!(queue["data"].as_array().unwrap().len(), 3)
```

Approved default Queue contains current/today only. Explicit All retains exact aggregate row count and ordering; Schedule and History receive exact separate assertions.

### 19. default queue uses server eligibility and preserves completed history

File: `apps/mobile/test/pre_e2e_queue_chat_test.dart` (frozen baseline line 167).

Old:
```
expect(tester.getTopLeft(find.text('Later')).dy,
        lessThan(tester.getTopLeft(find.text('Tomorrow')).dy))
```

New:
```
expect(find.text('Later'), findsOneWidget)
expect(find.text('Tomorrow'), findsNothing)
expect(find.textContaining('Completed ('), findsNothing)
```

Approved current/today operational Queue and separate History; tomorrow is asserted absent in Queue and history remains available separately.

### 20. default queue uses server eligibility and preserves completed history

File: `apps/mobile/test/pre_e2e_queue_chat_test.dart` (frozen baseline line 169).

Old:
```
expect(find.text('Tomorrow'), findsOneWidget)
```

New:
```
expect(find.text('Later'), findsOneWidget)
expect(find.text('Tomorrow'), findsNothing)
expect(find.textContaining('Completed ('), findsNothing)
```

Approved current/today operational Queue and separate History; tomorrow is asserted absent in Queue and history remains available separately.

### 21. $payment renders server truth and preserves narrow layout

File: `apps/mobile_patient/test/consultation_booking_test.dart` (frozen baseline line 114).

Old:
```
expect(find.text('Proceed to payment'),
          payment == 'PAYMENT_HELD' ? findsNothing : findsOneWidget)
```

New:
```
expect(find.text('Proceed to payment method'),
          payment == 'PAYMENT_HELD' ? findsNothing : findsOneWidget)
```

Approved exact CTA wording: Proceed to payment method. Enabled/disabled state, backend state and financial checks remain.

### 22. paid CHAT chooser through booking, payment polling and real repository chat at ${width}dp

File: `apps/mobile_patient/test/consultation_flow_test.dart` (frozen baseline line 157).

Old:
```
expect(
          tester
              .widget<FilledButton>(
                  find.widgetWithText(FilledButton, 'Proceed to payment'))
              .onPressed,
          isNotNull)
```

New:
```
expect(
          tester
              .widget<FilledButton>(find.widgetWithText(
                  FilledButton, 'Proceed to payment method'))
              .onPressed,
          isNotNull)
```

Approved exact CTA wording: Proceed to payment method. Enabled/disabled state, backend state and financial checks remain.

### 23. unpaid consultation has one payment CTA, no fake success, and safe layout at $width

File: `apps/mobile_patient/test/pre_e2e_availability_payment_test.dart` (frozen baseline line 173).

Old:
```
expect(find.text('Proceed to payment'), findsOneWidget)
```

New:
```
expect(find.text('Proceed to payment method'), findsOneWidget)
```

Approved exact CTA wording: Proceed to payment method. Enabled/disabled state, backend state and financial checks remain.

## Changes to expected-value tables and request surfaces

- test_doctor_history_window_pagination_and_cutoff: own-encounter list reads at +16 min and at/after 1440 min completion now return exact 200; unrelated Doctor still exact 404; own summary after cutoff is additionally exact 200.
- test_doctor_queue_shows_upcoming_consultations_first: old +3-day fixtures remain unchanged; original exact All ordering/pagination checks now explicitly request view=all, with new default Queue absence and exact Schedule/History assertions.
- test_pre_e2e_queue_excludes_stale_and_terminal_but_preserves_history: default Queue 3-to-2 and active Queue 4-to-3 intentionally move tomorrow into a separately asserted one-row Schedule. All retains exact five rows and historical ordering.
- test_identity_compliance_trace_scoped_authorized_and_audited: four durable denials remain required; the query includes two AUTH_ROLE_MISMATCH audit rows for retired claims, with a new exact two-row/session/request/account-role assertion.
