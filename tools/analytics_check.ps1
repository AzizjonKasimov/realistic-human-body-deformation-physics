<#
.SYNOPSIS
Checks the page's visit counts in a headless browser: every event the page
counts, and the request it sends GoatCounter for each.

.DESCRIPTION
By default it plays the local web build (target\web, rebuilt unless
-SkipBuild) at the live site's address, so analytics.js sends as it does
there and loads GoatCounter's real script, while the check answers every
request to GoatCounter itself, so nothing is counted (a host rule also stops
any request that would slip past). It plays six page loads:

- Playing: Start, a blow with each tool, a link followed, the about card
  opened (play time must stop, and its button must not count a second
  Start), then play past a minute for played-1min and the frame-rate event.
- No WebGL: WebGL is hidden from the page, which must count no-webgl.
- Download fails: the simulation's download fails, which must count
  load-failed.
- Crash: the simulation throws in its frame after loading, which must count
  crashed.
- Opened as an app: the page sees itself opened as the installed app, which
  must count opened-as-app.
- Installed: the browser tells the page it was installed, which must count
  installed.

It lists each page's events with the request sent for each, writes
output\analytics\report.txt, and exits 1 when an event is missing, counted
when it should not be, or sent with no request, two, or a wrong one.

-Live plays the live site instead (Start, a blow, a link) and lets its
requests reach GoatCounter, to check that GoatCounter accepts them (status
200 with no X-Goatcounter reason) and that the live analytics.js is the one
in web\. GoatCounter files a headless browser's visits under bots, outside
the dashboard's counts, so the check adds nothing to them; it stops before
opening the site if the browser would not be taken for one.

.PARAMETER Live
Check the live site and GoatCounter's answers instead of the local build.

.PARAMETER SkipBuild
Use the existing target\web build.

.PARAMETER Browser
Path to a Chromium browser; Edge or Chrome is found by default.

.EXAMPLE
.\tools\analytics_check.ps1
Builds the web version and checks every event without sending anything.

.EXAMPLE
.\tools\analytics_check.ps1 -Live
Checks that GoatCounter accepts the live site's requests, as a bot's.
#>
param(
    [switch]$Live,
    [switch]$SkipBuild,
    [string]$Browser = ""
)

. "$PSScriptRoot\common.ps1"
. "$PSScriptRoot\headless.ps1"

$repoRoot = Get-RepoRoot
$out = Join-Path $repoRoot "output\analytics"
New-Item -ItemType Directory -Force $out | Out-Null

# The site code and live host come from analytics.js, so the check follows it.
$analyticsPath = Join-Path $repoRoot "web\analytics.js"
$analyticsSource = [IO.File]::ReadAllText($analyticsPath)
$siteCode = [regex]::Match($analyticsSource, 'const SITE_CODE = "([^"]+)"').Groups[1].Value
$liveHost = [regex]::Match($analyticsSource, 'const LIVE_HOST = "([^"]+)"').Groups[1].Value
if (-not $siteCode -or -not $liveHost) {
    throw "web/analytics.js should set SITE_CODE and LIVE_HOST."
}
# GitHub Pages serves the repo's site under the repo's name.
$sitePath = "/realistic-human-body-deformation-physics/"
$liveUrl = "https://$liveHost$sitePath"
$countHost = "$siteCode.goatcounter.com"
$siteDir = Join-Path $repoRoot "target\web"
$contentTypes = Get-WebContentTypes
# GoatCounter answers a count with a 1x1 GIF, and so does the local check.
$gif = "R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7"

# Requests to GoatCounter's /count, by DevTools request id, for one page load.
$requests = [ordered]@{}
$fileCache = @{}
$failDownload = $false
$lines = [System.Collections.Generic.List[string]]::new()
$problems = [System.Collections.Generic.List[string]]::new()

function Add-Line([string]$Text) {
    $lines.Add($Text)
    Write-Host $Text
}

