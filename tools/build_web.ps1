param(
    [switch]$Serve,
    [int]$Port = 8080
)

. "$PSScriptRoot/common.ps1"

$repoRoot = Get-RepoRoot
$wasmTarget = "wasm32-unknown-unknown"
$siteDir = Join-Path $repoRoot "target/web"

# The page loads two scripts that ship inside crates, copied from the crates Cargo
# builds so they always match the wasm: miniquad's loader, gl.js, which has to match
# the miniquad version in Cargo.lock (macroquad pins it exactly), and the sound
# plugin, audio.js, from quad-snd, which Cargo.toml points at the patched copy in
# vendor/quad-snd. (macroquad's js/mq_js_bundle.js bundles both with a networking
# plugin this app does not use and that throws on load.)
function Get-CrateScripts {
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
    $scripts = @()
    foreach ($script in @(@{ Package = "miniquad"; File = "js/gl.js" }, @{ Package = "quad-snd"; File = "js/audio.js" })) {
        $package = $metadata.packages | Where-Object { $_.name -eq $script.Package } | Select-Object -First 1
        if (-not $package) {
            throw "cargo metadata did not list the $($script.Package) package."
        }
        $path = Join-Path (Split-Path -Parent $package.manifest_path) $script.File
        if (-not (Test-Path -LiteralPath $path)) {
            throw "$($script.Package) $($package.version) does not ship $($script.File) (looked in $path)."
        }
        $scripts += $path
    }
    return $scripts
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
Copy-Item -LiteralPath $wasm -Destination $siteDir
Copy-Item -LiteralPath (Get-CrateScripts) -Destination $siteDir

# GitHub Pages lets browsers cache every file for ten minutes, so a returning
# visitor could get a new page with the previous simulation. Stamping the wasm
# and script URLs with their content hashes makes each page load its own build.
function Get-ContentStamp([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.Substring(0, 12).ToLowerInvariant()
}

$wasmStamp = Get-ContentStamp (Join-Path $siteDir "realistic_physics.wasm")
$loaderStamp = Get-ContentStamp (Join-Path $siteDir "gl.js")
$audioStamp = Get-ContentStamp (Join-Path $siteDir "audio.js")
$page = [IO.File]::ReadAllText((Join-Path $repoRoot "web/index.html"))
$references = @(
    @{ From = 'src="gl.js"'; To = "src=`"gl.js?v=$loaderStamp`"" },
    @{ From = 'src="audio.js"'; To = "src=`"audio.js?v=$audioStamp`"" },
    @{ From = 'load("realistic_physics.wasm")'; To = "load(`"realistic_physics.wasm?v=$wasmStamp`")" }
)
foreach ($reference in $references) {
    $count = ([regex]::Matches($page, [regex]::Escape($reference.From))).Count
    if ($count -ne 1) {
        throw "web/index.html should contain $($reference.From) exactly once, found $count."
    }
    $page = $page.Replace($reference.From, $reference.To)
}
[IO.File]::WriteAllText((Join-Path $siteDir "index.html"), $page, [Text.UTF8Encoding]::new($false))

$wasmKb = (Get-Item -LiteralPath (Join-Path $siteDir "realistic_physics.wasm")).Length / 1KB
Write-Host ""
Write-Host ("Built web version in {0} (wasm {1:N0} KB)" -f $siteDir, $wasmKb)

if ($Serve) {
    & (Join-Path $PSScriptRoot "serve_web.ps1") -Port $Port -Root $siteDir
} else {
    Write-Host "Try it locally with: .\tools\serve_web.ps1"
}
