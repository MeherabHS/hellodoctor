<#
  Non-destructive diagnostics collection for the physical-device Agora test.
  Read-only: tails the disposable phonetest backend's container logs and
  redacts anything that looks like a token, secret, password or Authorization
  header before printing. Never prints the Agora App Certificate, webhook/JWT
  secrets, or raw clinical text (chief_complaint values are never logged by
  the backend itself; this is defense-in-depth in case a future log line
  changes that).

  Usage: ./phonetest_diagnostics.ps1 [-Lines 300]
#>
param([int]$Lines = 300)

$ErrorActionPreference = "Stop"

function Redact($line) {
    $line = $line -replace '(?i)(Bearer\s+)[A-Za-z0-9\-_\.]+', '$1[REDACTED]'
    $line = $line -replace '(?i)("?(access_token|refresh_token|token|password|otp_code|debug_otp_code|app_certificate|secret|x-signature|authorization)"?\s*[:=]\s*")[^"]+(")', '${1}[REDACTED]${3}'
    return $line
}

Write-Host "=== Backend container logs (hellodoctor-phonetest-backend, last $Lines lines, redacted) ===" -ForegroundColor Cyan
docker logs --tail $Lines hellodoctor-phonetest-backend 2>&1 | ForEach-Object { Redact $_ }

Write-Host ""
Write-Host "=== Container health ===" -ForegroundColor Cyan
docker ps --filter "name=hellodoctor-phonetest" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"

Write-Host ""
Write-Host "=== Schema verification (source == binary == database) ===" -ForegroundColor Cyan
& (Join-Path $PSScriptRoot "verify_backend_schema.ps1") -BaseUrl "http://127.0.0.1:8380"

Write-Host ""
Write-Host "=== Flutter-side diagnostics (run these yourself, per device) ===" -ForegroundColor Cyan
Write-Host "  Doctor phone (adb):   adb -s <doctor-device-serial> logcat -s flutter:V *:S"
Write-Host "  Patient phone (adb):  adb -s <patient-device-serial> logcat -s flutter:V *:S"
Write-Host "  List connected devices: adb devices -l"
Write-Host ""
Write-Host "Reminder: Agora join/leave server-side presence events arrive at" -ForegroundColor Yellow
Write-Host "POST /webhooks/agora/presence (HMAC-verified) and are reflected in the" -ForegroundColor Yellow
Write-Host "consultation session's patient_present/doctor_present fields - visible" -ForegroundColor Yellow
Write-Host "to the doctor app via GET /appointments/:id/consultation." -ForegroundColor Yellow