function ConvertFrom-QueryString([string]$Query) {
    $values = @{}
    foreach ($pair in $Query.TrimStart("?").Split("&", [StringSplitOptions]::RemoveEmptyEntries)) {
        $parts = $pair.Split("=", 2)
        $values[[Uri]::UnescapeDataString($parts[0])] = if ($parts.Count -gt 1) { [Uri]::UnescapeDataString($parts[1]) } else { "" }
    }
    return $values
}

function Get-Header($Headers, [string]$Name) {
    foreach ($property in $Headers.PSObject.Properties) {
        if ($property.Name -eq $Name) {
            return "$($property.Value)"
        }
    }
    return ""
}

# Answers a request the page makes in the local check: the live site's files
# from the local build, and GoatCounter's /count with its usual GIF.
function Send-LocalResponse($Params) {
    $url = [Uri]$Params.request.url
    if ($url.Host -eq $countHost) {
        Send-Cdp $page "Fetch.fulfillRequest" @{
            requestId       = $Params.requestId
            responseCode    = 200
            responseHeaders = @(@{ name = "Content-Type"; value = "image/gif" })
            body            = $gif
        } | Out-Null
        return
    }
    $relative = ""
    if ($url.AbsolutePath.StartsWith($sitePath)) {
        $relative = $url.AbsolutePath.Substring($sitePath.Length)
        if (-not $relative) {
            $relative = "index.html"
        }
    }
    if ($failDownload -and $relative.EndsWith(".wasm")) {
        Send-Cdp $page "Fetch.failRequest" @{ requestId = $Params.requestId; errorReason = "ConnectionFailed" } | Out-Null
        return
    }
    $type = $contentTypes[[IO.Path]::GetExtension($relative).ToLowerInvariant()]
    $file = if ($relative -and $type -and -not $relative.Contains("..")) { Join-Path $siteDir $relative } else { "" }
    if ($file -and (Test-Path -LiteralPath $file -PathType Leaf)) {
        if (-not $fileCache.ContainsKey($file)) {
            $fileCache[$file] = [Convert]::ToBase64String([IO.File]::ReadAllBytes($file))
        }
        Send-Cdp $page "Fetch.fulfillRequest" @{
            requestId       = $Params.requestId
            responseCode    = 200
            responseHeaders = @(@{ name = "Content-Type"; value = $type }, @{ name = "Cache-Control"; value = "no-store" })
            body            = $fileCache[$file]
        } | Out-Null
    } else {
        Send-Cdp $page "Fetch.fulfillRequest" @{ requestId = $Params.requestId; responseCode = 404; responseHeaders = @(); body = "" } | Out-Null
    }
}

# Notes every request to GoatCounter's /count and its answer, and in the
# local check answers the page's requests.
$onEvent = {
    param($message)
    $params = $message.params
    switch ($message.method) {
        "Network.requestWillBeSent" {
            $url = [Uri]$params.request.url
            if ($url.Host -eq $countHost -and $url.AbsolutePath -eq "/count") {
                $requests[$params.requestId] = [pscustomobject]@{
                    Query  = ConvertFrom-QueryString $url.Query
                    Status = ""
                    Reason = ""
                }
            }
        }
        "Network.responseReceived" {
            $request = $requests[$params.requestId]
            if ($request) {
                $request.Status = "$($params.response.status)"
                $request.Reason = Get-Header $params.response.headers "X-Goatcounter"
            }
        }
        "Network.loadingFailed" {
            $request = $requests[$params.requestId]
            if ($request) {
                $request.Status = "failed"
                $request.Reason = $params.errorText
            }
        }
        "Fetch.requestPaused" {
            Send-LocalResponse $params
        }
    }
}

