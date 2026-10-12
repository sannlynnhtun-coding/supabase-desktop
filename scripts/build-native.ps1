param([switch]$SkipStudioBuild)
$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath("$PSScriptRoot/..")
Set-Location $repo
$lock = Get-Content "$PSScriptRoot/sources.lock.json" -Raw | ConvertFrom-Json
function Check-Exit([string]$Task) { if ($LASTEXITCODE -ne 0) { throw "$Task failed ($LASTEXITCODE)" } }
function Source([string]$Name, $Pin) {
    $path = Join-Path $repo "work/upstream/$Name"
    if (!(Test-Path "$path/.git")) {
        New-Item -ItemType Directory -Force $path | Out-Null
        & git -C $path init | Out-Null; Check-Exit 'git init'
        & git -C $path remote add origin $Pin.repository | Out-Null; Check-Exit 'git remote'
        & git -C $path fetch --depth 1 origin $Pin.revision | Out-Null; Check-Exit 'source fetch'
        & git -C $path checkout --detach FETCH_HEAD | Out-Null; Check-Exit 'source checkout'
    }
    $revision = & git -C $path rev-parse HEAD; Check-Exit 'source revision'
    if ($revision -ne $Pin.revision) { throw "Unexpected revision in $path; expected $($Pin.revision)." }
    return $path
}
function Archive([string]$Name, $Pin, [string]$Destination, [string]$Marker) {
    New-Item -ItemType Directory -Force "$repo/work/downloads" | Out-Null
    $zip = Join-Path $repo "work/downloads/$Name.zip"
    if (!(Test-Path $zip)) { Invoke-WebRequest $Pin.url -OutFile $zip }
    if ((Get-FileHash $zip -Algorithm SHA256).Hash -ne $Pin.sha256) { throw "Checksum mismatch: $zip" }
    if (!(Test-Path $Marker)) { Expand-Archive -LiteralPath $zip -DestinationPath $Destination -Force }
}
foreach ($tool in @('git','node','dotnet','npm.cmd')) { Get-Command $tool -ErrorAction Stop | Out-Null }
$studio = Source 'supabase' $lock.studio
$meta = Source 'postgres-meta' $lock.pgmeta
$auth = Source 'auth' $lock.auth
Archive 'node' $lock.downloads.node "$repo/work/runtime" "$repo/work/runtime/node-v22.16.0-win-x64/node.exe"
Archive 'postgres' $lock.downloads.postgres "$repo/work/runtime" "$repo/work/runtime/pgsql/bin/postgres.exe"
Archive 'postgrest' $lock.downloads.postgrest "$repo/work/runtime/postgrest" "$repo/work/runtime/postgrest/postgrest.exe"
Archive 'go' $lock.downloads.go "$repo/work/toolchains" "$repo/work/toolchains/go/bin/go.exe"
Push-Location $meta
try { & npm.cmd ci --no-audit --no-fund; Check-Exit 'postgres-meta dependencies' } finally { Pop-Location }
& node scripts/build-pgmeta.mjs; Check-Exit 'postgres-meta Windows build'
& node scripts/patch-auth.mjs; Check-Exit 'Auth Windows patch'
$env:CGO_ENABLED = '0'
$env:GOCACHE = "$repo/work/go-cache"
$env:GOMODCACHE = "$repo/work/go-modules"
New-Item -ItemType Directory -Force "$repo/work/runtime/auth" | Out-Null
Push-Location $auth
try {
    & "$repo/work/toolchains/go/bin/go.exe" fmt ./cmd; Check-Exit 'Auth format'
    & "$repo/work/toolchains/go/bin/go.exe" build -trimpath -ldflags '-X github.com/supabase/auth/internal/utilities.Version=v2.197.0-windows' -o "$repo/work/runtime/auth/auth.exe" .; Check-Exit 'Auth build'
} finally { Pop-Location }
if (!$SkipStudioBuild) {
    & npm.cmd install --prefix "$repo/work/toolchains/pnpm" pnpm@11.13.1 --ignore-scripts --no-audit --no-fund; Check-Exit 'pnpm build tool'
    & node "$repo/work/toolchains/pnpm/node_modules/pnpm/bin/pnpm.cjs" --dir $studio --filter studio... install --frozen-lockfile --ignore-scripts; Check-Exit 'Studio dependencies'
    & node scripts/build-studio.mjs; Check-Exit 'Studio Windows build'
}
$runtime = "artifacts/runtime-" + [Guid]::NewGuid().ToString('N')
& node scripts/assemble-runtime.mjs $runtime; Check-Exit 'runtime assembly'
& "$PSScriptRoot/build-installer.ps1" -Runtime ([IO.Path]::GetFullPath($runtime))
