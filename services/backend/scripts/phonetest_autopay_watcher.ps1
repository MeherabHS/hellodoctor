<#
  Auto-confirms payment for ANY appointment booked through the real app UI
  during physical-device testing, so testers never have to wait on a real
  payment gateway. Polls the disposable phonetest Postgres for INITIATED
  transactions and, for each one found, calls the same real HMAC-signed
  payment webhook endpoint (`POST /webhooks/payment/:provider`) that a real
  gateway callback would use - this is not a backend bypass or a direct DB
  write to appointment/transaction status; it goes through the exact same
  signature-verified, business-logic-enforcing path as production, just
  triggered immediately instead of waiting on a real M-Pesa/card callback.

  Safe to leave running in a terminal for the whole test session; Ctrl+C to
  stop. Only ever touches the disposable hellodoctor-phonetest-postgres
  container - never a shared/production database.

  Usage: ./phonetest_autopay_watcher.ps1 [-BaseUrl http://127.0.0.1:8380] [-PollSeconds 3]
#>
param(
    [string]$BaseUrl = "http://127.0.0.1:8380",
    [int]$PollSeconds = 3
)

$ErrorActionPreference = "Stop"
$Api = "$BaseUrl/api/v1"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$BackendDir = Split-Path -Parent $ScriptDir
$EnvFile = Join-Path $BackendDir ".env"

function Read-EnvValue($name) {
    $line = Get-Content $EnvFile | Where-Object { $_ -match "^$name=" } | Select-Object -First 1
    if (-not $line) { throw "Could not find $name in $EnvFile" }
    return ($line -replace "^$name=", '').Trim().Trim('"')
}

$secret = Read-EnvValue "WEBHOOK_HMAC_SECRET"
$iso = "yyyy-MM-ddTHH:mm:ss.fffZ"

function Confirm-Payment($paymentSessionId, $gateway) {
    $eventTime = (Get-Date).ToUniversalTime().ToString($iso)
    $bodyObj = [ordered]@{
        payment_session_id    = $paymentSessionId
        transaction_reference = "AUTO-TXN-$([guid]::NewGuid())"
        status                = "SUCCESS"
        event_timestamp       = $eventTime
        provider_event_id     = "AUTO-EVT-$([guid]::NewGuid())"
    }
    $rawBody = $bodyObj | ConvertTo-Json -Compress
    $hmac = [System.Security.Cryptography.HMACSHA256]::new()
    $hmac.Key = [System.Text.Encoding]::UTF8.GetBytes($secret)
    $sigBytes = $hmac.ComputeHash([System.Text.Encoding]::UTF8.GetBytes($rawBody))
    $signature = ($sigBytes | ForEach-Object { $_.ToString("x2") }) -join ''
    $providerPath = $gateway.ToLower()
    try {
        $resp = Invoke-RestMethod -Method Post -Uri "$Api/webhooks/payment/$providerPath" `
            -Headers @{ "X-Signature" = $signature } -ContentType "application/json" -Body $rawBody
        Write-Host "  Confirmed payment_session=$paymentSessionId -> status=$($resp.data.status)" -ForegroundColor Green
    } catch {
        Write-Host "  Failed to confirm payment_session=$paymentSessionId : $($_.Exception.Message)" -ForegroundColor Red
    }
}

Write-Host "Watching for pending payments every ${PollSeconds}s (Ctrl+C to stop)..." -ForegroundColor Cyan
while ($true) {
    $rows = docker exec hellodoctor-phonetest-postgres psql -U hellodoctor -d hellodoctor -t -A -F "|" `
        -c "SELECT payment_session_id, gateway FROM transactions WHERE payment_status = 'INITIATED';"
    foreach ($line in $rows) {
        if ([string]::IsNullOrWhiteSpace($line)) { continue }
        $parts = $line -split '\|'
        if ($parts.Length -lt 2) { continue }
        $sessionId = $parts[0].Trim()
        $gateway = $parts[1].Trim()
        Write-Host "Found pending payment: $sessionId ($gateway)" -ForegroundColor Yellow
        Confirm-Payment -paymentSessionId $sessionId -gateway $gateway
    }
    Start-Sleep -Seconds $PollSeconds
}