# Opens the page afresh and waits until it has loaded GoatCounter's script,
# which it fetches from GoatCounter in both checks.
function Open-Page {
    $requests.Clear()
    Invoke-Cdp $page "Page.navigate" @{ url = $liveUrl } | Out-Null
    $deadline = (Get-Date).AddSeconds(30)
    while ($true) {
        $ready = $false
        try {
            $ready = Invoke-PageScript $page "return document.readyState === 'complete' && Array.isArray(window.__analytics) && typeof (window.goatcounter || {}).count === 'function';"
        } catch {
            # The page was still being replaced.
        }
        if ($ready) {
            return
        }
        if ((Get-Date) -gt $deadline) {
            throw "The page did not load GoatCounter's script within 30 s: offline, blocked, or its pinned hash in analytics.js no longer matches gc.zgo.at's file."
        }
        Start-Sleep -Milliseconds 250
    }
}

function Get-Events {
    return @(Invoke-PageScript $page "return window.__analytics || [];")
}

function Test-Counted([string]$Pattern) {
    return [bool](Get-Events | Where-Object { $_.name -like $Pattern })
}

function Wait-Counted([string]$Pattern, [int]$Seconds) {
    $deadline = (Get-Date).AddSeconds($Seconds)
    while ((Get-Date) -lt $deadline) {
        if (Test-Counted $Pattern) {
            return $true
        }
        Start-Sleep -Milliseconds 250
    }
    return $false
}

