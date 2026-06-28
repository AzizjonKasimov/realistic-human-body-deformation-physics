$ErrorActionPreference = "Stop"

function Get-RepoRoot {
    return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

function Get-CargoPath {
    $fromPath = Get-Command cargo -ErrorAction SilentlyContinue
    if ($fromPath) {
        return $fromPath.Source
    }

    $candidates = @(
        (Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"),
        (Join-Path $env:USERPROFILE ".rustup\toolchains\stable-x86_64-pc-windows-msvc\bin\cargo.exe")
    )

    foreach ($candidate in $candidates) {
        if (Test-Path $candidate) {
            return $candidate
        }
    }

    throw "Could not find cargo.exe. Install Rust with: winget install Rustlang.Rustup; then restart PowerShell."
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

function Invoke-Cargo {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments,
        [Parameter(Mandatory = $true)]
        [string]$Label
    )

    $repoRoot = Get-RepoRoot
    $cargo = Get-CargoPath
    Invoke-Checked `
        -Label $Label `
        -Command {
            Push-Location $repoRoot
            try {
                & $cargo @Arguments
            } finally {
                Pop-Location
            }
        }
}

function Copy-RustAppToRepoRoot {
    $repoRoot = Get-RepoRoot
    $builtExe = Join-Path $repoRoot "target\release\realistic_physics.exe"
    if (-not (Test-Path $builtExe)) {
        throw "Expected Rust app executable was not produced: $builtExe"
    }
    Copy-Item -LiteralPath $builtExe -Destination (Join-Path $repoRoot "realistic_physics.exe") -Force
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
