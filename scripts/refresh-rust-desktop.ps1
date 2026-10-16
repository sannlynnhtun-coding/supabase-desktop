param(
    [Parameter(Mandatory=$true)][string]$Payload,
    [string]$Output = "$PSScriptRoot/../artifacts/installer/rust-desktop"
)
$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath("$PSScriptRoot/..")
$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if (!$cargo) { $cargo = Get-Command "$env:USERPROFILE/.cargo/bin/cargo.exe" }
& $cargo.Source build --release --locked --manifest-path "$repo/src/SupabaseLocal.Setup.Rust/Cargo.toml"
if ($LASTEXITCODE -ne 0) { throw 'Rust application build failed' }
$zip = Join-Path $repo ('work/payload-rust-desktop-' + [Guid]::NewGuid().ToString('N') + '.zip')
& dotnet run --project "$repo/tools/PayloadArchive" -c Release -- --refresh-branding ([IO.Path]::GetFullPath($Payload)) "$repo/src/SupabaseLocal.Setup.Rust/target/release/supabase-local-app.exe" "$repo/src/SupabaseLocal.Setup.Rust/target/release/supabase-desktop-uninstall.exe" "$repo/src/SupabaseLocal/wwwroot/index.html" $zip
if ($LASTEXITCODE -ne 0) { throw 'Runtime desktop refresh failed' }
& "$PSScriptRoot/package-rust-installer.ps1" -Payload $zip -Output $Output
if ($LASTEXITCODE -ne 0) { throw 'Rust desktop installer packaging failed' }
Write-Output "Payload: $zip"
