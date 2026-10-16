param(
    [string]$Installer = "$PSScriptRoot/../artifacts/installer/SupabaseDesktop.Setup.exe",
    [string]$Evidence = "$PSScriptRoot/../work/rust-current-user-install-$([Guid]::NewGuid().ToString('N'))"
)
$ErrorActionPreference = 'Stop'
$installerPath = [IO.Path]::GetFullPath($Installer)
$evidencePath = [IO.Path]::GetFullPath($Evidence)
if (Test-Path -LiteralPath $evidencePath) { throw 'Use a fresh evidence directory.' }
$ports = @(54320,54321,54322,54323,54324,54325,54326,54328,54330,54331)
foreach ($port in $ports) {
    if (Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue) { throw 'Close the running local stack before testing installation.' }
}
New-Item -ItemType Directory -Path $evidencePath | Out-Null
$root = Join-Path $env:LOCALAPPDATA 'SupabaseLocal'
$data = Join-Path $root 'data'
$before = @{}
if (Test-Path -LiteralPath $data) {
    foreach ($file in Get-ChildItem -LiteralPath $data -File -Recurse) { $before[$file.FullName] = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash }
}
# --install calls the same function as the GUI Install button, including prerequisites and registration.
$process = Start-Process -FilePath $installerPath -ArgumentList '--install' -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $evidencePath 'install.out.log') -RedirectStandardError (Join-Path $evidencePath 'install.err.log')
$process.WaitForExit()
if ($process.ExitCode -ne 0) { throw ('Current-user installation failed: ' + (Get-Content (Join-Path $evidencePath 'install.err.log') -Raw)) }
foreach ($name in $before.Keys) {
    if (!(Test-Path -LiteralPath $name) -or (Get-FileHash -LiteralPath $name -Algorithm SHA256).Hash -ne $before[$name]) { throw 'Installation changed an existing project file.' }
}
$registration = Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\SupabaseLocal'
if ($registration.InstallerOwner -ne 'SupabaseLocal.NativeWindows.v1' -or $registration.InstallLocation -ne $root) { throw 'Incorrect installation registration.' }
$programs = Join-Path ([Environment]::GetFolderPath('Programs')) 'Supabase Desktop'
$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut((Join-Path $programs 'Supabase Desktop.lnk'))
$uninstall = $shell.CreateShortcut((Join-Path $programs 'Uninstall Supabase Desktop.lnk'))
$exe = $shortcut.TargetPath
if (!$exe.StartsWith((Join-Path $root 'versions') + '\', [StringComparison]::OrdinalIgnoreCase) -or !(Test-Path -LiteralPath $exe)) { throw 'Invalid installed launcher.' }
if ($shortcut.Arguments -ne '' -or $uninstall.TargetPath -ne $exe -or $uninstall.Arguments -ne '--uninstall' -or $registration.UninstallString -ne ('"' + $exe + '" --uninstall')) { throw 'Incorrect shortcuts or uninstall command.' }
'PASS: real current-user prerequisites, payload installation, Start menu shortcuts and Installed apps registration; existing project files preserved.'
$launchData = Join-Path $evidencePath 'launcher-data'
New-Item -ItemType Directory -Path $launchData | Out-Null
$launcher = Start-Process -FilePath $exe -ArgumentList @('--verify-launch', ('"' + $launchData + '"')) -WindowStyle Hidden -PassThru
if (!$launcher.WaitForExit(600000)) { $launcher.CloseMainWindow() | Out-Null; throw 'Installed launcher check timed out.' }
if ($launcher.ExitCode -ne 0 -or !(Test-Path (Join-Path $launchData 'launcher-verification.json'))) { throw 'Installed launcher verification failed.' }
$status = Get-Content (Join-Path $launchData 'launcher-verification.json') -Raw | ConvertFrom-Json
if ($status.Available.Count -ne 6) { throw 'Installed launcher did not start six services.' }
foreach ($port in $ports) {
    if (Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue) { throw "Listener remains after launcher shutdown: $port" }
}
@{ installed = $exe; prerequisites = $true; registration = $true; shortcuts = $true; preservedExistingProjectFiles = $before.Count; launcherServices = 6; gracefulShutdown = $true; installerSha256 = (Get-FileHash -LiteralPath $installerPath -Algorithm SHA256).Hash.ToLowerInvariant() } | ConvertTo-Json | Set-Content (Join-Path $evidencePath 'installation-result.json')
'PASS: installed launcher starts six services with isolated test data and shuts them down gracefully.'
Write-Output "Evidence: $evidencePath"
