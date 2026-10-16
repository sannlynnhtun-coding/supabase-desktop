param(
    [Parameter(Mandatory=$true)][string]$Uninstaller,
    [Parameter(Mandatory=$true)][string]$Evidence
)
$ErrorActionPreference = 'Stop'
$exe = [IO.Path]::GetFullPath($Uninstaller)
$evidencePath = [IO.Path]::GetFullPath($Evidence)
New-Item -ItemType Directory -Force -Path $evidencePath | Out-Null
foreach ($mode in @('uninstall', 'cleanup', 'ui')) {
    $result = Join-Path $evidencePath "rust-uninstaller-$mode.json"
    if (Test-Path -LiteralPath $result) { throw "Use fresh evidence: $result" }
    $process = Start-Process -FilePath $exe -ArgumentList @("--verify-$mode", ('"' + $result + '"')) -WindowStyle Hidden -PassThru -RedirectStandardError (Join-Path $evidencePath "rust-uninstaller-$mode.err.log")
    if (!$process.WaitForExit(90000)) { throw "Rust uninstaller $mode check timed out" }
    if ($process.ExitCode -ne 0 -or !(Test-Path -LiteralPath $result)) { throw "Rust uninstaller $mode failed: $(Get-Content (Join-Path $evidencePath "rust-uninstaller-$mode.err.log") -Raw)" }
    $status = Get-Content -LiteralPath $result -Raw | ConvertFrom-Json
    foreach ($property in $status.PSObject.Properties) { if ($property.Value -ne $true) { throw "Rust uninstaller check failed: $($property.Name)" } }
    Write-Output "PASS: Rust uninstaller $mode ($($status.PSObject.Properties.Name -join ', '))"
}
