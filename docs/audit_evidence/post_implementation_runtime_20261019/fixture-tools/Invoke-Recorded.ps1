param(
  [Parameter(Mandatory=$true)][string]$Executable,
  [string[]]$Arguments = @(),
  [Parameter(Mandatory=$true)][string]$WorkingDirectory,
  [Parameter(Mandatory=$true)][string]$LogStem,
  [hashtable]$ExtraEnvironment = @{},
  [int]$TimeoutSeconds = 900
)
$ErrorActionPreference = 'Stop'
$runtimeRoot = 'C:\Users\CUBE\Desktop\helodoc old site\scratch\acceptance-runtime-20261019'
$safeHome = Join-Path $runtimeRoot 'home'
$safeTemp = Join-Path $runtimeRoot 'temp'
New-Item -ItemType Directory -Path $safeHome,$safeTemp -Force | Out-Null
function Quote-WindowsArgument([string]$value) {
  if ($value -notmatch '[\s"]') { return $value }
  return '"' + [regex]::Replace([regex]::Replace($value, '(\\*)"', '$1$1\"'), '(\\+)$', '$1$1') + '"'
}
$info = New-Object System.Diagnostics.ProcessStartInfo
$info.FileName = $Executable
$info.Arguments = ($Arguments | ForEach-Object { Quote-WindowsArgument $_ }) -join ' '
$info.WorkingDirectory = $WorkingDirectory
$info.UseShellExecute = $false
$info.CreateNoWindow = $true
$info.RedirectStandardOutput = $true
$info.RedirectStandardError = $true
$info.EnvironmentVariables.Clear()
$allowed = @{
  SystemRoot='C:\Windows'; WINDIR='C:\Windows'; COMSPEC='C:\Windows\System32\cmd.exe';
  PATH='C:\Windows\System32;C:\Windows;C:\Program Files\nodejs;C:\Users\CUBE\.cargo\bin;C:\Users\CUBE\.codex\tools\flutter\bin;C:\Program Files\Eclipse Adoptium\jdk-21.0.11.10-hotspot\bin';
  HOME=$safeHome; USERPROFILE=$safeHome; TEMP=$safeTemp; TMP=$safeTemp;
  APPDATA=(Join-Path $safeHome 'AppData\Roaming'); LOCALAPPDATA=(Join-Path $safeHome 'AppData\Local');
  CARGO_HOME='C:\Users\CUBE\.cargo'; RUSTUP_HOME='C:\Users\CUBE\.rustup';
  CARGO_NET_OFFLINE='true'; CARGO_BUILD_JOBS='2'; CARGO_TARGET_DIR=(Join-Path $runtimeRoot 'cargo-target');
  CI='true'; NEXT_TELEMETRY_DISABLED='1'; DART_SUPPRESS_ANALYTICS='true'; FLUTTER_SUPPRESS_ANALYTICS='true';
  npm_config_offline='true'; npm_config_update_notifier='false';
  HTTP_PROXY='http://127.0.0.1:9'; HTTPS_PROXY='http://127.0.0.1:9'; NO_PROXY='127.0.0.1,localhost,::1';
  GIT_TERMINAL_PROMPT='0'; JAVA_HOME='C:\Program Files\Eclipse Adoptium\jdk-21.0.11.10-hotspot'
}
foreach($pair in $ExtraEnvironment.GetEnumerator()) { $allowed[$pair.Key] = [string]$pair.Value }
foreach($pair in $allowed.GetEnumerator()) { $info.EnvironmentVariables[$pair.Key] = [string]$pair.Value }
$process = New-Object System.Diagnostics.Process
$process.StartInfo = $info
$started = [DateTime]::UtcNow
$timer = [Diagnostics.Stopwatch]::StartNew()
[void]$process.Start()
$stdout = $process.StandardOutput.ReadToEndAsync()
$stderr = $process.StandardError.ReadToEndAsync()
$timedOut = -not $process.WaitForExit($TimeoutSeconds * 1000)
if($timedOut) {
  & C:\Windows\System32\taskkill.exe /PID $process.Id /T /F | Out-Null
  $process.WaitForExit()
}
$timer.Stop()
[IO.File]::WriteAllText($LogStem + '.stdout.log', $stdout.Result)
[IO.File]::WriteAllText($LogStem + '.stderr.log', $stderr.Result)
$metadata = [ordered]@{
  started_utc=$started.ToString('o'); executable=$Executable; arguments=$Arguments;
  working_directory=$WorkingDirectory; exit_code=$process.ExitCode; timed_out=$timedOut;
  duration_seconds=$timer.Elapsed.TotalSeconds; environment_keys=@($allowed.Keys | Sort-Object);
  environment_policy='Cleared, allowlisted, scratch home/temp, offline package policy, external HTTP proxies point to closed loopback; not formal OS network isolation';
  source_changes_authorized=$false; local_development_verification=$true; formal_sandbox_certified=$false
}
$metadata | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath ($LogStem + '.json') -Encoding UTF8
$metadata | ConvertTo-Json -Depth 5
