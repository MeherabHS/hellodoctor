<#
  One-command release build for sideloaded IRL/field testing (no adb/device
  attached). Auto-detects this machine's LAN IPv4 address and bakes it in as
  API_BASE_URL, so the dart-define flags never have to be retyped (and can't
  be forgotten) between builds. Run the Rust backend reachable on that
  address and port before installing the APK.

  Usage: ./build_field_test.ps1 [-Port 8080]
#>
param(
    [int]$Port = 8080
)

$ip = (Get-NetIPAddress -AddressFamily IPv4 -AddressState Preferred |
    Where-Object {
        $_.InterfaceAlias -notmatch 'Loopback|vEthernet|WSL|Virtual' -and
        $_.IPAddress -notlike '169.254.*'
    } |
    Select-Object -First 1 -ExpandProperty IPAddress)

if (-not $ip) {
    Write-Error "Could not auto-detect a LAN IPv4 address. Find yours with 'ipconfig' and pass it manually: flutter build apk --release --split-per-abi --dart-define=API_BASE_URL=http://<ip>:$Port/api/v1"
    exit 1
}

$baseUrl = "http://${ip}:${Port}/api/v1"
Write-Host "Building release APK against $baseUrl" -ForegroundColor Cyan
Write-Host "(Make sure the backend is actually listening on 0.0.0.0, not just 127.0.0.1, and the phone is on the same network.)" -ForegroundColor Yellow

flutter build apk --release --target-platform android-arm64 --split-per-abi --dart-define="API_BASE_URL=$baseUrl" --dart-define=ALLOW_INSECURE_HTTP=true --dart-define=DEV_PAYOUT_BINDING=true

if ($LASTEXITCODE -eq 0) {
    Write-Host "`nBuilt APKs:" -ForegroundColor Green
    Get-ChildItem "build\app\outputs\flutter-apk\*release.apk" | ForEach-Object { Write-Host "  $($_.FullName)" }
    Write-Host "`nCopy the arm64-v8a one to the phone and install it (unknown sources must be allowed)." -ForegroundColor Green
}
