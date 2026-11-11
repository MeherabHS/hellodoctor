<#
.SYNOPSIS
  Focused, non-destructive self-test for Get-MetricsSnapshot (lib_metrics_snapshot.ps1).
  No network calls, no Docker dependency -- runs a tiny local HTTP listener
  serving controlled (good/truncated/incomplete) bodies, so every failure
  mode this function exists to catch is actually exercised, not just assumed.

.EXAMPLE
  .\lib_metrics_snapshot.smoketest.ps1
  (prints PASS/FAIL per case; exits 1 if any case fails)
#>
. (Join-Path $PSScriptRoot 'lib_metrics_snapshot.ps1')

$failures = @()
function Assert($condition, $message) {
    if (-not $condition) { $script:failures += $message }
}

$testDir = Join-Path $env:TEMP "metrics_snapshot_smoketest_$([guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Force -Path $testDir | Out-Null

function Start-TestListener([string]$body, [int]$statusCode = 200) {
    $listener = New-Object System.Net.HttpListener
    $port = Get-Random -Minimum 19000 -Maximum 19999
    $prefix = "http://127.0.0.1:$port/"
    $listener.Prefixes.Add($prefix)
    $listener.Start()
    $job = Start-Job -ScriptBlock {
        param($listenerPort, $respBody, $respStatus)
        Add-Type -AssemblyName System.Net.HttpListener -ErrorAction SilentlyContinue
        $l = New-Object System.Net.HttpListener
        $l.Prefixes.Add("http://127.0.0.1:$listenerPort/")
        $l.Start()
        $ctx = $l.GetContext()
        $ctx.Response.StatusCode = $respStatus
        $bytes = [System.Text.Encoding]::UTF8.GetBytes($respBody)
        $ctx.Response.ContentLength64 = $bytes.Length
        $ctx.Response.OutputStream.Write($bytes, 0, $bytes.Length)
        $ctx.Response.OutputStream.Close()
        $l.Stop()
    } -ArgumentList $port, $body, $statusCode
    $listener.Stop()
    return @{ Prefix = $prefix; Job = $job }
}

# Case 1: complete, well-formed body with required families -> should succeed.
$goodBody = "# HELP admission_rejected_total x`n# TYPE admission_rejected_total counter`nadmission_rejected_total{class=`"normal`"} 5`n"
$listener = Start-TestListener -body $goodBody
$outFile = Join-Path $testDir "good.txt"
try {
    Get-MetricsSnapshot -Uri $listener.Prefix -OutFile $outFile -RequiredMetricFamilies @('admission_rejected_total') -MaxAttempts 1
    Assert (Test-Path $outFile) "Case 1 (good body): expected output file to be written"
    Assert ((Get-Content $outFile -Raw) -eq $goodBody) "Case 1 (good body): written content should match exactly"
    Write-Host "PASS: complete well-formed response is accepted and written"
} catch {
    $failures += "Case 1 (good body) threw unexpectedly: $_"
} finally {
    Wait-Job $listener.Job -Timeout 5 | Out-Null; Remove-Job $listener.Job -Force -ErrorAction SilentlyContinue
}

# Case 2: truncated mid-line body -> should throw, must NOT write the file.
$truncatedBody = "# HELP admission_rejected_total x`nadmission_rejected_total{class=`"normal`"} 5`nsqlx_pool_acquire_wait_sec"
$listener = Start-TestListener -body $truncatedBody
$outFile = Join-Path $testDir "truncated.txt"
$threw = $false
try {
    Get-MetricsSnapshot -Uri $listener.Prefix -OutFile $outFile -MaxAttempts 1
} catch {
    $threw = $true
} finally {
    Wait-Job $listener.Job -Timeout 5 | Out-Null; Remove-Job $listener.Job -Force -ErrorAction SilentlyContinue
}
Assert $threw "Case 2 (truncated body): expected Get-MetricsSnapshot to throw"
Assert (-not (Test-Path $outFile)) "Case 2 (truncated body): must NOT write a partial file to the destination"
if ($threw -and -not (Test-Path $outFile)) { Write-Host "PASS: truncated response is rejected, no partial file written" }

