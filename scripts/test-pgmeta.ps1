param([string]$Postgres = "$PSScriptRoot/../work/runtime/pgsql", [string]$Source = "$PSScriptRoot/../work/upstream/postgres-meta", [string]$Node = "$PSScriptRoot/../work/runtime/node-v22.16.0-win-x64/node.exe", [string]$TestDirectory = "$PSScriptRoot/../work/pgmeta-smoke")
$ErrorActionPreference = 'Stop'
$testRoot = [IO.Path]::GetFullPath($TestDirectory)
New-Item -ItemType Directory -Force $testRoot | Out-Null
$randomBytes = New-Object byte[] 24
$rng = [Security.Cryptography.RandomNumberGenerator]::Create()
$rng.GetBytes($randomBytes)
$rng.Dispose()
$password = [Convert]::ToBase64String($randomBytes)
$passwordPath = Join-Path $testRoot 'password.txt'
$dbPath = Join-Path $testRoot 'db'
$pgStarted = $false
$metaProcess = $null
try {
    if (Test-Path "$dbPath/PG_VERSION") { throw 'Use a fresh smoke-test directory; an existing database will not be overwritten.' }
    [IO.File]::WriteAllText($passwordPath, $password)
    & "$Postgres/bin/initdb.exe" -D $dbPath -U postgres --pwfile $passwordPath --auth-host=scram-sha-256 --auth-local=scram-sha-256 --encoding=UTF8 --locale=C
    if ($LASTEXITCODE -ne 0) { throw 'initdb failed' }
    & "$Postgres/bin/pg_ctl.exe" -D $dbPath -l "$testRoot/postgres.log" -o '-p 55432 -h 127.0.0.1' -w start
    if ($LASTEXITCODE -ne 0) { throw 'PostgreSQL failed to start' }
    $pgStarted = $true
    $env:PG_META_HOST = '127.0.0.1'
    $env:PG_META_PORT = '55434'
    $env:PG_META_DB_HOST = '127.0.0.1'
    $env:PG_META_DB_PORT = '55432'
    $env:PG_META_DB_PASSWORD = $password
    $metaProcess = Start-Process ([IO.Path]::GetFullPath($Node)) -ArgumentList 'dist/server/server.js' -WorkingDirectory ([IO.Path]::GetFullPath($Source)) -WindowStyle Hidden -PassThru -RedirectStandardOutput "$testRoot/meta.out.log" -RedirectStandardError "$testRoot/meta.err.log"
    $ready = $false
    for ($attempt = 0; $attempt -lt 30; $attempt++) {
        if ($metaProcess.HasExited) { throw 'postgres-meta exited; inspect work/pgmeta-smoke/meta.err.log' }
        try { Invoke-RestMethod http://127.0.0.1:55434/health -TimeoutSec 2 | Out-Null; $ready = $true; break } catch { Start-Sleep -Milliseconds 500 }
    }
    if (!$ready) { throw 'postgres-meta health timed out' }
    $body = @{ query = 'CREATE TABLE public.windows_port_smoke(id integer PRIMARY KEY, name text); INSERT INTO public.windows_port_smoke VALUES (1, ''native Windows''); SELECT * FROM public.windows_port_smoke;' } | ConvertTo-Json
    $rows = Invoke-RestMethod http://127.0.0.1:55434/query -Method Post -ContentType 'application/json' -Body $body
    if ($rows[-1].name -ne 'native Windows') { throw 'SQL roundtrip failed' }
    $tables = Invoke-RestMethod http://127.0.0.1:55434/tables
    if (!($tables | Where-Object name -eq 'windows_port_smoke')) { throw 'Table discovery failed' }
    'PASS: native PostgreSQL init, postgres-meta health, SQL create/insert/select and table discovery.'
} finally {
    if ($metaProcess -and !$metaProcess.HasExited) { Stop-Process -Id $metaProcess.Id }
    if ($pgStarted) { & "$Postgres/bin/pg_ctl.exe" -D $dbPath -m fast -w stop }
    if (Test-Path $passwordPath) { Remove-Item -LiteralPath $passwordPath }
    Remove-Item Env:PG_META_DB_PASSWORD -ErrorAction SilentlyContinue
}
