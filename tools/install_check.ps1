<#
.SYNOPSIS
Checks that the web version installs as an app: its manifest and icons, and
the start screen's steps for installing it in each kind of browser.

.DESCRIPTION
Serves the web build (target\web, rebuilt unless -SkipBuild) on localhost,
which browsers trust as they trust the live site's HTTPS, and opens it in
headless Edge (or Chrome). It checks that:

- the browser reads manifest.webmanifest without errors and finds nothing
  that stops the page from installing (DevTools' installability errors);
- every icon the manifest and the page name loads with the size it claims,
  and the icons that systems crop to their own shape (maskable, Apple's)
  have no transparent pixels;
- the start screen's install section gives the steps for each kind of
  browser (Chrome and Samsung Internet on Android, iPhone, iPad, an app's
  built-in browser on either, Chrome and Edge on Windows, a Mac, or a
  Chromebook, Safari on a Mac, Firefox on Windows), stays hidden where the
  browser cannot install pages (Firefox on a Mac or Linux, older Safari and
  Firefox) and when the game already runs as an installed app, and shows an
  Install button when the browser offers to install, which asks the browser
  once and then says the game is installed;
- the install box sits right under the Start button and shows whole before
  any scrolling on a 360x640 and a 390x844 phone and a 1280x720 laptop.

It writes output\install\report.txt and exits 1 on any problem.

.PARAMETER Live
Check the live site instead, after a deploy. GoatCounter's addresses are
blocked, so the check counts no visits.

.PARAMETER SkipBuild
Use the existing target\web build.

.PARAMETER Port
Port to serve the web build on while the check runs.

.PARAMETER Browser
Path to a Chromium browser; Edge or Chrome is found by default.

.EXAMPLE
.\tools\install_check.ps1
Builds the web version and checks that it installs as an app.

.EXAMPLE
.\tools\install_check.ps1 -Live
Checks the live site the same way.
#>
param(
    [switch]$Live,
    [switch]$SkipBuild,
    [int]$Port = 8091,
    [string]$Browser = ""
)

. "$PSScriptRoot\common.ps1"
. "$PSScriptRoot\headless.ps1"

$repoRoot = Get-RepoRoot
$out = Join-Path $repoRoot "output\install"
New-Item -ItemType Directory -Force $out | Out-Null
$siteDir = Join-Path $repoRoot "target\web"
$liveUrl = "https://azizjonkasimov.github.io/realistic-human-body-deformation-physics/"
$lines = [System.Collections.Generic.List[string]]::new()
$problems = [System.Collections.Generic.List[string]]::new()

function Add-Line([string]$Text) {
    $lines.Add($Text)
    Write-Host $Text
}

# The browsers the start screen tells apart, with what its install section
# must say in each. A case without Expect must leave the section hidden.
$chrome = "Chrome/141.0.0.0"
$cases = @(
    @{
        Name   = "Android, Chrome"
        Agent  = "Mozilla/5.0 (Linux; Android 15; Pixel 9) AppleWebKit/537.36 (KHTML, like Gecko) $chrome Mobile Safari/537.36"
        Touch  = 5
        Expect = @("Add to Home screen", "from your home screen")
    },
    @{
        Name   = "Android, Samsung"
        Agent  = "Mozilla/5.0 (Linux; Android 15; SM-S928N) AppleWebKit/537.36 (KHTML, like Gecko) SamsungBrowser/28.0 Chrome/130.0.0.0 Mobile Safari/537.36"
        Touch  = 5
        Expect = @("Add page to", "Home screen")
    },
    @{
        Name   = "Android, LinkedIn app"
        Agent  = "Mozilla/5.0 (Linux; Android 15; Pixel 9 Build/AP3A.241005.015; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 $chrome Mobile Safari/537.36 [LinkedInApp]/9.30.1155"
        Touch  = 5
        Expect = @("Open this page in Chrome first", "Add to Home screen")
    },
    @{
        Name   = "iPhone, Safari"
        Agent  = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Mobile/15E148 Safari/604.1"
        Touch  = 5
        Expect = @("Share", "Add to Home Screen", "from your home screen")
    },
    @{
        Name   = "iPhone, Chrome"
        Agent  = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/141.0.7390.96 Mobile/15E148 Safari/604.1"
        Touch  = 5
        Expect = @("Tap Share, then Add to Home Screen")
    },
    @{
        Name   = "iPhone, LinkedIn app"
        Agent  = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/15E148 [LinkedInApp]/9.31.6183"
        Touch  = 5
        Expect = @("Open this page in Safari first", "Add to Home Screen")
    },
    # iPads ask for the desktop site, so only their touch screen gives them away.
    @{
        Name   = "iPad, Safari"
        Agent  = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15"
        Touch  = 5
        Expect = @("Add to Home Screen")
    },
    @{
        Name   = "Mac, Safari"
        Agent  = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15"
        Touch  = 0
        Expect = @("Add to Dock", "in its own window")
    },
    # Add to Dock came with Safari 17 (macOS Sonoma).
    @{
        Name   = "Mac, Safari 16"
        Agent  = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.6 Safari/605.1.15"
        Touch  = 0
        Expect = @()
    },
    @{
        Name   = "Mac, Chrome"
        Agent  = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) $chrome Safari/537.36"
        Touch  = 0
        Expect = @("install icon", "in its own window")
    },
    @{
        Name   = "Mac, Firefox"
        Agent  = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:150.0) Gecko/20100101 Firefox/150.0"
        Touch  = 0
        Expect = @()
    },
    @{
        Name   = "Chromebook"
        Agent  = "Mozilla/5.0 (X11; CrOS x86_64 16328.65.0) AppleWebKit/537.36 (KHTML, like Gecko) $chrome Safari/537.36"
        Touch  = 10
        Expect = @("install icon", "in its own window")
    },
    @{
        Name   = "Linux, Firefox"
        Agent  = "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:150.0) Gecko/20100101 Firefox/150.0"
        Touch  = 0
        Expect = @()
    },
    @{
        Name   = "Windows, Edge"
        Agent  = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) $chrome Safari/537.36 Edg/141.0.0.0"
        Touch  = 0
        Expect = @("install icon", "in its own window")
    },
    @{
        Name   = "Windows, Chrome"
        Agent  = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) $chrome Safari/537.36"
        Touch  = 0
        Expect = @("install icon", "in its own window")
    },
    # Firefox installs pages on Windows from version 143.
    @{
        Name   = "Windows, Firefox"
        Agent  = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:150.0) Gecko/20100101 Firefox/150.0"
        Touch  = 0
        Expect = @("web apps button", "in its own window")
    },
    @{
        Name   = "Windows, Firefox 142"
        Agent  = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:142.0) Gecko/20100101 Firefox/142.0"
        Touch  = 0
        Expect = @()
    }
)