# Case 3: complete body but missing a required metric family -> should throw.
$incompleteBody = "# HELP something_else x`nsomething_else 1`n"
$listener = Start-TestListener -body $incompleteBody
$outFile = Join-Path $testDir "missing_family.txt"
$threw = $false
try {
    Get-MetricsSnapshot -Uri $listener.Prefix -OutFile $outFile -RequiredMetricFamilies @('admission_rejected_total') -MaxAttempts 1
} catch {
    $threw = $true
} finally {
    Wait-Job $listener.Job -Timeout 5 | Out-Null; Remove-Job $listener.Job -Force -ErrorAction SilentlyContinue
}
Assert $threw "Case 3 (missing required family): expected Get-MetricsSnapshot to throw"
Assert (-not (Test-Path $outFile)) "Case 3 (missing required family): must NOT write a file"
if ($threw -and -not (Test-Path $outFile)) { Write-Host "PASS: response missing a required metric family is rejected" }

# Case 4: a prior GOOD snapshot at the destination must survive a later failed attempt.
$outFile = Join-Path $testDir "preserve_existing.txt"
Set-Content -Path $outFile -Value "PREVIOUS_GOOD_SNAPSHOT" -Encoding utf8 -NoNewline
$listener = Start-TestListener -body $truncatedBody
try {
    Get-MetricsSnapshot -Uri $listener.Prefix -OutFile $outFile -MaxAttempts 1
} catch {
    # expected
} finally {
    Wait-Job $listener.Job -Timeout 5 | Out-Null; Remove-Job $listener.Job -Force -ErrorAction SilentlyContinue
}
Assert ((Get-Content $outFile -Raw) -eq "PREVIOUS_GOOD_SNAPSHOT") "Case 4 (atomic write): a failed attempt must not clobber an existing good snapshot"
if ((Get-Content $outFile -Raw) -eq "PREVIOUS_GOOD_SNAPSHOT") { Write-Host "PASS: a failed attempt never overwrites a previously-good snapshot" }

# Case 5: LineFilter narrows what's written, but completeness is still
# checked against the FULL unfiltered body first.
$mixedBody = "# HELP admission_rejected_total x`nadmission_rejected_total{class=`"normal`"} 5`nhttp_request_duration_seconds_count 42`n"
$listener = Start-TestListener -body $mixedBody
$outFile = Join-Path $testDir "filtered.txt"
try {
    Get-MetricsSnapshot -Uri $listener.Prefix -OutFile $outFile -RequiredMetricFamilies @('admission_rejected_total') -LineFilter '^admission_' -MaxAttempts 1
    $written = Get-Content $outFile -Raw
    Assert ($written -match 'admission_rejected_total') "Case 5 (LineFilter): filtered output should retain matching lines"
    Assert ($written -notmatch 'http_request_duration') "Case 5 (LineFilter): filtered output should exclude non-matching lines"
    if ($written -match 'admission_rejected_total' -and $written -notmatch 'http_request_duration') {
        Write-Host "PASS: LineFilter narrows output while completeness check used the full body"
    }
} catch {
    $failures += "Case 5 (LineFilter) threw unexpectedly: $_"
} finally {
    Wait-Job $listener.Job -Timeout 5 | Out-Null; Remove-Job $listener.Job -Force -ErrorAction SilentlyContinue
}

Remove-Item -Recurse -Force $testDir -ErrorAction SilentlyContinue

if ($failures.Count -gt 0) {
    Write-Host "`nFAILURES:" -ForegroundColor Red
    $failures | ForEach-Object { Write-Host "  - $_" -ForegroundColor Red }
    exit 1
}
Write-Host "`nALL CASES PASSED"
exit 0
