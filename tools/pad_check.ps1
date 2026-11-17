<#
.SYNOPSIS
Plays the site's Fight page with fake game controllers in a headless browser:
a DualSense for the player, then an Xbox controller for a friend.

.DESCRIPTION
Serves target\web, as built with build_web.ps1 -Fight, and opens the Fight
page in headless Edge (or Chrome) with the browser's Gamepad API replaced,
before the page loads, by fake controllers that this script presses. It
checks that:

- the start card asks for a controller, then names the DualSense once it is
  connected and shows its buttons beside the keys;
- cross on it starts the fight;
- the game gives it the left fighter (window.__fightPads), and the stick held
  right walks in, square jabs, R1 blocks, the stick down ducks and up leans
  back (the page counts fight-step, fight-punch, fight-block, fight-duck,
  fight-lean, and fight-controller), and the buttons show its symbols;
- a second controller, an Xbox one, gets the right fighter, and its Y crosses
  as player 2 (fight-two-players);
- clean punches rumble the controllers of both players, the one whose punch
  landed and the one hit (each fake controller records its rumbles);
- a key of the left fighter's turns the buttons back to keys, CREATE moves a
  controller to the other fighter, and the page logs no errors.

Screenshots go to output\pad_check. No agent can press a real controller's
buttons: this checks the page's and the game's side, not the browser's
reading of a real controller, its rumble motors, or whether a real press
starts the sound (Chrome and Edge treat one as a gesture; a fake press is not
one, so the browser runs here without its autoplay rule).

.PARAMETER Size
The window in CSS pixels, WIDTHxHEIGHT.

.PARAMETER Port
Port to serve the web build on while the check runs.

.PARAMETER Browser
Path to a Chromium browser; Edge or Chrome is found by default.

.EXAMPLE
.\tools\build_web.ps1 -Fight ..\realistic_physics_fight; .\tools\pad_check.ps1
The Fight page with a fake DualSense and Xbox controller at 1280x720.
#>
param(
    [string]$Size = "1280x720",
    [int]$Port = 8094,
    [string]$Browser = ""
)

. "$PSScriptRoot\common.ps1"
. "$PSScriptRoot\headless.ps1"

$repoRoot = Get-RepoRoot
$out = Join-Path $repoRoot "output\pad_check"
New-Item -ItemType Directory -Force $out | Out-Null
$width, $height = $Size -split "[xX]" | ForEach-Object { [int]$_ }
if (-not $width -or -not $height) {
    throw "-Size should look like 1280x720."
}
$siteDir = Join-Path $repoRoot "target\web"
if (-not (Test-Path -LiteralPath (Join-Path $siteDir "fight\index.html"))) {
    throw "No Fight page in $siteDir; build it with .\tools\build_web.ps1 -Fight <the game's checkout>."
}

$problems = [System.Collections.Generic.List[string]]::new()
$errors = [System.Collections.Generic.List[string]]::new()

# Fake controllers in place of the browser's own, before the page's scripts
# run: window.__fakePads connects one, presses its buttons (by their place in
# the standard layout: 0 cross or A, 2 square or X, 3 triangle or Y, 5 R1 or
# RB, 8 CREATE or VIEW, 9 OPTIONS or MENU) and moves its left stick, and every
# rumble the page asks of one lands in window.__rumbles.
$fakeGamepads = @"
(() => {
  const slots = [null, null, null, null];
  window.__rumbles = [];
  const button = () => ({ pressed: false, touched: false, value: 0 });
  window.__fakePads = {
    connect(index, id) {
      const pad = {
        id, index, connected: true, mapping: "standard", timestamp: performance.now(),
        axes: [0, 0, 0, 0], buttons: Array.from({ length: 17 }, button),
        vibrationActuator: {
          type: "dual-rumble",
          playEffect(type, params) {
            window.__rumbles.push(Object.assign({ index, type }, params));
            return Promise.resolve("complete");
          },
          reset() { return Promise.resolve("complete"); }
        }
      };
      slots[index] = pad;
      const event = new Event("gamepadconnected");
      event.gamepad = pad;
      window.dispatchEvent(event);
    },
    press(index, which, down) {
      const pressed = slots[index].buttons[which];
      pressed.pressed = down;
      pressed.touched = down;
      pressed.value = down ? 1 : 0;
      slots[index].timestamp = performance.now();
    },
    stick(index, x, y) {
      slots[index].axes[0] = x;
      slots[index].axes[1] = y;
      slots[index].timestamp = performance.now();
    }
  };
  Object.defineProperty(Navigator.prototype, "getGamepads", {
    configurable: true, writable: true, value: function () { return slots.slice(); }
  });
})();
"@

