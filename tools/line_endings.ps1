<#
.SYNOPSIS
Gives every text file git tracks in a checkout Unix line endings (LF), as
.gitattributes keeps them, without changing what git sees in them.

.DESCRIPTION
Git for Windows checks text files out with Windows line endings (CRLF) unless
.gitattributes says otherwise, and a checkout made before it did still holds
them. Tools that write files then mix the two: cargo fmt, sed -i, and scripts
write LF into a CRLF checkout. This rewrites each tracked text file that git
finds CRLF in (git ls-files --eol) with LF alone. Git stores LF already, so
the files read the same to git, uncommitted changes and all; only their bytes
on disk change. verify.ps1 runs it before its checks.

.PARAMETER Path
The checkout to switch; this repository's own by default. Any git checkout
works, such as the fighting game's.

.PARAMETER Check
List the files with Windows line endings and exit 1 if there are any, without
changing them.

.EXAMPLE
.\tools\line_endings.ps1

.EXAMPLE
.\tools\line_endings.ps1 -Path E:\PersonalProjects\realistic_physics_fight -Check
#>
param(
    [string]$Path = "",
    [switch]$Check
)

. "$PSScriptRoot\common.ps1"

$root = if ($Path) { (Resolve-Path -LiteralPath $Path).Path } else { Get-RepoRoot }
# Each entry reads "i/<index> w/<worktree> attr/<attributes>`t<path>"; git
# marks a file it takes for binary "-text" and leaves it alone.
$entries = (& git -C $root ls-files --eol -z) -split "`0" | Where-Object { $_ }
if ($LASTEXITCODE -ne 0) {
    throw "git ls-files failed in $root"
}
$crlf = foreach ($entry in $entries) {
    $info, $file = $entry -split "`t", 2
    if ($info -match "\bw/(crlf|mixed)\b") {
        $file
    }
}
$crlf = @($crlf)
if ($Check) {
    if ($crlf.Count -eq 0) {
        Write-Host "Every text file in $root has Unix line endings."
        exit 0
    }
    Write-Host "$($crlf.Count) files in $root have Windows line endings:"
    $crlf | ForEach-Object { Write-Host "  $_" }
    exit 1
}
if ($crlf.Count -eq 0) {
    Write-Host "Every text file in $root has Unix line endings."
    exit 0
}
# Files with uncommitted changes, which must stay unstaged.
$changed = @((& git -C $root diff --name-only -z) -split "`0" | Where-Object { $_ })
# Latin-1 maps every byte to one character and back, so the files' other
# bytes (UTF-8 text, a byte order mark) come back exactly as they were.
$latin1 = [System.Text.Encoding]::Latin1
foreach ($file in $crlf) {
    $full = Join-Path $root $file
    $text = $latin1.GetString([IO.File]::ReadAllBytes($full))
    # Only a CR before an LF goes; a lone CR stays.
    [IO.File]::WriteAllBytes($full, $latin1.GetBytes($text.Replace("`r`n", "`n")))
}
# Git keeps each file's size from when it last read it and takes a file of
# another size for changed without reading it, so it would list every switched
# file as changed. It reads the unchanged ones again: their content is what
# git holds already, so this stages nothing. A file with uncommitted changes
# is changed anyway and is left alone.
$unchanged = @($crlf | Where-Object { $changed -notcontains $_ })
for ($i = 0; $i -lt $unchanged.Count; $i += 100) {
    $batch = $unchanged[$i..([Math]::Min($i + 99, $unchanged.Count - 1))]
    # Without .gitattributes (a branch made before it), core.autocrlf would
    # warn of each file that it holds LF.
    & git -C $root -c core.safecrlf=false update-index -q -- @batch
    if ($LASTEXITCODE -ne 0) {
        throw "git update-index failed in $root"
    }
}
Write-Host "Switched $($crlf.Count) files in $root to Unix line endings."
