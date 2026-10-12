param([string]$Postgres = "$PSScriptRoot/../work/runtime/pgsql", [string]$Source = "$PSScriptRoot/../work/upstream/postgres-meta", [string]$Node = "$PSScriptRoot/../work/runtime/node-v22.16.0-win-x64/node.exe", [string]$TestDirectory = "$PSScriptRoot/../work/pgmeta-smoke", [switch]$WithApiServices)
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
$authProcess = $null
$restProcess = $null
$originalPath = $env:PATH
function Wait-Healthy([string]$Url, $Process) {
    for ($attempt = 0; $attempt -lt 90; $attempt++) {
        if ($Process.HasExited) { throw "Service exited before health check: $Url" }
        try { Invoke-RestMethod $Url -TimeoutSec 2 | Out-Null; return } catch { Start-Sleep -Milliseconds 500 }
    }
    throw "Health timed out: $Url"
}
function Base64Url([byte[]]$Bytes) { [Convert]::ToBase64String($Bytes).TrimEnd('=').Replace('+','-').Replace('/','_') }
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
    if ($WithApiServices) {
        $env:PATH = [IO.Path]::GetFullPath("$Postgres/bin") + ';' + $env:PATH
        $env:PGPASSWORD = $password
        $env:SUPABASE_LOCAL_DATABASE_PASSWORD = $password
        & "$Postgres/bin/psql.exe" -h 127.0.0.1 -p 55432 -U postgres -d postgres -v ON_ERROR_STOP=1 -f "$PSScriptRoot/../src/SupabaseLocal/bootstrap.sql"
        if ($LASTEXITCODE -ne 0) { throw 'Database bootstrap failed' }
        $jwtSecret = $password + $password
        $env:GOTRUE_API_HOST = '127.0.0.1'
        $env:GOTRUE_API_PORT = '55438'
        $env:API_EXTERNAL_URL = 'http://127.0.0.1:55438'
        $env:GOTRUE_SITE_URL = 'http://localhost:3000'
        $env:GOTRUE_DB_DRIVER = 'postgres'
        $env:GOTRUE_DB_DATABASE_URL = "postgresql://supabase_auth_admin:$([Uri]::EscapeDataString($password))@127.0.0.1:55432/postgres"
        $env:GOTRUE_JWT_SECRET = $jwtSecret
        $env:GOTRUE_JWT_AUD = 'authenticated'
        $env:GOTRUE_JWT_DEFAULT_GROUP_NAME = 'authenticated'
        $env:GOTRUE_JWT_ADMIN_ROLES = 'service_role'
        $env:GOTRUE_EXTERNAL_EMAIL_ENABLED = 'true'
        $env:GOTRUE_MAILER_AUTOCONFIRM = 'true'
        $authProcess = Start-Process ([IO.Path]::GetFullPath("$PSScriptRoot/../work/runtime/auth/auth.exe")) -WorkingDirectory $testRoot -WindowStyle Hidden -PassThru -RedirectStandardOutput "$testRoot/auth.out.log" -RedirectStandardError "$testRoot/auth.err.log"
        Wait-Healthy http://127.0.0.1:55438/health $authProcess
        $credentials = @{email='windows-smoke@example.com'; password='Native-Windows-test-password-123!'} | ConvertTo-Json
        $signup = Invoke-RestMethod http://127.0.0.1:55438/signup -Method Post -ContentType 'application/json' -Body $credentials
        if (!$signup.user.id -or !$signup.access_token) { throw 'Auth signup did not issue a user/token' }
        $login = Invoke-RestMethod 'http://127.0.0.1:55438/token?grant_type=password' -Method Post -ContentType 'application/json' -Body $credentials
        if ($login.user.id -ne $signup.user.id) { throw 'Auth password login mismatch' }
        $env:PGRST_DB_URI = "postgresql://authenticator:$([Uri]::EscapeDataString($password))@127.0.0.1:55432/postgres"
        $env:PGRST_DB_SCHEMAS = 'public'
        $env:PGRST_DB_ANON_ROLE = 'anon'
        $env:PGRST_JWT_SECRET = $jwtSecret
        $env:PGRST_SERVER_HOST = '127.0.0.1'
        $env:PGRST_SERVER_PORT = '55436'
        $restProcess = Start-Process ([IO.Path]::GetFullPath("$PSScriptRoot/../work/runtime/postgrest/postgrest.exe")) -WorkingDirectory $testRoot -WindowStyle Hidden -PassThru -RedirectStandardOutput "$testRoot/rest.out.log" -RedirectStandardError "$testRoot/rest.err.log"
        Wait-Healthy http://127.0.0.1:55436/ $restProcess
        $query = @{query="CREATE TABLE public.windows_rls_smoke(id int PRIMARY KEY, owner uuid); ALTER TABLE public.windows_rls_smoke ENABLE ROW LEVEL SECURITY; GRANT SELECT ON public.windows_rls_smoke TO anon, authenticated; GRANT ALL ON public.windows_rls_smoke TO service_role; CREATE POLICY own_rows ON public.windows_rls_smoke FOR SELECT TO authenticated USING (owner=auth.uid()); INSERT INTO public.windows_rls_smoke VALUES (1, '$($signup.user.id)'); NOTIFY pgrst, 'reload schema';"} | ConvertTo-Json
        Invoke-RestMethod http://127.0.0.1:55434/query -Method Post -ContentType 'application/json' -Body $query | Out-Null
        Start-Sleep -Seconds 2
        $ownRows = Invoke-RestMethod http://127.0.0.1:55436/windows_rls_smoke -Headers @{Authorization="Bearer $($login.access_token)"}
        if ($ownRows.Count -ne 1 -or $ownRows[0].id -ne 1) { throw 'Authenticated RLS read failed' }
        $anonRows = Invoke-RestMethod http://127.0.0.1:55436/windows_rls_smoke
        if ($anonRows.Count -ne 0) { throw 'Anonymous RLS isolation failed' }
        $header = Base64Url ([Text.Encoding]::UTF8.GetBytes('{"alg":"HS256","typ":"JWT"}'))
        $claims = Base64Url ([Text.Encoding]::UTF8.GetBytes('{"role":"service_role","exp":4102444800}'))
        $hmac = New-Object Security.Cryptography.HMACSHA256
        $hmac.Key = [Text.Encoding]::UTF8.GetBytes($jwtSecret)
        $serviceToken = "$header.$claims.$(Base64Url ($hmac.ComputeHash([Text.Encoding]::ASCII.GetBytes("$header.$claims"))))"
        $hmac.Dispose()
        $insert = @{id=2;owner='00000000-0000-0000-0000-000000000001'} | ConvertTo-Json
        Invoke-RestMethod http://127.0.0.1:55436/windows_rls_smoke -Method Post -ContentType 'application/json' -Headers @{Authorization="Bearer $serviceToken"} -Body $insert | Out-Null
        $ownRows = Invoke-RestMethod http://127.0.0.1:55436/windows_rls_smoke -Headers @{Authorization="Bearer $($login.access_token)"}
        if ($ownRows.Count -ne 1) { throw 'Cross-user RLS isolation failed' }
        'PASS: upstream Auth migrations, signup, password login; PostgREST JWT validation, service-role insert and anonymous/cross-user RLS isolation.'
    }
} finally {
    if ($restProcess -and !$restProcess.HasExited) { Stop-Process -Id $restProcess.Id }
    if ($authProcess -and !$authProcess.HasExited) { Stop-Process -Id $authProcess.Id }
    if ($metaProcess -and !$metaProcess.HasExited) { Stop-Process -Id $metaProcess.Id }
    if ($pgStarted) { & "$Postgres/bin/pg_ctl.exe" -D $dbPath -m fast -w stop }
    if (Test-Path $passwordPath) { Remove-Item -LiteralPath $passwordPath }
    Remove-Item Env:PG_META_DB_PASSWORD -ErrorAction SilentlyContinue
    $env:PATH = $originalPath
}
