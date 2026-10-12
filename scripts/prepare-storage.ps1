param([string]$Source = "$PSScriptRoot/../work/upstream/storage", [string]$Destination = "$PSScriptRoot/../work/runtime/storage")
$ErrorActionPreference = 'Stop'
$sourceRoot = [IO.Path]::GetFullPath($Source)
$targetRoot = [IO.Path]::GetFullPath($Destination)
if (!(Test-Path "$sourceRoot/dist/start/server.js")) { throw 'Build Storage before preparing its runtime.' }
New-Item -ItemType Directory -Force $targetRoot | Out-Null
foreach ($name in @('dist','migrations','patches','package.json','package-lock.json','LICENSE')) {
    Copy-Item -LiteralPath (Join-Path $sourceRoot $name) -Destination $targetRoot -Recurse -Force
}
Push-Location $targetRoot
try {
    # Windows npm reification is reliable with bounded download concurrency.
    & npm.cmd ci --omit=dev --ignore-scripts --no-audit --no-fund --maxsockets=4 --fetch-retries=1 --fetch-timeout=30000
    if ($LASTEXITCODE -ne 0) { throw 'Storage production dependencies failed' }
    & node "$sourceRoot/node_modules/patch-package/index.js"
    if ($LASTEXITCODE -ne 0) { throw 'Storage upstream dependency patch failed' }
} finally { Pop-Location }
