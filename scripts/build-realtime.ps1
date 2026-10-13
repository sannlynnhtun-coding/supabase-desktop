$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath("$PSScriptRoot/..")
$lock = Get-Content "$PSScriptRoot/sources.lock.json" -Raw | ConvertFrom-Json
$source = Join-Path $repo 'work/upstream/realtime'
function Check-Exit([string]$Task) { if ($LASTEXITCODE -ne 0) { throw "$Task failed ($LASTEXITCODE)" } }
function Toolchain([string]$Name, $Pin, [string]$Marker, [string]$Directory) {
    $archive = Join-Path $repo "work/downloads/$Name.zip"
    $target = Join-Path $repo "work/toolchains/$Directory"
    New-Item -ItemType Directory -Force "$repo/work/downloads" | Out-Null
    if (!(Test-Path $archive)) { Invoke-WebRequest $Pin.url -OutFile $archive }
    if ((Get-FileHash $archive).Hash -ne $Pin.sha256) { throw "$Name checksum mismatch" }
    if (!(Test-Path (Join-Path $target $Marker))) { Expand-Archive $archive $target -Force }
    return $target
}
if (!(Test-Path "$source/.git")) {
    & git clone --no-checkout $lock.realtime.repository $source; Check-Exit 'Realtime source clone'
    & git -C $source checkout --detach $lock.realtime.revision; Check-Exit 'Realtime source checkout'
}
$revision = & git -C $source rev-parse HEAD; Check-Exit 'Realtime revision'
if ($revision -ne $lock.realtime.revision) { throw 'Realtime source revision differs from its pin' }
$erlang = Toolchain 'erlang' $lock.downloads.erlang 'bin/erl.exe' 'erlang27'
$elixir = Toolchain 'elixir' $lock.downloads.elixir 'bin/elixir.bat' 'elixir18'
$savedEnvironment = @{}
foreach ($name in @('PATH','ERL_FLAGS','MIX_HOME','HEX_HOME','MIX_ENV')) { $savedEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
Push-Location $source
try {
    $env:PATH = "$erlang/bin;$elixir/bin;" + $env:PATH
    $env:ERL_FLAGS = '+S 4:4'
    $env:MIX_HOME = "$repo/work/mix-home"
    $env:HEX_HOME = "$repo/work/hex-home"
    $env:MIX_ENV = 'prod'
    & elixir.bat --version; Check-Exit 'Windows Erlang/Elixir startup'
    & mix.bat local.hex 2.5.1 --force; Check-Exit 'Hex build tool'
    $rebar = "$repo/work/downloads/rebar3"
    if (!(Test-Path $rebar)) { Invoke-WebRequest $lock.downloads.rebar.url -OutFile $rebar }
    if ((Get-FileHash $rebar).Hash -ne $lock.downloads.rebar.sha256) { throw 'Rebar checksum mismatch' }
    & mix.bat local.rebar rebar3 $rebar --force; Check-Exit 'Rebar build tool'
    & mix.bat deps.get --only prod; Check-Exit 'Realtime locked dependencies'
    & git diff --exit-code -- mix.lock; Check-Exit 'Realtime dependency lock changed'
    & mix.bat compile; Check-Exit 'Realtime Windows compilation'
    & mix.bat release --overwrite; Check-Exit 'Realtime Windows release'
    Write-Output "Native release: $source/_build/prod/rel/realtime (runtime integration is not yet validated)"
} finally {
    Pop-Location
    foreach ($name in $savedEnvironment.Keys) { [Environment]::SetEnvironmentVariable($name, $savedEnvironment[$name], 'Process') }
}