# Waits while still answering the page's requests.
function Wait-Page([double]$Seconds) {
    $deadline = (Get-Date).AddSeconds($Seconds)
    while ((Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 250
        Invoke-PageScript $page "return true;" | Out-Null
    }
}

function Send-Mouse([string]$Type, [double]$X, [double]$Y, [bool]$Held) {
    $params = @{ type = $Type; x = $X; y = $Y; button = "left"; buttons = $(if ($Held) { 1 } else { 0 }) }
    if ($Type -eq "mouseMoved" -and -not $Held) {
        $params.button = "none"
    } elseif ($Type -ne "mouseMoved") {
        $params.clickCount = 1
    }
    Invoke-Cdp $page "Input.dispatchMouseEvent" $params | Out-Null
}

function Invoke-Click([string]$Selector) {
    $box = Invoke-PageScript $page "const r = document.querySelector('$Selector').getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 };"
    Send-Mouse "mouseMoved" $box.x $box.y $false
    Send-Mouse "mousePressed" $box.x $box.y $true
    Send-Mouse "mouseReleased" $box.x $box.y $false
}

# Presses a letter key; gl.js reads the key's code.
function Send-Key([string]$Letter) {
    $upper = $Letter.ToUpperInvariant()
    $key = @{ key = $Letter; code = "Key$upper"; windowsVirtualKeyCode = [int][char]$upper }
    Invoke-Cdp $page "Input.dispatchKeyEvent" ($key + @{ type = "keyDown"; text = $Letter }) | Out-Null
    Invoke-Cdp $page "Input.dispatchKeyEvent" ($key + @{ type = "keyUp" }) | Out-Null
}

# One swing across the body's middle, with the tool parked beside the body
# first so it starts outside it, held at the end since the tool trails the
# pointer, and parked beside the body again afterwards.
function Invoke-Swing {
    $view = Invoke-PageScript $page "return { w: innerWidth, h: innerHeight };"
    $y = $view.h * 0.5
    $from = $view.w * 0.25
    $to = $view.w * 0.55
    for ($i = 0; $i -lt 10; $i++) {
        Send-Mouse "mouseMoved" $from $y $false
        Start-Sleep -Milliseconds 50
    }
    Send-Mouse "mousePressed" $from $y $true
    for ($i = 1; $i -le 10; $i++) {
        Send-Mouse "mouseMoved" ($from + ($to - $from) * $i / 10) $y $true
        Start-Sleep -Milliseconds 16
    }
    for ($i = 0; $i -lt 20; $i++) {
        Send-Mouse "mouseMoved" $to $y $true
        Start-Sleep -Milliseconds 20
    }
    Send-Mouse "mouseReleased" $to $y $false
    for ($i = 0; $i -lt 10; $i++) {
        Send-Mouse "mouseMoved" $from $y $false
        Start-Sleep -Milliseconds 50
    }
}

# Strikes with the tool in hand until the page counts its first blow (two
# tries, since a fast swing can pass between the body's points).
function Invoke-Blow([string]$Tool) {
    for ($try = 0; $try -lt 2; $try++) {
        Invoke-Swing
        if (Wait-Counted "tool-$Tool" 3) {
            return
        }
    }
}

# Clicks a link as a visitor would, with its navigation stopped so the check
# stays on the page.
function Invoke-Link([string]$Text) {
    Invoke-PageScript $page @"
const link = [...document.querySelectorAll('a[href]')].find(a => a.textContent.trim() === '$Text');
link.addEventListener('click', event => event.preventDefault(), { once: true });
link.click();
return true;
"@ | Out-Null
}

# Compares what the page counted with what it should have, and each count
# with the one request the page should have sent for it.
function Test-Run([string]$Name, [string[]]$Expected, [string[]]$Optional = @()) {
    # Let the last requests go out and their answers come back.
    Wait-Page 2
    $events = Get-Events
    $sent = @($requests.Values)
    $used = @{}
    Add-Line ""
    Add-Line $Name
    foreach ($entry in $events) {
        $isVisit = $entry.name -eq "visit"
        $path = if ($isVisit) { $sitePath } else { $entry.name }
        $matching = @(for ($i = 0; $i -lt $sent.Count; $i++) {
                $query = $sent[$i].Query
                if ($query["p"] -eq $path -and $query["t"] -eq $entry.title -and (($query["e"] -eq "true") -ne $isVisit)) {
                    $i
                }
            })
        foreach ($i in $matching) {
            $used[$i] = $true
        }
        $note = if ($matching.Count -eq 0) {
            $problems.Add("${Name}: no request was sent for $($entry.name)")
            "NO REQUEST"
        } elseif ($matching.Count -gt 1) {
            $problems.Add("${Name}: $($matching.Count) requests were sent for $($entry.name)")
            "$($matching.Count) REQUESTS"
        } else {
            $request = $sent[$matching[0]]
            if ($request.Status -ne "200" -or $request.Reason) {
                $problems.Add("${Name}: the request for $($entry.name) was answered '$($request.Status)' $($request.Reason)")
            }
            $answer = if ($Live) { "GoatCounter answered $($request.Status)" } else { "answered by the check" }
            if ($request.Reason) {
                $answer += " ($($request.Reason))"
            }
            if ($Live) {
                $answer += ", b=$($request.Query['b'])"
            }
            "sent, $answer"
        }
        Add-Line ("  {0,-22} {1,6:N1} s  {2}" -f $entry.name, ($entry.at / 1000), $note)
    }
    foreach ($pattern in $Expected) {
        $count = @($events | Where-Object { $_.name -like $pattern }).Count
        if ($count -eq 0) {
            $problems.Add("${Name}: $pattern was not counted")
        } elseif ($count -gt 1) {
            $problems.Add("${Name}: $pattern was counted $count times")
        }
    }
    foreach ($pattern in $Optional) {
        $count = @($events | Where-Object { $_.name -like $pattern }).Count
        if ($count -gt 1) {
            $problems.Add("${Name}: $pattern was counted $count times")
        }
    }
    foreach ($entry in $events) {
        if (-not (@($Expected) + @($Optional) | Where-Object { $entry.name -like $_ })) {
            $problems.Add("${Name}: $($entry.name) was counted but should not have been")
        }
    }
    for ($i = 0; $i -lt $sent.Count; $i++) {
        if (-not $used[$i]) {
            $problems.Add("${Name}: a request for '$($sent[$i].Query['p'])' matched no counted event")
        }
    }
}

if (-not $Live) {
    if (-not $SkipBuild) {
        & (Join-Path $PSScriptRoot "build_web.ps1")
    }
    if (-not (Test-Path -LiteralPath (Join-Path $siteDir "index.html"))) {
        throw "No web build in $siteDir; run without -SkipBuild."
    }
}

# The local check also turns away every GoatCounter address at the browser's
# name lookup, so a request the check failed to answer cannot be counted.
$arguments = @()
if (-not $Live) {
    $arguments = @("--host-resolver-rules=`"MAP *.goatcounter.com ~NOTFOUND`"")
}
$page = $null
try {
    $page = Start-HeadlessBrowser $Browser "analytics" $arguments
    $page.OnEvent = $onEvent
    Invoke-Cdp $page "Page.enable" | Out-Null
    Invoke-Cdp $page "Network.enable" | Out-Null
    Invoke-Cdp $page "Network.setCacheDisabled" @{ cacheDisabled = $true } | Out-Null

    if ($Live) {
        $agent = Invoke-PageScript $page "return { webdriver: navigator.webdriver, userAgent: navigator.userAgent };"
        if (-not $agent.webdriver) {
            throw "This browser does not tell pages it is automated, so GoatCounter would count the check as a real visit. Stopped before opening the live site."
        }
        Add-Line "Live site $liveUrl"
        Add-Line "in a headless browser that GoatCounter files under bots ($($agent.userAgent))."
        $liveScript = (Invoke-WebRequest -Uri "$($liveUrl)analytics.js" -UseBasicParsing).Content
        if ($liveScript -is [byte[]]) {
            $liveScript = [Text.Encoding]::UTF8.GetString($liveScript)
        }
        if (($liveScript -replace "`r`n", "`n") -eq ($analyticsSource -replace "`r`n", "`n")) {
            Add-Line "The live analytics.js is the same as web\analytics.js."
        } else {
            $problems.Add("The live analytics.js differs from web\analytics.js (not deployed yet, or changed since).")
        }

        Write-Host "Opening the live site, pressing Start, striking once, following a link..."
        Open-Page
        Invoke-Click "#start"
        if (Wait-Counted "loaded" 60) {
            Invoke-Blow "sledgehammer"
        }
        Invoke-Link "Source code"
        Test-Run "Live site: Start, a blow, a link" @("visit", "start", "loaded", "hit", "tool-sledgehammer", "link-source-code") @("fps-*")
    } else {
        $reached = Invoke-PageScript $page "try { await fetch('https://$countHost/', { mode: 'no-cors' }); return true; } catch (error) { return false; }"
        if ($reached) {
            throw "The browser still reached $countHost despite the host rule, so the check could send counts. Stopped before opening the page."
        }
        Invoke-Cdp $page "Fetch.enable" @{
            patterns = @(
                @{ urlPattern = "https://$liveHost/*"; requestStage = "Request" },
                @{ urlPattern = "https://$countHost/*"; requestStage = "Request" }
            )
        } | Out-Null
        Add-Line "Local build $siteDir at the live address $liveUrl;"
        Add-Line "the check answers every request to GoatCounter itself, so nothing is counted."

        Write-Host ""
        Write-Host "Playing: Start, a blow with each tool, a link, the about card, then a minute of play (about 90 s)..."
        Open-Page
        Invoke-Click "#start"
        if (Wait-Counted "loaded" 60) {
            Invoke-Blow "sledgehammer"
            foreach ($tool in @(@{ Key = "b"; Name = "bat" }, @{ Key = "s"; Name = "knife" })) {
                Send-Key $tool.Key
                Wait-Page 0.5
                Invoke-Blow $tool.Name
            }
            Invoke-Link "Source code"

            # The about card pauses play time, and its button back to the
            # simulation is the start button, which must not count again.
            Send-Key "i"
            Wait-Page 1
            if (Invoke-PageScript $page "return document.getElementById('gate').hidden;") {
                $problems.Add("Playing: the I key did not open the about card")
            } else {
                $before = Invoke-PageScript $page "return window.rpAnalytics.playedSeconds();"
                Wait-Page 3
                $after = Invoke-PageScript $page "return window.rpAnalytics.playedSeconds();"
                Add-Line ("About card open for 3 s: play time {0:N1} s before, {1:N1} s after." -f $before, $after)
                if ($after -gt $before) {
                    $problems.Add("Playing: play time ran on while the about card was open")
                }
                Invoke-Click "#start"
                Wait-Page 0.5
            }

            # A visitor who moves the pointer now and then is playing, not away.
            $view = Invoke-PageScript $page "return { w: innerWidth, h: innerHeight };"
            $deadline = (Get-Date).AddSeconds(120)
            for ($step = 0; -not (Test-Counted "played-1min") -and (Get-Date) -lt $deadline; $step++) {
                Send-Mouse "mouseMoved" ($view.w * (0.2 + 0.02 * ($step % 3))) ($view.h * 0.5) $false
                Wait-Page 2
            }
        }
        Test-Run "Playing: Start, each tool, a link, the about card, a minute" @(
            "visit", "start", "loaded", "hit", "tool-sledgehammer", "tool-bat", "tool-knife",
            "link-source-code", "fps-*-mouse", "played-1min"
        )

        Write-Host ""
        Write-Host "No WebGL: pressing Start with WebGL hidden from the page..."
        $hide = Invoke-Cdp $page "Page.addScriptToEvaluateOnNewDocument" @{
            source = "const getContext = HTMLCanvasElement.prototype.getContext; HTMLCanvasElement.prototype.getContext = function (type, ...rest) { return /webgl/i.test(type) ? null : getContext.call(this, type, ...rest); };"
        }
        Open-Page
        Invoke-Click "#start"
        Wait-Counted "no-webgl" 5 | Out-Null
        Invoke-Cdp $page "Page.removeScriptToEvaluateOnNewDocument" @{ identifier = $hide.identifier } | Out-Null
        Test-Run "No WebGL" @("visit", "start", "no-webgl")

        Write-Host ""
        Write-Host "Download fails: pressing Start with the simulation's download failing..."
        $failDownload = $true
        Open-Page
        Invoke-Click "#start"
        Wait-Counted "load-failed" 10 | Out-Null
        $failDownload = $false
        Test-Run "Download fails" @("visit", "start", "load-failed")

        Write-Host ""
        Write-Host "Crash: making the simulation throw in its frame after it loads..."
        Open-Page
        Invoke-Click "#start"
        if (Wait-Counted "loaded" 60) {
            # gl.js calls the global wasm_exports.frame() every frame; the wasm
            # module's own exports cannot be changed, so a copy replaces them.
            Invoke-PageScript $page "window.wasm_exports = Object.assign({}, window.wasm_exports, { frame() { throw new Error('analytics_check: the simulation stops here on purpose'); } }); return true;" | Out-Null
            Wait-Counted "crashed" 6 | Out-Null
        }
        Test-Run "Crash" @("visit", "start", "loaded", "crashed")

        Write-Host ""
        Write-Host "Opened as an app: loading the page as the installed app opens it..."
        $asApp = Invoke-Cdp $page "Page.addScriptToEvaluateOnNewDocument" @{ source = Get-InstalledAppScript }
        Open-Page
        Wait-Counted "opened-as-app" 3 | Out-Null
        Invoke-Cdp $page "Page.removeScriptToEvaluateOnNewDocument" @{ identifier = $asApp.identifier } | Out-Null
        Test-Run "Opened as an app" @("visit", "opened-as-app")

        Write-Host ""
        Write-Host "Installed: the browser telling the page the visitor installed it..."
        Open-Page
        Invoke-PageScript $page "window.dispatchEvent(new Event('appinstalled')); return true;" | Out-Null
        Wait-Counted "installed" 3 | Out-Null
        Test-Run "Installed" @("visit", "installed")
    }
} finally {
    Stop-HeadlessBrowser $page
}

Add-Line ""
if ($problems.Count -eq 0) {
    Add-Line "Every event was counted when it should be, once, with one matching request."
} else {
    Add-Line "$($problems.Count) problem(s):"
    foreach ($problem in $problems) {
        Add-Line "  $problem"
    }
}
[IO.File]::WriteAllLines((Join-Path $out "report.txt"), $lines)
Write-Host "Report in $(Join-Path $out 'report.txt')"
if ($problems.Count -gt 0) {
    exit 1
}