# The install section as the visitor sees it.
$readInstall = @"
const section = document.getElementById('install');
return {
  hidden: section.hidden,
  text: document.getElementById('install-how').textContent,
  button: !document.getElementById('install-button').hidden
};
"@

# Opens the page afresh, waiting until the new page has loaded.
function Open-Page([string]$Url) {
    try {
        Invoke-PageScript $page "window.__installCheckOld = true; return true;" | Out-Null
    } catch {
        # No page yet.
    }
    Invoke-Cdp $page "Page.navigate" @{ url = $Url } | Out-Null
    $deadline = (Get-Date).AddSeconds(30)
    while ($true) {
        $ready = $false
        try {
            $ready = Invoke-PageScript $page "return !window.__installCheckOld && document.readyState === 'complete' && Boolean(document.getElementById('install'));"
        } catch {
            # The page was still being replaced.
        }
        if ($ready) {
            return
        }
        if ((Get-Date) -gt $deadline) {
            throw "The page at $Url did not load within 30 s."
        }
        Start-Sleep -Milliseconds 200
    }
}

function Set-Visitor([string]$Agent, [int]$Touch) {
    Invoke-Cdp $page "Emulation.setUserAgentOverride" @{ userAgent = $Agent } | Out-Null
    $emulation = @{ enabled = ($Touch -gt 0) }
    if ($Touch -gt 0) {
        $emulation.maxTouchPoints = $Touch
    }
    Invoke-Cdp $page "Emulation.setTouchEmulationEnabled" $emulation | Out-Null
}

