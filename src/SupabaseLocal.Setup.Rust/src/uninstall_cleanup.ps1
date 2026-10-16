param([int]$WorkerId, [string]$Folder)
$ErrorActionPreference = 'Stop'
Wait-Process -Id $WorkerId -ErrorAction SilentlyContinue
$full = [IO.Path]::GetFullPath($Folder).TrimEnd('\')
$temp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')
if (![string]::Equals([IO.Path]::GetDirectoryName($full), $temp, [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe cleanup parent' }
if ([IO.Path]::GetFileName($full) -cnotmatch '^SupabaseDesktop-Uninstall-[a-f0-9]{32}$') { throw 'Unsafe cleanup directory' }
for ($item = Get-Item -LiteralPath $full -Force; $null -ne $item; $item = $item.Parent) {
    if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Cleanup path contains a reparse point' }
}
# The private worker contains only regular files; reject added junctions before recursive removal.
foreach ($item in Get-ChildItem -LiteralPath $full -Force) {
    if ($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Unexpected cleanup entry' }
}
Remove-Item -LiteralPath $full -Recurse -Force
