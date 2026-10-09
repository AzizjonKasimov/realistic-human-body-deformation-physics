<#
.SYNOPSIS
Checks that the browser build simulates exactly like the native one: plays the
self-test natively and in a headless browser and compares every scenario's
numbers.

.DESCRIPTION
Builds the native app and the web version (unless -SkipBuild), runs
`realistic_physics.exe --selftest`, serves target\web and opens it with
?selftest in headless Edge (or Chrome), reads window.__selftest over the
DevTools protocol, and lists every scenario whose injuries, blood, tool turn,
or tissue speed differ. Exits 1 on any mismatch. The headless browser keeps
animation frames running, which the app's browser pane stops while it is
hidden. Reports go to output\parity (native.json, web.json, report.txt).

.PARAMETER SkipBuild
Use the existing target\release app and target\web build.

.PARAMETER Port
Port to serve the web build on while the check runs.

.PARAMETER Browser
Path to a Chromium browser; Edge or Chrome is found by default.

.PARAMETER TimeoutSeconds
How long to wait for the browser self-test.

.EXAMPLE
.\tools\parity.ps1
Builds both, plays both self-tests, and compares them.
#>
param(
    [switch]$SkipBuild,
    [int]$Port = 8090,
    [string]$Browser = "",
    [int]$TimeoutSeconds = 300
)

. "$PSScriptRoot\common.ps1"

$repoRoot = Get-RepoRoot
$out = Join-Path $repoRoot "output\parity"
New-Item -ItemType Directory -Force $out | Out-Null
$nativeReport = Join-Path $out "native.json"
$webReport = Join-Path $out "web.json"

function Find-Browser {
    if ($Browser) {
        if (-not (Test-Path -LiteralPath $Browser)) {
            throw "No browser at $Browser"
        }
        return $Browser
    }
    $candidates = @(
        "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe",
        "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe",
        "$env:ProgramFiles\Google\Chrome\Application\chrome.exe",
        "$env:LOCALAPPDATA\Google\Chrome\Application\chrome.exe"
    )
    foreach ($candidate in $candidates) {
        if (Test-Path -LiteralPath $candidate) {
            return $candidate
        }
    }
    throw "Found neither Edge nor Chrome; pass -Browser with a Chromium browser's path."
}

function Receive-CdpMessage([System.Net.WebSockets.ClientWebSocket]$Socket) {
    $buffer = [byte[]]::new(1MB)
    $message = [IO.MemoryStream]::new()
    do {
        $segment = [ArraySegment[byte]]::new($buffer)
        $result = $Socket.ReceiveAsync($segment, [Threading.CancellationToken]::None).GetAwaiter().GetResult()
        if ($result.MessageType -eq [System.Net.WebSockets.WebSocketMessageType]::Close) {
            throw "The browser closed the DevTools connection."
        }
        $message.Write($buffer, 0, $result.Count)
    } while (-not $result.EndOfMessage)
    return [Text.Encoding]::UTF8.GetString($message.ToArray())
}

# Sends one DevTools command and returns its result, skipping the events the
# browser sends in between.
function Invoke-Cdp([System.Net.WebSockets.ClientWebSocket]$Socket, [int]$Id, [string]$Method, [hashtable]$Params) {
    $request = @{ id = $Id; method = $Method; params = $Params } | ConvertTo-Json -Depth 8 -Compress
    $bytes = [Text.Encoding]::UTF8.GetBytes($request)
    $segment = [ArraySegment[byte]]::new($bytes)
    $Socket.SendAsync($segment, [System.Net.WebSockets.WebSocketMessageType]::Text, $true, [Threading.CancellationToken]::None).GetAwaiter().GetResult() | Out-Null
    while ($true) {
        $reply = Receive-CdpMessage $Socket | ConvertFrom-Json -Depth 64
        if ($reply.id -eq $Id) {
            if ($reply.error) {
                throw "$Method failed: $($reply.error.message)"
            }
            return $reply.result
        }
    }
}

