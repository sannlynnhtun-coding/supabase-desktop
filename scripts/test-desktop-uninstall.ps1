param([Parameter(Mandatory=$true)][string]$Evidence)
$ErrorActionPreference = 'Stop'
$evidencePath = [IO.Path]::GetFullPath($Evidence)
$root = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'SupabaseLocal'))
$registry = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\SupabaseLocal'
$programs = Join-Path ([Environment]::GetFolderPath('Programs')) 'Supabase Desktop'
$shortcut = Get-Content (Join-Path $evidencePath 'shortcuts.json') -Raw | ConvertFrom-Json
$desktopId = [int](Get-Content (Join-Path $evidencePath 'desktop.pid'))
$desktop = Get-Process -Id $desktopId
if (![string]::Equals($desktop.Path,$shortcut.target,[StringComparison]::OrdinalIgnoreCase)) { throw 'Recorded PID belongs to another executable' }
if (!$desktop.CloseMainWindow() -or !$desktop.WaitForExit(60000)) { throw 'Desktop app failed graceful shutdown' }
foreach ($port in @(54320,54321,54322,54323,54324,54325,54326,54328,54330)) {
    if (Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue) { throw "Owned listener remains: $port" }
}
$data = Join-Path $root 'data'
# Hash all project files after graceful shutdown. Only hashes, never secrets, enter the evidence.
$before = @{}
foreach ($file in Get-ChildItem -LiteralPath $data -File -Recurse) {
    $before[[IO.Path]::GetRelativePath($data,$file.FullName)] = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash
}
if ($before.Count -eq 0) { throw 'No project data exists to verify preservation' }
$temporary = Join-Path ([IO.Path]::GetTempPath()) ('SupabaseLocal-Uninstall-'+[Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $temporary | Out-Null
$workerPath = Join-Path $temporary 'SupabaseLocal.App.exe'
$uninstaller = Join-Path ([IO.Path]::GetDirectoryName($shortcut.target)) 'SupabaseLocal.Uninstall.exe'
if (!(Test-Path -LiteralPath $uninstaller)) { $uninstaller = $shortcut.target }
Copy-Item -LiteralPath $uninstaller -Destination $workerPath
# Exercise the shipped worker and its real current-user installation scope; false keeps data.
# The original launcher is already stopped, so the worker takes the normal parent-exited path.
$worker = Start-Process -FilePath $workerPath -ArgumentList '--uninstall-worker',$desktopId,'false' -PassThru
for ($attempt=0; $attempt -lt 1200; $attempt++) {
    if (!(Test-Path $registry)) { break }
    if ($worker.HasExited) { throw "Uninstaller exited early: $($worker.ExitCode)" }
    Start-Sleep -Seconds 1
}
if (Test-Path $registry) { throw 'Uninstaller did not remove Installed apps registration' }
if (Test-Path (Join-Path $root 'versions')) { throw 'Installed application files remain' }
foreach ($name in @('Supabase Desktop.lnk','Uninstall Supabase Desktop.lnk')) {
    if (Test-Path -LiteralPath (Join-Path $programs $name)) { throw "Shortcut remains: $name" }
}
$after = @(Get-ChildItem -LiteralPath $data -File -Recurse)
if ($after.Count -ne $before.Count) { throw 'Project file count changed during uninstall' }
foreach ($file in $after) {
    $relative = [IO.Path]::GetRelativePath($data,$file.FullName)
    if ($before[$relative] -ne (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash) { throw "Project data changed: $relative" }
}
if (!$worker.CloseMainWindow() -or !$worker.WaitForExit(30000)) { throw 'Uninstall completion window did not close' }
if ($worker.ExitCode -ne 0) { throw "Uninstaller failed: $($worker.ExitCode)" }
for ($attempt=0; $attempt -lt 60 -and (Test-Path -LiteralPath $temporary); $attempt++) { Start-Sleep -Seconds 1 }
if (Test-Path -LiteralPath $temporary) { throw 'Deferred worker cleanup did not finish' }
@{removedApplication=$true;removedShortcuts=$true;removedRegistration=$true;preservedProjectFiles=$before.Count;workerCleanup=$true} |
    ConvertTo-Json | Set-Content (Join-Path $evidencePath 'uninstall-result.json')
Write-Output "PASS: real uninstall removes application, shortcuts and registration; preserves all $($before.Count) project files; removes its temporary worker."
