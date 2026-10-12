param([Parameter(Mandatory=$true)][string]$Manager, [Parameter(Mandatory=$true)][string]$Runtime, [string]$Data = "$PSScriptRoot/../work/manager-integration", [ValidateRange(2,10)][int]$Cycles = 2, [switch]$WithStorage)
$ErrorActionPreference = 'Stop'
$managerPath = [IO.Path]::GetFullPath($Manager)
$runtimePath = [IO.Path]::GetFullPath($Runtime)
$dataPath = [IO.Path]::GetFullPath($Data)
New-Item -ItemType Directory -Force $dataPath | Out-Null
$env:RuntimeRoot = $runtimePath
$env:DataRoot = $dataPath
$env:LauncherToken = [Guid]::NewGuid().ToString('N')
function Key([string]$Secret) {
    function Encode([byte[]]$bytes) { [Convert]::ToBase64String($bytes).TrimEnd('=').Replace('+','-').Replace('/','_') }
    $header = Encode ([Text.Encoding]::UTF8.GetBytes('{"alg":"HS256","typ":"JWT"}'))
    $claims = Encode ([Text.Encoding]::UTF8.GetBytes('{"role":"anon","iss":"supabase","iat":0,"exp":4102444800}'))
    $hmac = New-Object Security.Cryptography.HMACSHA256
    $hmac.Key = [Text.Encoding]::UTF8.GetBytes($Secret)
    try { return "$header.$claims.$(Encode ($hmac.ComputeHash([Text.Encoding]::ASCII.GetBytes("$header.$claims"))))" } finally { $hmac.Dispose() }
}
$initialSecretsHash = $null
$email = [Guid]::NewGuid().ToString('N') + '@example.com'
$password = [Guid]::NewGuid().ToString('N') + '-Native123!'
for ($cycle = 1; $cycle -le $Cycles; $cycle++) {
    $process = Start-Process $managerPath -WorkingDirectory ([IO.Path]::GetDirectoryName($managerPath)) -WindowStyle Hidden -PassThru -RedirectStandardOutput "$dataPath/manager-$cycle.out.log" -RedirectStandardError "$dataPath/manager-$cycle.err.log"
    try {
        $ready = $false
        for ($attempt = 0; $attempt -lt 600; $attempt++) {
            if ($process.HasExited) { throw 'Manager exited before readiness' }
            try {
                $status = Invoke-RestMethod http://127.0.0.1:54320/api/status -TimeoutSec 2
                if ($status.state -eq 'failed') { throw "Stack startup failed: $($status.message)" }
                if ($status.studioUrl) { $ready=$true; break }
            } catch [System.Net.WebException] { }
            Start-Sleep -Milliseconds 500
        }
        if (!$ready) { throw 'Manager readiness timed out' }
        foreach ($service in @('PostgreSQL','postgres-meta','Studio','PostgREST','Auth')) { if ($status.available -notcontains $service) { throw "Missing service: $service" } }
        $secretsHash = (Get-FileHash "$dataPath/secrets.json").Hash
        if ($initialSecretsHash -and $initialSecretsHash -ne $secretsHash) { throw 'Restart changed project secrets' }
        $initialSecretsHash = $secretsHash
        $secrets = Get-Content "$dataPath/secrets.json" -Raw | ConvertFrom-Json
        $headers = @{apikey=(Key $secrets.JwtSecret)}
        $query = if ($cycle -eq 1) { 'CREATE TABLE IF NOT EXISTS public.native_manager_persistence(id integer PRIMARY KEY, message text); INSERT INTO public.native_manager_persistence VALUES(1, ''survives restart'') ON CONFLICT(id) DO NOTHING; SELECT message FROM public.native_manager_persistence;' } else { 'SELECT message FROM public.native_manager_persistence;' }
        $rows = Invoke-RestMethod http://127.0.0.1:54323/api/platform/pg-meta/default/query -Method Post -ContentType 'application/json' -Body (@{query=$query}|ConvertTo-Json) -TimeoutSec 10
        if ($rows[-1].message -ne 'survives restart') { throw 'Studio SQL persistence failed' }
        $credentials = @{email=$email;password=$password} | ConvertTo-Json
        if ($cycle -eq 1) {
            $signup = Invoke-RestMethod http://127.0.0.1:54321/auth/v1/signup -Headers $headers -Method Post -ContentType 'application/json' -Body $credentials -TimeoutSec 10
            if (!$signup.user.id) { throw 'Gateway Auth signup failed' }
        }
        $login = Invoke-RestMethod 'http://127.0.0.1:54321/auth/v1/token?grant_type=password' -Headers $headers -Method Post -ContentType 'application/json' -Body $credentials -TimeoutSec 10
        if (!$login.access_token) { throw 'Gateway Auth login failed' }
        if ($WithStorage) {
            if ($status.available -notcontains 'Storage') { throw 'Storage is unavailable' }
            $env:STORAGE_TEST_ACCESS_TOKEN = $login.access_token
            & node "$PSScriptRoot/test-storage-api.mjs" $dataPath $cycle
            if ($LASTEXITCODE -ne 0) { throw 'Storage integration checks failed' }
            Remove-Item Env:STORAGE_TEST_ACCESS_TOKEN
        }
        Invoke-RestMethod http://127.0.0.1:54321/rest/v1/ -Headers $headers -TimeoutSec 10 | Out-Null
        try { Invoke-WebRequest http://127.0.0.1:54321/rest/v1/ -UseBasicParsing -TimeoutSec 3 | Out-Null; throw 'Gateway accepted a missing API key' } catch [System.Net.WebException] { if ([int]$_.Exception.Response.StatusCode -ne 401) { throw } }
        "PASS: manager cycle $cycle, real Studio SQL, gateway Auth/REST, missing-key rejection and stable secrets."
    } finally {
        if (!$process.HasExited) {
            Invoke-RestMethod http://127.0.0.1:54320/api/shutdown -Method Post -Headers @{'X-Launcher-Token'=$env:LauncherToken} -TimeoutSec 5 | Out-Null
            if (!$process.WaitForExit(90000)) { throw 'Graceful manager shutdown timed out' }
        }
        $process.Dispose()
    }
    foreach ($port in @(54320,54321,54322,54323,54324,54325,54326,54328,54330,54331)) {
        if (Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue) { throw "Service left listening after shutdown: $port" }
    }
    'PASS: all owned listeners stopped without deleting the database.'
}