# Waits while the page keeps drawing frames.
function Wait-Page([double]$Seconds) {
    $deadline = (Get-Date).AddSeconds($Seconds)
    while ((Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 100
        Invoke-PageScript $page "return true;" | Out-Null
    }
}

# Waits until the page answers `$Script` with true.
function Wait-Until([string]$Script, [string]$What) {
    $deadline = (Get-Date).AddSeconds(30)
    while ($true) {
        $ready = $false
        try {
            $ready = Invoke-PageScript $page $Script
        } catch {
            # The page was still being replaced.
        }
        if ($ready) {
            return
        }
        if ((Get-Date) -gt $deadline) {
            throw "$What within 30 s."
        }
        Start-Sleep -Milliseconds 250
    }
}

function Save-Screenshot([string]$Name) {
    $shot = Invoke-Cdp $page "Page.captureScreenshot" @{ format = "png" }
    $path = Join-Path $out $Name
    [IO.File]::WriteAllBytes($path, [Convert]::FromBase64String($shot.data))
    Write-Host "wrote $path"
}

# What the page counted so far (web/analytics.js), by name.
function Get-Counted {
    return @(Invoke-PageScript $page "return (window.__analytics || []).map(event => event.name);")
}

# A fake controller's button, pressed and let go, or held for a while.
function Send-Press([int]$Pad, [int]$Button, [double]$Hold = 0.15, [double]$Then = 0.3) {
    Invoke-PageScript $page "window.__fakePads.press($Pad, $Button, true); return true;" | Out-Null
    Wait-Page $Hold
    Invoke-PageScript $page "window.__fakePads.press($Pad, $Button, false); return true;" | Out-Null
    Wait-Page $Then
}

# A fake controller's left stick held at (X, Y) for a while, then let go.
function Send-Stick([int]$Pad, [double]$X, [double]$Y, [double]$Hold, [double]$Then = 0.3) {
    Invoke-PageScript $page "window.__fakePads.stick($Pad, $X, $Y); return true;" | Out-Null
    Wait-Page $Hold
    Invoke-PageScript $page "window.__fakePads.stick($Pad, 0, 0); return true;" | Out-Null
    Wait-Page $Then
}

# Which fighter each controller plays, and whose buttons the screen shows, as
# the game told the page.
function Get-Pads {
    return Invoke-PageScript $page "return window.__fightPads || null;"
}

$server = $null
$page = $null
$fps = 0
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

    # A fake press is no gesture, so without this rule the page's sound,
    # started by a press of cross, would log the browser's warning.
    $page = Start-HeadlessBrowser $Browser "pads" @("--autoplay-policy=no-user-gesture-required")
    $page.OnEvent = {
        param($message)
        $params = $message.params
        if ($message.method -eq "Runtime.exceptionThrown") {
            $errors.Add("exception: $($params.exceptionDetails.text) $($params.exceptionDetails.exception.description)")
        } elseif ($message.method -eq "Runtime.consoleAPICalled" -and $params.type -in @("error", "warning", "assert")) {
            $errors.Add("console $($params.type): $(($params.args | ForEach-Object { $_.value ?? $_.description }) -join ' ')")
        }
    }
    Invoke-Cdp $page "Runtime.enable" | Out-Null
    Invoke-Cdp $page "Page.enable" | Out-Null
    Invoke-Cdp $page "Page.addScriptToEvaluateOnNewDocument" @{ source = $fakeGamepads } | Out-Null
    Invoke-Cdp $page "Emulation.setDeviceMetricsOverride" @{
        width = $width; height = $height; deviceScaleFactor = 1; mobile = $false
    } | Out-Null
    Invoke-Cdp $page "Page.navigate" @{ url = $url + "fight/?stats" } | Out-Null
    Wait-Until "return document.readyState === 'complete' && !!document.getElementById('pads');" "The Fight page did not load"

    # The start card before and after a controller connects.
    $before = Invoke-PageScript $page "return document.getElementById('pads').textContent;"
    if ($before -notmatch "press any button") {
        $problems.Add("Before any controller, the start card said '$before'.")
    }
    Invoke-PageScript $page "window.__fakePads.connect(0, 'DualSense Wireless Controller (STANDARD GAMEPAD Vendor: 054c Product: 0ce6)'); return true;" | Out-Null
    Wait-Page 0.5
    $card = Invoke-PageScript $page @"
const card = document.querySelector('#gate .card');
return {
  line: document.getElementById('pads').textContent,
  shown: card.classList.contains('has-pad'),
  pictures: card.querySelectorAll('.controls .pad svg').length,
  labels: Array.from(card.querySelectorAll('.controls .pad svg')).map(svg => svg.getAttribute('aria-label'))
};
"@
    if ($card.line -notmatch "DualSense connected" -or -not $card.shown) {
        $problems.Add("With a DualSense connected, the start card said '$($card.line)' (its buttons shown: $($card.shown)).")
    }
    foreach ($label in @("square", "triangle", "R1", "R2", "OPTIONS", "CREATE", "cross")) {
        if ($card.labels -notcontains $label) {
            $problems.Add("The start card shows no $label button (it shows: $($card.labels -join ', ')).")
        }
    }
    Invoke-Cdp $page "Emulation.setDeviceMetricsOverride" @{
        width = $width; height = [Math]::Max($height, 1100); deviceScaleFactor = 1; mobile = $false
    } | Out-Null
    Save-Screenshot "card-dualsense.png"
    Invoke-Cdp $page "Emulation.setDeviceMetricsOverride" @{
        width = $width; height = $height; deviceScaleFactor = 1; mobile = $false
    } | Out-Null

    # Cross starts the fight, as the Start button does.
    Send-Press 0 0 0.15 0.2
    Wait-Until "return !!window.wasm_exports;" "Cross on the DualSense did not start the fight"
    Wait-Page 1.5
    $fps = Invoke-PageScript $page @"
return await new Promise(done => {
  const start = performance.now();
  let frames = 0;
  const frame = () => { frames++; if (performance.now() - start < 1000) { requestAnimationFrame(frame); } else { done(frames); } };
  requestAnimationFrame(frame);
});
"@

    # The DualSense plays the left fighter: walk in, jab, block, duck, lean.
    Send-Stick 0 0.9 0 1.6
    Send-Press 0 2 0.12 0.6
    Send-Press 0 5 0.6 0.3
    Send-Stick 0 0 0.9 0.6
    Save-Screenshot "fight-dualsense-duck.png"
    Send-Stick 0 0 -0.9 0.6
    $pads = Get-Pads
    $left = @($pads.pads | Where-Object { $_.slot -eq 0 })
    if (-not $left -or $left[0].kind -ne "playstation" -or $left[0].fighter -ne 0) {
        $problems.Add("The game does not have the DualSense playing the left fighter: $($pads | ConvertTo-Json -Compress -Depth 4).")
    }
    if ($pads.prompts -ne "playstation") {
        $problems.Add("The buttons show $($pads.prompts), not the DualSense's.")
    }
    Save-Screenshot "fight-dualsense.png"

    # A friend's Xbox controller takes the right fighter, and its Y crosses.
    Invoke-PageScript $page "window.__fakePads.connect(1, 'Xbox 360 Controller (XInput STANDARD GAMEPAD)'); return true;" | Out-Null
    Wait-Page 0.3
    Send-Press 1 3 0.12 0.8
    $pads = Get-Pads
    $right = @($pads.pads | Where-Object { $_.slot -eq 1 })
    if (-not $right -or $right[0].kind -ne "xbox" -or $right[0].fighter -ne 1) {
        $problems.Add("The game does not have the Xbox controller playing the right fighter: $($pads | ConvertTo-Json -Compress -Depth 4).")
    }

    # The DualSense steps in and jabs the friend, who throws nothing more,
    # until a punch lands clean: the DualSense ticks with its light motor
    # alone, and the friend's controller thumps with its heavy one too.
    Send-Stick 0 0.9 0 1.2
    Invoke-PageScript $page "window.__rumbles.length = 0; return true;" | Out-Null
    for ($jab = 0; $jab -lt 8; $jab++) {
        Send-Press 0 2 0.1 0.45
        $rumbled = Invoke-PageScript $page "return [0, 1].map(index => window.__rumbles.some(rumble => rumble.index === index));"
        if ($rumbled[0] -and $rumbled[1]) {
            break
        }
    }
    Save-Screenshot "fight-two-controllers.png"
    $rumbles = @(Invoke-PageScript $page "return window.__rumbles;")
    $tick = @($rumbles | Where-Object { $_.index -eq 0 -and $_.type -eq "dual-rumble" -and $_.strongMagnitude -eq 0 -and $_.weakMagnitude -gt 0 })
    $thump = @($rumbles | Where-Object { $_.index -eq 1 -and $_.type -eq "dual-rumble" -and $_.strongMagnitude -ge 0.5 })
    if ($tick.Count -eq 0 -or $thump.Count -eq 0) {
        $problems.Add("A clean jab did not tick the DualSense and thump the Xbox controller (rumbles: $($rumbles | ConvertTo-Json -Compress -Depth 3)).")
    }

    # A key of the left fighter's turns its buttons back to keys.
    foreach ($type in @("keyDown", "keyUp")) {
        Invoke-Cdp $page "Input.dispatchKeyEvent" @{
            type = $type; key = "j"; code = "KeyJ"; windowsVirtualKeyCode = 74; nativeVirtualKeyCode = 74
        } | Out-Null
        Wait-Page 0.1
    }
    Wait-Page 0.4
    if ((Get-Pads).prompts -ne "keys") {
        $problems.Add("After a press of J the buttons still show $((Get-Pads).prompts).")
    }
    Save-Screenshot "fight-keys-again.png"

    # CREATE moves the DualSense to the right fighter.
    Send-Press 0 8 0.12 0.4
    $moved = @((Get-Pads).pads | Where-Object { $_.slot -eq 0 })
    if (-not $moved -or $moved[0].fighter -ne 1) {
        $problems.Add("CREATE did not move the DualSense to the right fighter: $((Get-Pads) | ConvertTo-Json -Compress -Depth 4).")
    }

    # OPTIONS pauses.
    Send-Press 1 9 0.12 0.4
    Save-Screenshot "fight-paused.png"

    $counted = Get-Counted
    foreach ($name in @("fight-start", "fight-loaded", "fight-controller", "fight-step", "fight-punch", "fight-block", "fight-duck", "fight-lean", "fight-two-players")) {
        if ($counted -notcontains $name) {
            $problems.Add("The page did not count $name (it counted: $($counted -join ', ')).")
        }
    }
} finally {
    Stop-HeadlessBrowser $page
    if ($server -and -not $server.HasExited) {
        Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue
    }
}

foreach ($line in $errors) {
    $problems.Add("The page logged $line")
}
Write-Host ""
Write-Host "Fake controllers on the Fight page at $Size, $fps fps:"
if ($problems.Count -eq 0) {
    Write-Host "  the start card named the DualSense and showed its buttons, cross started the fight, the DualSense walked, jabbed, blocked, ducked and leaned back as the left fighter, the Xbox controller crossed as the right, a clean punch rumbled both (the one hit harder), J brought the keys back, CREATE switched fighters, OPTIONS paused, and the console stayed clean."
} else {
    $problems | ForEach-Object { Write-Host "  FAIL: $_" }
    exit 1
}