# Plays ?selftest in a headless browser and returns window.__selftest as JSON.
function Invoke-WebSelfTest {
    $siteDir = Join-Path $repoRoot "target\web"
    if (-not (Test-Path -LiteralPath (Join-Path $siteDir "index.html"))) {
        throw "No web build in $siteDir; run without -SkipBuild."
    }
    $browserPath = Find-Browser
    $profileDir = Join-Path ([IO.Path]::GetTempPath()) "rp-parity-$PID"
    $server = $null
    $browserProcess = $null
    $socket = $null
    try {
        $server = Start-Process pwsh -PassThru -WindowStyle Hidden -ArgumentList @(
            "-NoProfile", "-File", (Join-Path $PSScriptRoot "serve_web.ps1"), "-Port", $Port, "-Root", $siteDir
        )
        $url = "http://localhost:$Port/"
        $deadline = (Get-Date).AddSeconds(20)
        while ($true) {
            try {
                Invoke-WebRequest -Uri $url -Method Head -TimeoutSec 2 | Out-Null
                break
            } catch {
                if ($server.HasExited -or (Get-Date) -gt $deadline) {
                    throw "The web server did not start on port $Port (is the port taken? try -Port)."
                }
                Start-Sleep -Milliseconds 200
            }
        }

        # Port 0 lets the browser pick a free DevTools port, which it writes
        # to DevToolsActivePort in its profile.
        if (Test-Path -LiteralPath $profileDir) {
            Remove-Item -LiteralPath $profileDir -Recurse -Force
        }
        $browserProcess = Start-Process $browserPath -PassThru -ArgumentList @(
            "--headless=new", "--remote-debugging-port=0", "--user-data-dir=`"$profileDir`"",
            "--no-first-run", "--no-default-browser-check", "--disable-extensions",
            "--enable-unsafe-swiftshader", "--window-size=1280,720", "about:blank"
        )
        $portFile = Join-Path $profileDir "DevToolsActivePort"
        $deadline = (Get-Date).AddSeconds(30)
        while (-not (Test-Path -LiteralPath $portFile) -or -not (Get-Content -LiteralPath $portFile -ErrorAction SilentlyContinue)) {
            if ((Get-Date) -gt $deadline) {
                throw "The headless browser did not open its DevTools port."
            }
            Start-Sleep -Milliseconds 200
        }
        $devtoolsPort = (Get-Content -LiteralPath $portFile | Select-Object -First 1).Trim()
        $targets = Invoke-RestMethod "http://127.0.0.1:$devtoolsPort/json/list"
        $page = @($targets) | Where-Object { $_.type -eq "page" } | Select-Object -First 1
        if (-not $page) {
            throw "The headless browser has no page to drive."
        }

        $socket = [System.Net.WebSockets.ClientWebSocket]::new()
        $socket.Options.KeepAliveInterval = [TimeSpan]::Zero
        $socket.ConnectAsync([Uri]$page.webSocketDebuggerUrl, [Threading.CancellationToken]::None).GetAwaiter().GetResult() | Out-Null
        $id = 1
        Invoke-Cdp $socket ($id++) "Page.navigate" @{ url = "$($url)?selftest" } | Out-Null
        $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
        while ($true) {
            Start-Sleep -Seconds 1
            $result = Invoke-Cdp $socket ($id++) "Runtime.evaluate" @{
                expression    = "window.__selftest ? JSON.stringify(window.__selftest) : ''"
                returnByValue = $true
            }
            if ($result.result.value) {
                return $result.result.value
            }
            if ((Get-Date) -gt $deadline) {
                $status = Invoke-Cdp $socket ($id++) "Runtime.evaluate" @{
                    expression    = "document.getElementById('status').textContent"
                    returnByValue = $true
                }
                throw "The browser self-test did not finish in $TimeoutSeconds s (status: $($status.result.value))."
            }
        }
    } finally {
        if ($socket) {
            $socket.Dispose()
        }
        foreach ($process in @($browserProcess, $server)) {
            if ($process -and -not $process.HasExited) {
                Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
            }
        }
        # The browser's helper processes hold the profile for a moment.
        for ($attempt = 0; $attempt -lt 20 -and (Test-Path -LiteralPath $profileDir); $attempt++) {
            Start-Sleep -Milliseconds 250
            Remove-Item -LiteralPath $profileDir -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}

if (-not $SkipBuild) {
    Invoke-Cargo -Arguments @("build", "--release", "--bin", "realistic_physics") -Label "Build native app (Release)"
    & (Join-Path $PSScriptRoot "build_web.ps1")
}

$app = Join-Path $repoRoot "target\release\realistic_physics.exe"
if (-not (Test-Path -LiteralPath $app)) {
    throw "No native build at $app; run without -SkipBuild."
}
Write-Host ""
Write-Host "==> Native self-test"
$nativeJson = (& $app --selftest | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or -not $nativeJson.StartsWith("{")) {
    throw "The native self-test failed: $nativeJson"
}
[IO.File]::WriteAllText($nativeReport, $nativeJson)

Write-Host ""
Write-Host "==> Browser self-test (headless, http://localhost:$Port/?selftest)"
$webJson = Invoke-WebSelfTest
[IO.File]::WriteAllText($webReport, $webJson)

$native = $nativeJson | ConvertFrom-Json -Depth 16
$web = $webJson | ConvertFrom-Json -Depth 16
$fields = @("in_band", "bones", "ribs", "skin", "muscle", "bruises", "fluid", "blood_loss", "max_tool_turn", "fastest_tissue")
$lines = [System.Collections.Generic.List[string]]::new()
$mismatched = 0
$names = @($native.scenarios.name) + @($web.scenarios.name) | Select-Object -Unique
foreach ($name in $names) {
    $a = $native.scenarios | Where-Object { $_.name -eq $name }
    $b = $web.scenarios | Where-Object { $_.name -eq $name }
    if (-not $a -or -not $b) {
        $mismatched++
        $lines.Add(("{0,-28} only {1}" -f $name, $(if ($a) { "native" } else { "browser" })))
        continue
    }
    $differences = foreach ($field in $fields) {
        if ("$($a.$field)" -ne "$($b.$field)") {
            "$field $($a.$field) native, $($b.$field) browser"
        }
    }
    if ($differences) {
        $mismatched++
        $lines.Add(("{0,-28} DIFFERS: {1}" -f $name, ($differences -join "; ")))
    } else {
        $lines.Add(("{0,-28} same" -f $name))
    }
}
$lines.Add("")
$lines.Add(("{0} of {1} scenarios match exactly." -f ($names.Count - $mismatched), $names.Count))
$lines.Add(("Step time: native {0:N2} ms on average ({1:N2} max), browser {2:N2} ms ({3:N2} max)." -f `
    $native.avg_step_ms, $native.max_step_ms, $web.avg_step_ms, $web.max_step_ms))
$lines.Add(("Self-test in band: native {0}, browser {1}." -f $native.passed, $web.passed))
[IO.File]::WriteAllLines((Join-Path $out "report.txt"), $lines)
Write-Host ""
$lines | ForEach-Object { Write-Host $_ }
Write-Host "Reports in $out"
if ($mismatched -gt 0) {
    exit 1
}
