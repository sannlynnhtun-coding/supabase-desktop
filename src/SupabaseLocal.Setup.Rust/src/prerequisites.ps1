$ErrorActionPreference = 'Stop'
function Test-Runtime {
    foreach ($name in @('vcruntime140.dll', 'vcruntime140_1.dll', 'msvcp140.dll')) {
        $file = Join-Path ([Environment]::SystemDirectory) $name
        if (!(Test-Path -LiteralPath $file)) { return $false }
        $v = [Diagnostics.FileVersionInfo]::GetVersionInfo($file)
        if ($v.FileMajorPart -lt 14 -or ($v.FileMajorPart -eq 14 -and $v.FileMinorPart -lt 44)) { return $false }
    }
    return $true
}
if (Test-Runtime) { exit 0 }
$download = Join-Path ([IO.Path]::GetTempPath()) ('SupabaseLocal-vcredist-' + [Guid]::NewGuid().ToString('N') + '.exe')
try {
    Invoke-WebRequest 'https://aka.ms/vs/17/release/vc_redist.x64.exe' -OutFile $download -UseBasicParsing
    if ((Get-FileHash -LiteralPath $download -Algorithm SHA256).Hash -ne 'CC0FF0EB1DC3F5188AE6300FAEF32BF5BEEBA4BDD6E8E445A9184072096B713B') { throw 'Microsoft runtime download changed. Refresh the pinned package before installing.' }
    $process = Start-Process -FilePath $download -ArgumentList '/install /quiet /norestart' -Verb RunAs -WindowStyle Hidden -Wait -PassThru
    if ($process.ExitCode -eq 3010) { throw 'Restart Windows, then reopen setup.' }
    if ($process.ExitCode -notin @(0, 1638) -or !(Test-Runtime)) { throw 'Microsoft C++ runtime installation failed.' }
} finally { if (Test-Path -LiteralPath $download) { Remove-Item -LiteralPath $download } }
