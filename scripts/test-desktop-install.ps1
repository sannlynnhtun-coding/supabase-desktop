param([Parameter(Mandatory=$true)][Alias('InstallerId')][int]$LauncherId, [Parameter(Mandatory=$true)][string]$Evidence)
$ErrorActionPreference = 'Stop'
$root = Join-Path $env:LOCALAPPDATA 'SupabaseLocal'
$registry = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\SupabaseLocal'
$programs = Join-Path ([Environment]::GetFolderPath('Programs')) 'Supabase Local'
$evidencePath = [IO.Path]::GetFullPath($Evidence)
New-Item -ItemType Directory -Force $evidencePath | Out-Null
function Wait-Ready {
    param([int]$ProcessId)
    for ($attempt=0; $attempt -lt 1350; $attempt++) {
        if (!(Get-Process -Id $ProcessId -ErrorAction SilentlyContinue)) { throw 'Launcher exited before becoming ready' }
        $status = $null
        try {
            $status = Invoke-RestMethod 'http://127.0.0.1:54320/api/status' -TimeoutSec 3
        } catch { if ($attempt -eq 1349) { throw } }
        if ($status.state -eq 'failed') { throw $status.message }
        if ($status.studioUrl -and $status.available.Count -eq 6) { return $status }
        Start-Sleep -Seconds 2
    }
    throw 'Timed out waiting for six native services'
}
function Close-Launcher {
    param([int]$ProcessId)
    $process = Get-Process -Id $ProcessId
    Write-Output 'Confirm exit in the launcher window to complete this interactive shutdown check.'
    if (!$process.CloseMainWindow()) { throw 'Could not request graceful launcher shutdown' }
    if (!$process.WaitForExit(60000)) { throw 'Launcher did not stop gracefully' }
    foreach ($port in @(54320,54321,54322,54323,54324,54325,54326,54328,54330)) {
        if (Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue) { throw "Owned listener remains: $port" }
    }
}
$status = Wait-Ready $LauncherId
$status | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $evidencePath 'installer-status.json')
$registration = Get-ItemProperty $registry
if ($registration.InstallerOwner -ne 'SupabaseLocal.NativeWindows.v1' -or $registration.InstallLocation -ne $root) { throw 'Wrong uninstall registration' }
$shell = New-Object -ComObject WScript.Shell
$link = $shell.CreateShortcut((Join-Path $programs 'Supabase Local.lnk'))
$uninstallLink = $shell.CreateShortcut((Join-Path $programs 'Uninstall Supabase Local.lnk'))
$executable = [IO.Path]::GetFullPath($link.TargetPath)
if (!$executable.StartsWith((Join-Path $root 'versions')+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Shortcut escapes installation' }
if (!(Test-Path -LiteralPath $executable) -or $link.Arguments -ne '' -or $uninstallLink.TargetPath -ne $executable -or $uninstallLink.Arguments -ne '--uninstall') { throw 'Invalid Start menu shortcuts' }
if ($registration.UninstallString -ne ('"'+$executable+'" --uninstall')) { throw 'Wrong Installed apps uninstall command' }
@{target=$executable; icon=$link.IconLocation; uninstall=$registration.UninstallString} | ConvertTo-Json | Set-Content (Join-Path $evidencePath 'shortcuts.json')
$studio = Invoke-WebRequest 'http://127.0.0.1:54323' -TimeoutSec 30
if ($studio.StatusCode -ne 200 -or $studio.Content -notmatch 'Supabase' -or $studio.Content -notmatch '/_next/') { throw 'Upstream Studio HTML was not served' }
$studio.Content | Set-Content (Join-Path $evidencePath 'studio.html')
Write-Output 'PASS: installed launcher starts six services, upstream Studio and registered shortcuts.'
Close-Launcher $LauncherId
$desktop = Start-Process -FilePath (Join-Path $programs 'Supabase Local.lnk') -PassThru
$status = Wait-Ready $desktop.Id
$status | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $evidencePath 'shortcut-status.json')
Write-Output 'PASS: one Start menu shortcut launch starts all six services.'
$desktop.Id | Set-Content (Join-Path $evidencePath 'desktop.pid')
Write-Output 'Ready for browser inspection. Run the uninstall verification after closing the launcher gracefully.'
