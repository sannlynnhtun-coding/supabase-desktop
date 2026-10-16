param([string]$Runtime = "$PSScriptRoot/../artifacts/runtime")
$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath("$PSScriptRoot/..")
$runtimePath = [IO.Path]::GetFullPath($Runtime)
if (!(Test-Path "$runtimePath/studio/server.js")) { throw 'Build and assemble the native runtime first.' }
$buildId = [Guid]::NewGuid().ToString('N')
$payloadRoot = Join-Path $repo "work/package-$buildId"
$zip = Join-Path $repo "work/payload-$buildId.zip"
$output = Join-Path $repo 'artifacts/installer'
& dotnet publish "$repo/src/SupabaseLocal/SupabaseLocal.csproj" -c Release -r win-x64 --self-contained true -o $payloadRoot
if ($LASTEXITCODE -ne 0) { throw 'Manager publish failed' }
$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if (!$cargo) { $cargo = Get-Command "$env:USERPROFILE/.cargo/bin/cargo.exe" }
& $cargo.Source build --release --locked --manifest-path "$repo/src/SupabaseLocal.Setup.Rust/Cargo.toml" --bins
if ($LASTEXITCODE -ne 0) { throw 'Rust desktop app build failed' }
Copy-Item -LiteralPath "$repo/src/SupabaseLocal.Setup.Rust/target/release/supabase-local-app.exe" -Destination (Join-Path $payloadRoot 'SupabaseLocal.App.exe')
Copy-Item -LiteralPath "$repo/src/SupabaseLocal.Setup.Rust/target/release/supabase-desktop-uninstall.exe" -Destination (Join-Path $payloadRoot 'SupabaseLocal.Uninstall.exe')
& node "$repo/scripts/copy-runtime.mjs" $runtimePath "$payloadRoot/runtime"
if ($LASTEXITCODE -ne 0) { throw 'Runtime payload copy failed' }
& node "$repo/scripts/create-manifest.mjs" $payloadRoot
if ($LASTEXITCODE -ne 0) { throw 'Payload manifest generation failed' }
& dotnet run --project "$repo/tools/PayloadArchive" -c Release -- $payloadRoot $zip
if ($LASTEXITCODE -ne 0) { throw 'Verified payload archive failed' }
& "$PSScriptRoot/package-rust-installer.ps1" -Payload $zip -Output $output
if ($LASTEXITCODE -ne 0) { throw 'Rust installer packaging failed' }
$installer = Join-Path $output 'SupabaseDesktop.Setup.exe'
$hash = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
[IO.File]::WriteAllText("$installer.sha256", "$hash  SupabaseDesktop.Setup.exe`n")
Write-Output "Installer: $installer"
