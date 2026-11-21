<#
.SYNOPSIS
  Writes the backend's Firebase Cloud Messaging settings (FCM_PROJECT_ID,
  FCM_CLIENT_EMAIL, FCM_PRIVATE_KEY_B64) into an env file from a Firebase
  service-account JSON key.

.DESCRIPTION
  Get the key in the Firebase console: Project settings > Service accounts >
  Generate new private key (project afyamkononi-ec314, the project the apps'
  google-services.json files point at). Keep the JSON outside the repository.

  The key is never printed. Existing FCM_* lines in the env file are replaced;
  every other line is left as it is. Restart the backend afterwards.

.EXAMPLE
  .\scripts\set_fcm_env.ps1 -ServiceAccountJson C:\keys\afyamkononi-fcm.json
  .\scripts\set_fcm_env.ps1 -ServiceAccountJson .\sa.json -EnvFile /etc/hellodoctor/hellodoctor-vm.env
#>
param(
    [Parameter(Mandatory = $true)][string]$ServiceAccountJson,
    [string]$EnvFile = (Join-Path $PSScriptRoot '..\.env'),
    [string]$ExpectedProjectId = 'afyamkononi-ec314'
)
$ErrorActionPreference = 'Stop'

$account = Get-Content -Raw -LiteralPath $ServiceAccountJson | ConvertFrom-Json
if ($account.type -ne 'service_account') { throw 'This is not a service-account key file.' }
if (-not $account.project_id -or -not $account.client_email -or -not $account.private_key) {
    throw 'The key file is missing project_id, client_email or private_key.'
}
if ($ExpectedProjectId -and $account.project_id -ne $ExpectedProjectId) {
    throw "The key is for project '$($account.project_id)', but the apps use '$ExpectedProjectId'."
}
if ($account.private_key -notmatch 'BEGIN PRIVATE KEY') { throw 'private_key is not a PEM key.' }

$keyB64 = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($account.private_key))
$settings = [ordered]@{
    FCM_PROJECT_ID      = $account.project_id
    FCM_CLIENT_EMAIL    = $account.client_email
    FCM_PRIVATE_KEY_B64 = $keyB64
}

$lines = @()
if (Test-Path -LiteralPath $EnvFile) {
    $lines = @(Get-Content -LiteralPath $EnvFile | Where-Object { $_ -notmatch '^\s*FCM_(PROJECT_ID|CLIENT_EMAIL|PRIVATE_KEY_B64)\s*=' })
}
foreach ($name in $settings.Keys) { $lines += "$name=$($settings[$name])" }
Set-Content -LiteralPath $EnvFile -Value $lines -Encoding ascii

Write-Output "Wrote FCM settings for project '$($account.project_id)' ($($account.client_email)) to $EnvFile."
Write-Output 'Restart the backend; its startup log should no longer say "FCM push transport: Noop".'
