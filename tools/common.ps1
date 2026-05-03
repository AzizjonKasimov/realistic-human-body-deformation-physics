$ErrorActionPreference = "Stop"

function Get-RepoRoot {
    return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

function Get-CMakePath {
    $fromPath = Get-Command cmake -ErrorAction SilentlyContinue
    if ($fromPath) {
        return $fromPath.Source
    }

    $candidates = @(
        "C:\Program Files\CMake\bin\cmake.exe",
        "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe",
        "C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe"
    )

    foreach ($candidate in $candidates) {
        if (Test-Path $candidate) {
            return $candidate
        }
    }

    throw "Could not find cmake.exe. Install CMake or update tools/common.ps1 with the local CMake path."
}

function Invoke-Checked {
    param(
        [Parameter(Mandatory = $true)]
        [scriptblock]$Command,
        [Parameter(Mandatory = $true)]
        [string]$Label
    )

    Write-Host ""
    Write-Host "==> $Label"
    $global:LASTEXITCODE = 0
    $output = & $Command 2>&1
    foreach ($line in $output) {
        Write-Host $line
    }
    $exitCode = if ($null -eq $LASTEXITCODE) { 0 } else { $LASTEXITCODE }
    if ($exitCode -ne 0) {
        throw "$Label failed with exit code $exitCode"
    }
}

function Invoke-CmdChecked {
    param(
        [Parameter(Mandatory = $true)]
        [string]$CommandLine,
        [Parameter(Mandatory = $true)]
        [string]$Label
    )

    Write-Host ""
    Write-Host "==> $Label"
    $outputFile = [System.IO.Path]::GetTempFileName()
    try {
        & "$env:SystemRoot\System32\cmd.exe" /d /c "$CommandLine > `"$outputFile`" 2>&1"
        $exitCode = if ($null -eq $LASTEXITCODE) { 0 } else { $LASTEXITCODE }
        Get-Content $outputFile | ForEach-Object { Write-Host $_ }
        if ($exitCode -ne 0) {
            throw "$Label failed with exit code $exitCode"
        }
        Write-Host "OK: $Label"
    } finally {
        Remove-Item $outputFile -ErrorAction SilentlyContinue
    }
}

function Initialize-WindowsBuild {
    param(
        [string]$Generator = "Visual Studio 17 2022",
        [string]$Architecture = "x64",
        [switch]$Force
    )

    $repoRoot = Get-RepoRoot
    $cmake = Get-CMakePath
    $cache = Join-Path $repoRoot "build\vs\CMakeCache.txt"
    if ($Force -or -not (Test-Path $cache)) {
        Invoke-CmdChecked `
            -Label "Configure Visual Studio build" `
            -CommandLine "cd /d `"$repoRoot`" && `"$cmake`" -S . -B build\vs -G `"$Generator`" -A $Architecture"
    }
}

function Invoke-WindowsTarget {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Target,
        [string]$Config = "Debug"
    )

    $repoRoot = Get-RepoRoot
    $cmake = Get-CMakePath
    Invoke-CmdChecked `
        -Label "Build $Target ($Config)" `
        -CommandLine "cd /d `"$repoRoot`" && `"$cmake`" --build build\vs --config $Config --target $Target"
}

function Invoke-ExeViaCmd {
    param(
        [Parameter(Mandatory = $true)]
        [string]$ExePath,
        [string[]]$Arguments = @(),
        [Parameter(Mandatory = $true)]
        [string]$Label
    )

    $repoRoot = Get-RepoRoot
    $argumentText = if ($Arguments.Count -gt 0) { " " + ($Arguments -join " ") } else { "" }
    Invoke-CmdChecked `
        -Label $Label `
        -CommandLine "cd /d `"$repoRoot`" && `"$ExePath`"$argumentText"
}

function Stop-RunningAppIfRequested {
    param(
        [switch]$StopRunningApp
    )

    $running = Get-Process realistic_physics -ErrorAction SilentlyContinue
    if (-not $running) {
        return
    }

    if (-not $StopRunningApp) {
        $ids = ($running | Select-Object -ExpandProperty Id) -join ", "
        throw "realistic_physics.exe is running (PID $ids). Close it or rerun with -StopRunningApp so the linker can replace the root executable."
    }

    $running | Stop-Process -Force
    Write-Host "Stopped running realistic_physics.exe before rebuild."
}
