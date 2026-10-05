param(
    [switch]$Serve,
    [int]$Port = 8080
)

. "$PSScriptRoot/common.ps1"

$repoRoot = Get-RepoRoot
$wasmTarget = "wasm32-unknown-unknown"
$siteDir = Join-Path $repoRoot "target/web"

# The JS loader has to match the miniquad version in Cargo.lock (macroquad pins it
# exactly), so copy the gl.js shipped inside that crate instead of vendoring it.
# macroquad's js/mq_js_bundle.js adds audio and networking plugins this app does not
# use, and its networking plugin throws on load; switch to that bundle only if the
# macroquad "audio" feature is ever enabled.
function Get-MiniquadLoader {
    $cargo = Get-CargoPath
    Push-Location $repoRoot
    try {
        $json = & $cargo metadata --format-version 1 --filter-platform $wasmTarget
        if ($LASTEXITCODE -ne 0) {
            throw "cargo metadata failed with exit code $LASTEXITCODE"
        }
    } finally {
        Pop-Location
    }

    $metadata = ($json -join "`n") | ConvertFrom-Json
    $miniquad = $metadata.packages | Where-Object { $_.name -eq "miniquad" } | Select-Object -First 1
    if (-not $miniquad) {
        throw "cargo metadata did not list the miniquad package."
    }

    $loader = Join-Path (Split-Path -Parent $miniquad.manifest_path) "js/gl.js"
    if (-not (Test-Path -LiteralPath $loader)) {
        throw "miniquad $($miniquad.version) does not ship js/gl.js (looked in $loader)."
    }
    return $loader
}

Invoke-Cargo -Arguments @("build", "--release", "--target", $wasmTarget, "--bin", "realistic_physics") -Label "Build Rust app for the web (wasm32, Release)"

$wasm = Join-Path $repoRoot "target/$wasmTarget/release/realistic_physics.wasm"
if (-not (Test-Path -LiteralPath $wasm)) {
    throw "Expected web build was not produced: $wasm"
}

if (Test-Path -LiteralPath $siteDir) {
    Remove-Item -LiteralPath $siteDir -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $siteDir | Out-Null
Copy-Item -LiteralPath (Join-Path $repoRoot "web/index.html") -Destination $siteDir
Copy-Item -LiteralPath $wasm -Destination $siteDir
Copy-Item -LiteralPath (Get-MiniquadLoader) -Destination $siteDir

$wasmKb = (Get-Item -LiteralPath (Join-Path $siteDir "realistic_physics.wasm")).Length / 1KB
Write-Host ""
Write-Host ("Built web version in {0} (wasm {1:N0} KB)" -f $siteDir, $wasmKb)

if ($Serve) {
    & (Join-Path $PSScriptRoot "serve_web.ps1") -Port $Port -Root $siteDir
} else {
    Write-Host "Try it locally with: .\tools\serve_web.ps1"
}
