param(
    [Parameter(Mandatory=$true)][string]$Payload,
    [string]$Output = "$PSScriptRoot/../artifacts/installer"
)
$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath("$PSScriptRoot/..")
$payloadPath = [IO.Path]::GetFullPath($Payload)
$outputPath = [IO.Path]::GetFullPath($Output)
if (!(Test-Path -LiteralPath $payloadPath)) { throw 'Payload archive is missing.' }
$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if (!$cargo) { $cargo = Get-Command "$env:USERPROFILE/.cargo/bin/cargo.exe" }
& $cargo.Source build --release --locked --manifest-path "$repo/src/SupabaseLocal.Setup.Rust/Cargo.toml"
if ($LASTEXITCODE -ne 0) { throw 'Rust release build failed.' }
New-Item -ItemType Directory -Force -Path $outputPath | Out-Null
$installer = Join-Path $outputPath 'SupabaseLocal.Setup.exe'
$temporary = Join-Path $outputPath ('SupabaseLocal.Setup.build-' + [Guid]::NewGuid().ToString('N'))
$outputStream = [IO.File]::Open($temporary, [IO.FileMode]::CreateNew)
try {
    $stub = [IO.File]::OpenRead("$repo/src/SupabaseLocal.Setup.Rust/target/release/supabase-local-setup.exe")
    try { $stub.CopyTo($outputStream) } finally { $stub.Dispose() }
    $payloadStream = [IO.File]::OpenRead($payloadPath)
    try {
        $sha = [Security.Cryptography.SHA256]::Create()
        try { $digest = $sha.ComputeHash($payloadStream) } finally { $sha.Dispose() }
        $payloadStream.Position = 0
        $payloadStream.CopyTo($outputStream)
        $magic = [Text.Encoding]::ASCII.GetBytes('SBL_PAYLOAD_V001')
        $length = [BitConverter]::GetBytes([UInt64]$payloadStream.Length)
        $outputStream.Write($magic, 0, $magic.Length)
        $outputStream.Write($length, 0, $length.Length)
        $outputStream.Write($digest, 0, $digest.Length)
    } finally { $payloadStream.Dispose() }
} finally { $outputStream.Dispose() }
Move-Item -LiteralPath $temporary -Destination $installer -Force
$hash = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
[IO.File]::WriteAllText("$installer.sha256", "$hash  SupabaseLocal.Setup.exe`n")
Write-Output "Bundled Rust installer: $installer"
