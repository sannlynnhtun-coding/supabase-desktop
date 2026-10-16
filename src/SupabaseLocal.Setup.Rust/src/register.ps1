param(
    [string]$Version,
    [string]$Root,
    [string]$Programs = (Join-Path ([Environment]::GetFolderPath('Programs')) 'Supabase Local'),
    [string]$RegistryPath = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\SupabaseLocal'
)
$ErrorActionPreference = 'Stop'
$exe = Join-Path $Version 'SupabaseLocal.App.exe'
if (!(Test-Path -LiteralPath $exe)) { throw 'Verified desktop app is missing.' }
New-Item -ItemType Directory -Force -Path $programs | Out-Null
$shell = New-Object -ComObject WScript.Shell
foreach ($item in @(@('Supabase Local', ''), @('Uninstall Supabase Local', '--uninstall'))) {
    $shortcut = $shell.CreateShortcut((Join-Path $programs ($item[0] + '.lnk')))
    $shortcut.TargetPath = $exe
    $shortcut.Arguments = $item[1]
    $shortcut.WorkingDirectory = $Version
    $shortcut.IconLocation = $exe
    $shortcut.Save()
}
$key = $RegistryPath
New-Item -Path $key -Force | Out-Null
$values = @{
    InstallerOwner = 'SupabaseLocal.NativeWindows.v1'
    DisplayName = 'Supabase Local'
    DisplayVersion = '0.3.0'
    Publisher = 'Supabase Local community port'
    InstallLocation = $Root
    DisplayIcon = $exe
    UninstallString = ('"' + $exe + '" --uninstall')
    URLInfoAbout = 'https://github.com/sannlynnhtun-coding/supabase-local-installer-with-rust'
}
foreach ($name in $values.Keys) { New-ItemProperty -Path $key -Name $name -Value $values[$name] -PropertyType String -Force | Out-Null }
foreach ($name in @('NoModify', 'NoRepair')) { New-ItemProperty -Path $key -Name $name -Value 1 -PropertyType DWord -Force | Out-Null }