function Test-Manifest {
    $manifest = Invoke-Cdp $page "Page.getAppManifest"
    if (-not $manifest.url) {
        $problems.Add("The page names no manifest")
        return
    }
    Add-Line "Manifest $($manifest.url)"
    foreach ($issue in @($manifest.errors)) {
        if ($issue) {
            $problems.Add("Manifest line $($issue.line): $($issue.message)")
        }
    }
    $data = $manifest.data | ConvertFrom-Json
    Add-Line ("  name '{0}', short name '{1}', start {2}, display {3}, theme {4}" -f `
            $data.name, $data.short_name, $data.start_url, $data.display, $data.theme_color)
    if ($data.display -notin @("standalone", "fullscreen", "minimal-ui")) {
        $problems.Add("Manifest: display '$($data.display)' opens the app in a browser tab")
    }
    $icons = @($data.icons)
    foreach ($size in @("192x192", "512x512")) {
        if (-not ($icons | Where-Object { $_.sizes -eq $size -and (-not $_.purpose -or $_.purpose -match "any") })) {
            $problems.Add("Manifest: no $size icon for any purpose, which Chrome needs to install the page")
        }
    }
    if (-not ($icons | Where-Object { $_.purpose -match "maskable" })) {
        $problems.Add("Manifest: no maskable icon, so Android would shrink the icon onto a white circle")
    }

    $installability = Invoke-Cdp $page "Page.getInstallabilityErrors"
    $errors = @($installability.installabilityErrors | Where-Object { $_ })
    if ($errors.Count -eq 0) {
        Add-Line "  The browser finds nothing that stops it from installing the page."
    }
    foreach ($installError in $errors) {
        $arguments = @($installError.errorArguments | ForEach-Object { "$($_.name)=$($_.value)" }) -join ", "
        $problems.Add("Not installable: $($installError.errorId) $arguments".Trim())
    }
}

# Loads every icon the manifest and the page name, as the browser would.
function Test-Icons {
    $icons = Invoke-PageScript $page @"
const manifestUrl = document.querySelector('link[rel=manifest]').href;
const manifest = await fetch(manifestUrl).then(response => response.json());
const wanted = manifest.icons.map(icon => ({
  from: 'manifest', src: new URL(icon.src, manifestUrl).href, sizes: icon.sizes, purpose: icon.purpose || 'any'
}));
for (const link of document.querySelectorAll('link[rel~=icon], link[rel=apple-touch-icon]')) {
  const apple = link.rel === 'apple-touch-icon';
  wanted.push({ from: link.rel, src: link.href, sizes: link.getAttribute('sizes') || (apple ? '180x180' : ''), purpose: apple ? 'apple' : 'any' });
}
const results = [];
for (const icon of wanted) {
  const response = await fetch(icon.src, { cache: 'no-store' });
  const result = Object.assign({}, icon, { status: response.status, type: response.headers.get('content-type') || '' });
  if (response.ok) {
    const bitmap = await createImageBitmap(await response.blob());
    result.size = bitmap.width + 'x' + bitmap.height;
    const context = new OffscreenCanvas(bitmap.width, bitmap.height).getContext('2d');
    context.drawImage(bitmap, 0, 0);
    const pixels = context.getImageData(0, 0, bitmap.width, bitmap.height).data;
    let transparent = 0;
    for (let i = 3; i < pixels.length; i += 4) {
      if (pixels[i] < 255) {
        transparent += 1;
      }
    }
    result.transparent = transparent;
  }
  results.push(result);
}
return results;
"@
    Add-Line ""
    Add-Line "Icons"
    foreach ($icon in @($icons)) {
        $name = $icon.src.Substring($icon.src.LastIndexOf("/") + 1)
        Add-Line ("  {0,-24} {1,-16} {2,-9} {3}, {4} transparent pixels" -f $name, $icon.from, $icon.purpose, $icon.size, $icon.transparent)
        if ($icon.status -ne 200) {
            $problems.Add("Icon $name ($($icon.from)) answered $($icon.status)")
            continue
        }
        if ($icon.type -notmatch "^image/png") {
            $problems.Add("Icon $name came as '$($icon.type)', not image/png")
        }
        if ($icon.sizes -and $icon.size -ne $icon.sizes) {
            $problems.Add("Icon $name is $($icon.size), but $($icon.from) says $($icon.sizes)")
        }
        if ($icon.purpose -match "maskable|apple" -and $icon.transparent -gt 0) {
            $problems.Add("Icon $name has $($icon.transparent) transparent pixels; a system that crops it to its own shape shows them black or white")
        }
    }
}

function Test-InstallSection([string]$Url) {
    Add-Line ""
    Add-Line "The start screen's install section"
    foreach ($case in $cases) {
        Set-Visitor $case.Agent $case.Touch
        Open-Page $Url
        $state = Invoke-PageScript $page $readInstall
        $shown = if ($state.hidden) { "(hidden)" } else { $state.text }
        Add-Line ("  {0,-22} {1}" -f $case.Name, $shown)
        if ($case.Expect.Count -eq 0) {
            if (-not $state.hidden) {
                $problems.Add("$($case.Name): the install section shows, but this browser cannot install pages")
            }
            continue
        }
        if ($state.hidden) {
            $problems.Add("$($case.Name): the install section is hidden")
            continue
        }
        foreach ($phrase in $case.Expect) {
            if (-not $state.text.Contains($phrase)) {
                $problems.Add("$($case.Name): the install section does not say '$phrase'")
            }
        }
        if ($state.button) {
            $problems.Add("$($case.Name): the Install button shows before the browser offered to install")
        }
    }

    # Opened from the home screen, the game runs as an app and needs no steps.
    Set-Visitor $cases[0].Agent $cases[0].Touch
    $standalone = Invoke-Cdp $page "Page.addScriptToEvaluateOnNewDocument" @{ source = Get-InstalledAppScript }
    Open-Page $Url
    $state = Invoke-PageScript $page $readInstall
    Invoke-Cdp $page "Page.removeScriptToEvaluateOnNewDocument" @{ identifier = $standalone.identifier } | Out-Null
    Add-Line ("  {0,-22} {1}" -f "Opened as an app", $(if ($state.hidden) { "(hidden)" } else { $state.text }))
    if (-not $state.hidden) {
        $problems.Add("Opened as an app: the install section still shows")
    }

    # The browser offers to install (Chrome, Edge, Samsung Internet): the
    # card shows the Install button, which asks once, and the steps come back
    # if the visitor says no. A yes says the game is installed.
    Set-Visitor $cases[0].Agent $cases[0].Touch
    Open-Page $Url
    $flow = Invoke-PageScript $page @"
const read = () => { $readInstall };
const offer = (outcome) => {
  const event = new Event('beforeinstallprompt', { cancelable: true });
  event.prompts = 0;
  event.prompt = () => { event.prompts += 1; return Promise.resolve(); };
  event.userChoice = new Promise(resolve => { event.choose = () => resolve({ outcome: outcome, platform: 'web' }); });
  window.dispatchEvent(event);
  return event;
};
const pause = () => new Promise(resolve => setTimeout(resolve, 50));
const refused = offer('dismissed');
const offered = read();
document.getElementById('install-button').click();
document.getElementById('install-button').click();
refused.choose();
await pause();
const afterNo = read();
const accepted = offer('accepted');
document.getElementById('install-button').click();
accepted.choose();
await pause();
window.dispatchEvent(new Event('appinstalled'));
await pause();
return {
  prevented: refused.defaultPrevented, offered: offered, promptsOnTwoClicks: refused.prompts,
  afterNo: afterNo, promptsOnYes: accepted.prompts, installed: read()
};
"@
    Add-Line ("  {0,-22} {1} [Install app]" -f "Offered to install", $flow.offered.text)
    Add-Line ("  {0,-22} {1}" -f "Turned down", $flow.afterNo.text)
    Add-Line ("  {0,-22} {1}" -f "Installed", $flow.installed.text)
    if (-not $flow.prevented) {
        $problems.Add("Offered to install: the page let the browser show its own install bar over the game")
    }
    if (-not $flow.offered.button -or -not $flow.offered.text.StartsWith("Install it")) {
        $problems.Add("Offered to install: no Install button")
    }
    if ($flow.promptsOnTwoClicks -ne 1) {
        $problems.Add("Offered to install: two clicks asked the browser $($flow.promptsOnTwoClicks) times, not once")
    }
    if ($flow.afterNo.button -or -not $flow.afterNo.text.Contains("Add to Home screen")) {
        $problems.Add("Turned down: the steps did not come back in place of the button")
    }
    if ($flow.promptsOnYes -ne 1 -or $flow.installed.button -or -not $flow.installed.text.StartsWith("Installed")) {
        $problems.Add("Installed: the card does not say the game is installed")
    }
}

# The install box sits right under Start (the user's wish, 2026-10-13), so
# visitors see that they can install the game without scrolling: on a small
# and a common phone, with the longest steps (an app's built-in browser) on
# the small one, and on a laptop.
function Test-InstallPlacement([string]$Url) {
    $windowsChrome = $cases | Where-Object { $_.Name -eq "Windows, Chrome" }
    $inApp = $cases | Where-Object { $_.Name -eq "Android, LinkedIn app" }
    $screens = @(
        @{ Name = "Phone 360x640"; Width = 360; Height = 640; Scale = 3; Mobile = $true; Visitor = $inApp },
        @{ Name = "Phone 390x844"; Width = 390; Height = 844; Scale = 3; Mobile = $true; Visitor = $cases[0] },
        @{ Name = "Laptop 1280x720"; Width = 1280; Height = 720; Scale = 1; Mobile = $false; Visitor = $windowsChrome }
    )
    Add-Line ""
    Add-Line "Where the install box shows, before any scrolling"
    foreach ($screen in $screens) {
        Set-Visitor $screen.Visitor.Agent $screen.Visitor.Touch
        Invoke-Cdp $page "Emulation.setDeviceMetricsOverride" @{
            width = $screen.Width; height = $screen.Height; deviceScaleFactor = $screen.Scale; mobile = $screen.Mobile
        } | Out-Null
        Open-Page $Url
        $box = Invoke-PageScript $page @"
const box = document.getElementById('install');
const start = document.getElementById('start').getBoundingClientRect();
const rect = box.getBoundingClientRect();
return { hidden: box.hidden, top: Math.round(rect.top), bottom: Math.round(rect.bottom), screen: innerHeight, below: rect.top >= start.bottom };
"@
        Add-Line ("  {0,-22} {1} ({2})" -f $screen.Name, $(if ($box.hidden) { "hidden" } else { "from $($box.top) to $($box.bottom) px of $($box.screen)" }), $screen.Visitor.Name)
        if ($box.hidden) {
            $problems.Add("$($screen.Name): the install box is hidden")
        } elseif ($box.bottom -gt $box.screen) {
            $problems.Add("$($screen.Name): the install box ends $($box.bottom - $box.screen) px below the first screen")
        } elseif (-not $box.below) {
            $problems.Add("$($screen.Name): the install box is not under the Start button")
        }
    }
    Invoke-Cdp $page "Emulation.clearDeviceMetricsOverride" | Out-Null
}

$url = $liveUrl
$server = $null
$page = $null
try {
    $arguments = @()
    if ($Live) {
        # The live page counts visits; with GoatCounter's script and counter
        # unreachable it sends nothing.
        $arguments = @("--host-resolver-rules=`"MAP *.goatcounter.com ~NOTFOUND, MAP gc.zgo.at ~NOTFOUND`"")
        Add-Line "Live site $liveUrl"
    } else {
        if (-not $SkipBuild) {
            & (Join-Path $PSScriptRoot "build_web.ps1")
        }
        if (-not (Test-Path -LiteralPath (Join-Path $siteDir "index.html"))) {
            throw "No web build in $siteDir; run without -SkipBuild."
        }
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
        Add-Line "Local build $siteDir at $url"
    }

    $page = Start-HeadlessBrowser $Browser "install" $arguments
    Invoke-Cdp $page "Page.enable" | Out-Null
    # The headless browser offers to install the page itself a moment after
    # it loads, which would turn the steps into the Install button partway
    # through a case. Its offers are counted and kept from the page; the
    # cases offer their own.
    Invoke-Cdp $page "Page.addScriptToEvaluateOnNewDocument" @{
        source = "window.__browserOffers = 0; window.addEventListener('beforeinstallprompt', event => { if (event.isTrusted) { window.__browserOffers += 1; event.preventDefault(); event.stopImmediatePropagation(); } }, true);"
    } | Out-Null
    Open-Page $url
    Test-Manifest
    $offered = $false
    $deadline = (Get-Date).AddSeconds(5)
    while (-not $offered -and (Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 250
        $offered = Invoke-PageScript $page "return window.__browserOffers > 0;"
    }
    if ($offered) {
        Add-Line "  The browser offered to install it (beforeinstallprompt), so Chrome and Edge show the Install button."
    } else {
        Add-Line "  The browser did not offer to install it within 5 s; it may wait until a visitor has used the page."
    }
    Test-Icons
    Test-InstallSection $url
    Test-InstallPlacement $url
} finally {
    Stop-HeadlessBrowser $page
    if ($server -and -not $server.HasExited) {
        Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue
    }
}

Add-Line ""
if ($problems.Count -eq 0) {
    Add-Line "The page installs as an app, and the start screen tells each browser how."
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
