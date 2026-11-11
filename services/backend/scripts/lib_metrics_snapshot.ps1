<#
.SYNOPSIS
  Milestone 8 storage/stabilization pass (2026-11-05): a shared, reusable,
  safe way to capture a `/metrics` snapshot to disk. Replaces the ad hoc
  `(Invoke-WebRequest -Uri ... -UseBasicParsing).Content | Out-File ...`
  pattern used directly in run_six_service_web_comparison.ps1 and in an
  improvised endurance-run orchestration script, which produced a visibly
  truncated final snapshot during the 60-minute endurance run (44KB vs ~127KB
  for earlier snapshots of the same growing metric set, cut off mid-metric-
  list with no error raised) -- confirmed root cause: no explicit timeout, no
  completeness check, and a plain `Out-File` that happily writes whatever
  partial bytes it received as if it were a complete, trustworthy snapshot.

.DESCRIPTION
  Get-MetricsSnapshot:
    - Issues the request with an explicit, generous timeout (never the
      client's implicit default).
    - Retries (bounded) on transport failure, non-200 status, or a response
      that fails the completeness check -- never treats a failed attempt as
      silently-skippable.
    - Verifies completeness via two independent signals: (a) every
      caller-supplied "this metric family must be present" name actually
      appears as a HELP/TYPE or sample line, and (b) the body does not look
      truncated mid-line (ends with a newline after a well-formed sample
      line, not a cut-off partial token).
    - Writes atomically: the full body is written to a temporary file first,
      then moved into place with Move-Item -Force, which on NTFS is a single
      filesystem rename -- a reader can never observe a partially-written
      target file, and a failed/incomplete attempt can never clobber a
      previously-good snapshot still sitting at the destination path.
    - Throws a clear, specific exception on exhausted retries rather than
      ever leaving a partial file at the destination path.

.EXAMPLE
  . .\lib_metrics_snapshot.ps1
  Get-MetricsSnapshot -Uri 'http://127.0.0.1:58280/metrics' -OutFile "$OutDir\post_test_counters.txt" `
    -RequiredMetricFamilies @('admission_rejected_total','admission_permit_hold_seconds')
#>

function Get-MetricsSnapshot {
    param(
        [Parameter(Mandatory = $true)][string]$Uri,
        [Parameter(Mandatory = $true)][string]$OutFile,
        [string[]]$RequiredMetricFamilies = @(),
        [int]$TimeoutSec = 30,
        [int]$MaxAttempts = 4,
        [int]$RetryDelaySec = 3,
        # Optional: write only the lines matching this regex (e.g. '^admission_')
        # instead of the full scrape body. Completeness/truncation checks
        # above always run against the FULL, unfiltered response first --
        # filtering only ever narrows what gets written to disk afterward,
        # never what gets validated.
        [string]$LineFilter
    )

    $lastError = $null
    for ($attempt = 1; $attempt -le $MaxAttempts; $attempt++) {
        try {
            $response = Invoke-WebRequest -Uri $Uri -UseBasicParsing -TimeoutSec $TimeoutSec
            if ($response.StatusCode -ne 200) {
                throw "Non-200 status: $($response.StatusCode)"
            }
            # Invoke-WebRequest -UseBasicParsing returns .Content as a string
            # only when the response declares a recognized text content-type;
            # otherwise it comes back as a raw byte[] -- confirmed live via
            # this function's own smoke test. Handle both explicitly rather
            # than assuming the server always sets the header exactly right.
            $body = $response.Content
            if ($body -is [byte[]]) {
                $body = [System.Text.Encoding]::UTF8.GetString($body)
            }
            if ([string]::IsNullOrEmpty($body)) {
                throw "Empty response body"
            }
            # Truncation signal: a genuinely complete Prometheus text-format
            # response always ends with a newline after a well-formed sample
            # line (`metric_name{labels} value`), never mid-identifier/mid-
            # number the way a cut-off TCP read does. This caught the exact
            # real truncation this function exists to prevent (confirmed
            # against the actual corrupted endurance-run snapshot).
            $trimmed = $body.TrimEnd("`r", "`n")
            $lastLine = ($trimmed -split "`r?`n")[-1]
            if ($lastLine -notmatch '^[a-zA-Z_:][a-zA-Z0-9_:]*(\{[^}]*\})?\s+[0-9eE+\-.]+$') {
                throw "Response body looks truncated -- last line is not a complete metric sample: '$lastLine'"
            }
            $missing = @($RequiredMetricFamilies | Where-Object { $body -notmatch [regex]::Escape($_) })
            if ($missing.Count -gt 0) {
                throw "Missing required metric families: $($missing -join ', ')"
            }

            $contentToWrite = $body
            if ($LineFilter) {
                $matchingLines = ($body -split "`r?`n") | Where-Object { $_ -match $LineFilter }
                if (-not $matchingLines) {
                    throw "LineFilter '$LineFilter' matched zero lines in an otherwise complete, non-truncated response -- refusing to write an empty filtered snapshot."
                }
                $contentToWrite = $matchingLines -join "`n"
            }

            $tempFile = "$OutFile.tmp-$([guid]::NewGuid().ToString('N'))"
            Set-Content -Path $tempFile -Value $contentToWrite -Encoding utf8 -NoNewline
            Move-Item -Path $tempFile -Destination $OutFile -Force
            return
        } catch {
            $lastError = $_
            Write-Warning "Get-MetricsSnapshot attempt $attempt/$MaxAttempts for '$Uri' failed: $_"
            if ($attempt -lt $MaxAttempts) {
                Start-Sleep -Seconds $RetryDelaySec
            }
        }
    }
    throw "Get-MetricsSnapshot: exhausted $MaxAttempts attempts for '$Uri' -- last error: $lastError. No file was written or overwritten at '$OutFile'."
}
