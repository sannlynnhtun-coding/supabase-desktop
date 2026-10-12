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
& node "$repo/scripts/copy-runtime.mjs" $runtimePath "$payloadRoot/runtime"
if ($LASTEXITCODE -ne 0) { throw 'Runtime payload copy failed' }
& node "$repo/scripts/create-manifest.mjs" $payloadRoot
if ($LASTEXITCODE -ne 0) { throw 'Payload manifest generation failed' }
Add-Type -AssemblyName System.IO.Compression.FileSystem
[IO.Compression.ZipFile]::CreateFromDirectory($payloadRoot, $zip, [IO.Compression.CompressionLevel]::Optimal, $false)
& dotnet publish "$repo/src/SupabaseLocal.Setup/SupabaseLocal.Setup.csproj" -c Release -r win-x64 --self-contained true -p:PublishSingleFile=true -p:IncludeNativeLibrariesForSelfExtract=true "-p:PayloadPath=$zip" -o $output
if ($LASTEXITCODE -ne 0) { throw 'Installer publish failed' }
$installer = Join-Path $output 'SupabaseLocal.Setup.exe'
$hash = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
[IO.File]::WriteAllText("$installer.sha256", "$hash  SupabaseLocal.Setup.exe`n")
Write-Output "Installer: $installer"
