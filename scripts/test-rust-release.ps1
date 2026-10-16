param(
    [string]$Installer = "$PSScriptRoot/../artifacts/installer/SupabaseLocal.Setup.exe",
    [string]$Evidence = "$PSScriptRoot/../work/rust-release-test-$([Guid]::NewGuid().ToString('N'))",
    [switch]$Resume
)
$ErrorActionPreference = 'Stop'
$installerPath = [IO.Path]::GetFullPath($Installer)
$evidencePath = [IO.Path]::GetFullPath($Evidence)
if ((Test-Path -LiteralPath $evidencePath) -and !$Resume) { throw 'Use a fresh evidence directory, or -Resume after a completed extraction.' }
$ports = @(54320,54321,54322,54323,54324,54325,54326,54328,54330,54331)
foreach ($port in $ports) {
    if (Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue) { throw "Port $port is already in use. Close the local stack first." }
}
New-Item -ItemType Directory -Force -Path $evidencePath | Out-Null
$root = Join-Path $evidencePath 'installation'
$expected = (Get-Content -LiteralPath "$installerPath.sha256" -Raw).Split(' ')[0]
$actual = (Get-FileHash -LiteralPath $installerPath -Algorithm SHA256).Hash.ToLowerInvariant()
if ($expected -ne $actual) { throw 'Installer sidecar checksum mismatch.' }
function Verify-Install {
    param([string]$Log, [bool]$ShouldSucceed = $true)
    $process = Start-Process -FilePath $installerPath -ArgumentList @('--verify-install', ('"' + $root + '"')) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $evidencePath "$Log.out.log") -RedirectStandardError (Join-Path $evidencePath "$Log.err.log")
    $process.WaitForExit()
    if (($process.ExitCode -eq 0) -ne $ShouldSucceed) { throw "Unexpected installer verification result: $Log ($($process.ExitCode))" }
}
if ($Resume) {
    if (!(Test-Path (Join-Path $root 'verification-result.json'))) { throw 'Resume requires a completed extraction.' }
    'Resuming release checks after completed extraction.'
} else { Verify-Install 'extract' }
$version = (Get-Content (Join-Path $root 'verification-result.json') -Raw | ConvertFrom-Json).installed
if (!$version.StartsWith((Join-Path $root 'versions') + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Version escaped test installation.' }
New-Item -ItemType Directory -Path (Join-Path $root 'data') | Out-Null
$marker = Join-Path $root 'data/preservation.txt'
[IO.File]::WriteAllText($marker, 'Preserve existing project data')
Verify-Install 'repeat'
$target = Join-Path $version 'SupabaseLocal.deps.json'
$original = [IO.File]::ReadAllBytes($target)
try {
    [IO.File]::WriteAllText($target, 'corrupt payload file')
    Verify-Install 'tamper' $false
} finally { [IO.File]::WriteAllBytes($target, $original) }
Verify-Install 'restored'
if ([IO.File]::ReadAllText($marker) -ne 'Preserve existing project data') { throw 'Installer modified project data.' }
'PASS: release SHA256, actual embedded extraction, repeat verification, tamper rejection and project data preservation.'

$runtime = Join-Path $version 'runtime'
$data = Join-Path $evidencePath 'manager-data'
& powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$PSScriptRoot/test-manager.ps1" -Manager (Join-Path $version 'SupabaseLocal.exe') -Runtime $runtime -Data $data -WithStorage 2>&1 | Tee-Object (Join-Path $evidencePath 'manager-checks.log')
if ($LASTEXITCODE -ne 0) { throw 'Bundled manager integration checks failed.' }

$launchData = Join-Path $evidencePath 'launcher-data'
New-Item -ItemType Directory -Path $launchData | Out-Null
$launcher = Start-Process -FilePath (Join-Path $version 'SupabaseLocal.App.exe') -ArgumentList @('--verify-launch', ('"' + $launchData + '"')) -WindowStyle Hidden -PassThru
if (!$launcher.WaitForExit(600000)) {
    $launcher.CloseMainWindow() | Out-Null
    throw 'Installed desktop launcher verification timed out.'
}
if ($launcher.ExitCode -ne 0 -or !(Test-Path (Join-Path $launchData 'launcher-verification.json'))) { throw 'Installed desktop launcher failed.' }
$status = Get-Content (Join-Path $launchData 'launcher-verification.json') -Raw | ConvertFrom-Json
if ($status.Available.Count -ne 6) { throw 'Desktop launcher did not start all six services.' }
foreach ($port in $ports) {
    if (Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue) { throw "Listener remains after launcher shutdown: $port" }
}
'PASS: installed desktop launcher starts six services and stops all listeners gracefully.'

$uiResult = Join-Path $evidencePath 'gpui-window.json'
$gui = Start-Process -FilePath $installerPath -ArgumentList @('--verify-ui', ('"' + $uiResult + '"')) -WindowStyle Hidden -PassThru -RedirectStandardError (Join-Path $evidencePath 'gpui-window.err.log')
if (!$gui.WaitForExit(60000)) { throw 'GPUI setup window verification timed out.' }
if ($gui.ExitCode -ne 0 -or !(Test-Path -LiteralPath $uiResult)) { throw 'GPUI setup window verification failed.' }
$uiStatus = Get-Content -LiteralPath $uiResult -Raw | ConvertFrom-Json
if (!$uiStatus.nativeWindow -or !$uiStatus.frameRendered) { throw 'GPUI did not render the setup window.' }
'PASS: packaged GPUI setup creates and closes its native window.'
@{ installer = $installerPath; sha256 = $actual; bytes = (Get-Item $installerPath).Length; version = $version; extraction = $true; repeat = $true; tamperRejected = $true; dataPreserved = $true; managerCycles = 2; storage = $true; launcher = $true; gpuiWindow = $true; completedUtc = [DateTime]::UtcNow.ToString('o') } | ConvertTo-Json | Set-Content (Join-Path $evidencePath 'release-test-result.json')
Write-Output "Evidence: $evidencePath"
