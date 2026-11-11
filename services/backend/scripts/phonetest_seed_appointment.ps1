<#
  Physical-device Agora consultation test: seeds a fresh, immediately-joinable
  Patient<->Doctor video appointment against the disposable phone-test backend
  started via docker-compose.phonetest.yml, using only real HTTP calls against
  already-implemented endpoints (no direct DB writes, no dev-only client
  bypasses). Safe to re-run any time to get a brand new near-term slot +
  appointment before a repeat test.

  Relies on src/seed.rs's fixed demo fixtures (SEED_DEV_DATA=true):
    - Demo patient: phone +254700000002 (no password; phone+OTP login)
    - Doctor 1:     sabrina.akter@hellodoctor.health / Password123! (password + email-OTP MFA)
  Both logins use the real OTP/MFA flow; in ENVIRONMENT=development with
  DEV_AUTH_OTP_ENABLED=true the backend returns the plaintext code in-band
  (`debug_otp_code`) instead of actually dispatching SMS/email, precisely so
  this kind of local/device testing doesn't need a real SMS/email gateway.

  Payment is confirmed the same way a real gateway webhook would: this script
  computes the HMAC-SHA256 signature over the exact webhook body using
  WEBHOOK_HMAC_SECRET read locally from .env (never printed), matching
  handle_payment_webhook's real signature verification — no payment-flow code
  is bypassed or weakened.

  Usage: ./phonetest_seed_appointment.ps1 [-BaseUrl http://127.0.0.1:8380] [-LeadMinutes 8] [-DurationMinutes 20]
#>
param(
    [string]$BaseUrl = "http://127.0.0.1:8380",
    [int]$LeadMinutes = 8,
    [int]$DurationMinutes = 20
)

$ErrorActionPreference = "Stop"
$Api = "$BaseUrl/api/v1"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$BackendDir = Split-Path -Parent $ScriptDir
$EnvFile = Join-Path $BackendDir ".env"

function Read-EnvValue($name) {
    $line = Get-Content $EnvFile | Where-Object { $_ -match "^$name=" } | Select-Object -First 1
    if (-not $line) { throw "Could not find $name in $EnvFile" }
    $value = $line -replace "^$name=", ''
    return $value.Trim().Trim('"')
}

function Invoke-Api {
    param([string]$Method, [string]$Path, [hashtable]$Body = $null, [string]$Token = $null)
    $headers = @{}
    if ($Token) { $headers["Authorization"] = "Bearer $Token" }
    $uri = "$Api$Path"
    if ($Body -ne $null) {
        $json = $Body | ConvertTo-Json -Depth 10 -Compress
        return Invoke-RestMethod -Method $Method -Uri $uri -Headers $headers -ContentType "application/json" -Body $json
    }
    return Invoke-RestMethod -Method $Method -Uri $uri -Headers $headers
}

Write-Host "=== 1. Doctor login (sabrina.akter@hellodoctor.health) ===" -ForegroundColor Cyan
$docStep1 = Invoke-Api -Method Post -Path "/auth/doctor/login" -Body @{ email = "sabrina.akter@hellodoctor.health"; password = "Password123!" }
$docOtp = $docStep1.data.debug_otp_code
if (-not $docOtp) { throw "No debug_otp_code returned for doctor login - is DEV_AUTH_OTP_ENABLED=true?" }
$docVerify = Invoke-Api -Method Post -Path "/auth/doctor/verify-otp" -Body @{ session_token = $docStep1.data.session_token; otp_code = $docOtp }
$doctorToken = $docVerify.data.access_token
Write-Host "  Doctor authenticated. user_id=$($docVerify.data.user.id)" -ForegroundColor Green

Write-Host "=== 2. Clear any unbooked slots left by a previous test run ===" -ForegroundColor Cyan
$existingSlots = (Invoke-Api -Method Get -Path "/doctor/slots" -Token $doctorToken).data
foreach ($s in $existingSlots) {
    if ($s.status -eq "AVAILABLE" -and -not $s.appointment_id -and [datetime]$s.start_time -gt (Get-Date).ToUniversalTime()) {
        try { Invoke-Api -Method Delete -Path "/doctor/slots/$($s.id)" -Token $doctorToken | Out-Null } catch {}
    }
}

Write-Host "=== 3. Create a near-term schedule slot (starts in $LeadMinutes min) ===" -ForegroundColor Cyan
$iso = "yyyy-MM-ddTHH:mm:ss.fffZ"
$slotStart = (Get-Date).ToUniversalTime().AddMinutes($LeadMinutes)
$slotEnd = $slotStart.AddMinutes($DurationMinutes)
$slotResp = Invoke-Api -Method Post -Path "/doctor/slots" -Token $doctorToken -Body @{
    start_time = $slotStart.ToString($iso)
    end_time   = $slotEnd.ToString($iso)
}
$slotId = $slotResp.data.id
if (-not $slotId) { $slotId = $slotResp.data.slot_id }
Write-Host "  Slot created: $slotId  ($($slotStart.ToString('u')) - $($slotEnd.ToString('u')))" -ForegroundColor Green

Write-Host "=== 3. Patient OTP login (+254700000002) ===" -ForegroundColor Cyan
$patOtpReq = Invoke-Api -Method Post -Path "/auth/register-otp" -Body @{ phone_number = "+254700000002" }
$patOtp = $patOtpReq.data.debug_otp_code
if (-not $patOtp) { throw "No debug_otp_code returned for patient OTP request." }
$patLogin = Invoke-Api -Method Post -Path "/auth/verify-otp-and-login" -Body @{
    phone_number = "+254700000002"; otp_code = $patOtp; accepted_documents = @()
}
$patientToken = $patLogin.data.access_token
Write-Host "  Patient authenticated. user_id=$($patLogin.data.user.id)" -ForegroundColor Green

Write-Host "=== 4. Book the appointment (as patient) ===" -ForegroundColor Cyan
$doctorProfileId = "da000001-0000-0000-0000-000000000001"
$booking = Invoke-Api -Method Post -Path "/appointments/book" -Token $patientToken -Body @{
    doctor_id       = $doctorProfileId
    slot_id         = $slotId
    modality        = "VIDEO"
    gateway         = "MPESA"
    chief_complaint = "Synthetic physical-device test consultation - no real clinical content."
}
$appointmentId = $booking.data.appointment_id
$paymentSessionId = $booking.data.payment_session_id
Write-Host "  Appointment $appointmentId booked, status PENDING_PAYMENT. payment_session_id=$paymentSessionId" -ForegroundColor Green

Write-Host "=== 5. Confirm payment via signed webhook (dev-mode HMAC, same as a real gateway callback) ===" -ForegroundColor Cyan
$secret = Read-EnvValue "WEBHOOK_HMAC_SECRET"
$eventTime = (Get-Date).ToUniversalTime().ToString($iso)
$txnRef = "TEST-TXN-$([guid]::NewGuid())"
$eventId = "TEST-EVT-$([guid]::NewGuid())"
# Field order matches PaymentWebhookPayload's declaration; raw byte-for-byte
# body is what's signed and what's sent, so no server-side/client-side JSON
# re-serialization can ever disagree with the signature.
$bodyObj = [ordered]@{
    payment_session_id    = $paymentSessionId
    transaction_reference = $txnRef
    status                = "SUCCESS"
    event_timestamp       = $eventTime
    provider_event_id     = $eventId
}
$rawBody = $bodyObj | ConvertTo-Json -Compress
$hmac = [System.Security.Cryptography.HMACSHA256]::new()
$hmac.Key = [System.Text.Encoding]::UTF8.GetBytes($secret)
$sigBytes = $hmac.ComputeHash([System.Text.Encoding]::UTF8.GetBytes($rawBody))
$signature = ($sigBytes | ForEach-Object { $_.ToString("x2") }) -join ''
$webhookResp = Invoke-RestMethod -Method Post -Uri "$Api/webhooks/payment/mpesa" -Headers @{ "X-Signature" = $signature } -ContentType "application/json" -Body $rawBody
Write-Host "  Webhook accepted. Appointment status now: $($webhookResp.data.status)" -ForegroundColor Green

Write-Host "=== 6. Verify appointment is CONFIRMED and Agora tokens mint for both participants ===" -ForegroundColor Cyan
$aptAsPatient = Invoke-Api -Method Get -Path "/appointments/$appointmentId" -Token $patientToken
if ($aptAsPatient.data.status -ne "CONFIRMED") {
    throw "Expected CONFIRMED, got $($aptAsPatient.data.status)"
}
$patientRtc = Invoke-Api -Method Post -Path "/consultations/$appointmentId/rtc-token" -Token $patientToken
$doctorRtc = Invoke-Api -Method Post -Path "/consultations/$appointmentId/rtc-token" -Token $doctorToken
if ($patientRtc.data.channel_name -ne $doctorRtc.data.channel_name) {
    throw "Patient and doctor were assigned different Agora channels - this would be a real bug."
}
Write-Host "  Agora channel: $($patientRtc.data.channel_name)  app_id=$($patientRtc.data.app_id)" -ForegroundColor Green
Write-Host "  Patient uid=$($patientRtc.data.uid)  Doctor uid=$($doctorRtc.data.uid)  expires_in=$($patientRtc.data.expires_in)s" -ForegroundColor Green

Write-Host "=== 7. Negative check: an unrelated account must NOT be able to mint a token ===" -ForegroundColor Cyan
$adminStep1 = Invoke-Api -Method Post -Path "/auth/admin/login" -Body @{ email = "admin@hellodoctor.health"; password = "AdminPass123!" }
$adminOtp = $adminStep1.data.debug_otp_code
$adminVerify = Invoke-Api -Method Post -Path "/auth/admin/verify-otp" -Body @{ session_token = $adminStep1.data.session_token; otp_code = $adminOtp }
$adminToken = $adminVerify.data.access_token
try {
    Invoke-Api -Method Post -Path "/consultations/$appointmentId/rtc-token" -Token $adminToken | Out-Null
    Write-Host "  UNEXPECTED: unrelated account was able to mint a token!" -ForegroundColor Red
} catch {
    Write-Host "  Correctly rejected (unrelated account cannot join): $($_.Exception.Response.StatusCode.value__)" -ForegroundColor Green
}

Write-Host "=== 8. Chat: confirm a message round-trips on this appointment's conversation ===" -ForegroundColor Cyan
$conv = Invoke-Api -Method Get -Path "/appointments/$appointmentId/conversation" -Token $patientToken
$conversationId = $conv.data.id
if (-not $conversationId) { $conversationId = $conv.data.conversation_id }
Invoke-Api -Method Post -Path "/chat/$conversationId/messages" -Token $patientToken -Body @{ content = "Hello doctor - synthetic test message." } | Out-Null
$messages = Invoke-Api -Method Get -Path "/chat/$conversationId/messages" -Token $doctorToken
Write-Host "  Doctor sees $($messages.data.Count) message(s) in conversation $conversationId" -ForegroundColor Green

$result = [ordered]@{
    appointment_id   = $appointmentId
    conversation_id  = $conversationId
    doctor_profile_id = $doctorProfileId
    slot_id          = $slotId
    slot_start_utc   = $slotStart.ToString('o')
    slot_end_utc     = $slotEnd.ToString('o')
    earliest_join_utc = $slotStart.AddMinutes(-15).ToString('o')
    latest_join_utc  = $slotEnd.AddMinutes(60).ToString('o')
    agora_channel    = $patientRtc.data.channel_name
    patient_uid      = $patientRtc.data.uid
    doctor_uid       = $doctorRtc.data.uid
}
Write-Host ""
Write-Host "=== DONE: appointment ready for physical-device join ===" -ForegroundColor Cyan
$result | ConvertTo-Json
