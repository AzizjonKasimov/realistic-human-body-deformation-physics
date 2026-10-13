# Drives a headless Edge or Chrome over the DevTools protocol, for the checks
# that need a real browser page: parity.ps1 (the self-test) and
# analytics_check.ps1 (visit counts). Dot-source it after common.ps1.

function Find-Browser([string]$Path) {
    if ($Path) {
        if (-not (Test-Path -LiteralPath $Path)) {
            throw "No browser at $Path"
        }
        return $Path
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

# A script for Page.addScriptToEvaluateOnNewDocument that makes the page see
# itself running as an installed app opened from the home screen
# (display-mode: standalone), which a headless browser cannot open it as.
function Get-InstalledAppScript {
    return "const matchMedia = window.matchMedia.bind(window); window.matchMedia = query => /display-mode:\s*standalone/.test(query) ? { matches: true, media: query, onchange: null, addListener() {}, removeListener() {}, addEventListener() {}, removeEventListener() {}, dispatchEvent() { return false; } } : matchMedia(query);"
}

# Starts a headless browser on a fresh profile and connects to its page. Pass
# the result to the other functions here, and to Stop-HeadlessBrowser in a
# finally block. A script block in its OnEvent receives each DevTools event
# that arrives while a command waits for its reply.
function Start-HeadlessBrowser([string]$Path, [string]$Name, [string[]]$Arguments = @()) {
    $browserPath = Find-Browser $Path
    $profileDir = Join-Path ([IO.Path]::GetTempPath()) "rp-$Name-$PID"
    if (Test-Path -LiteralPath $profileDir) {
        Remove-Item -LiteralPath $profileDir -Recurse -Force
    }
    $session = [pscustomobject]@{
        Process    = $null
        ProfileDir = $profileDir
        Socket     = $null
        NextId     = 1
        OnEvent    = $null
    }
    try {
        # Port 0 lets the browser pick a free DevTools port, which it writes
        # to DevToolsActivePort in its profile.
        $session.Process = Start-Process $browserPath -PassThru -ArgumentList (@(
                "--headless=new", "--remote-debugging-port=0", "--user-data-dir=`"$profileDir`"",
                "--no-first-run", "--no-default-browser-check", "--disable-extensions",
                "--enable-unsafe-swiftshader", "--window-size=1280,720"
            ) + @($Arguments | Where-Object { $_ }) + @("about:blank"))
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
        $session.Socket = [System.Net.WebSockets.ClientWebSocket]::new()
        $session.Socket.Options.KeepAliveInterval = [TimeSpan]::Zero
        $session.Socket.ConnectAsync([Uri]$page.webSocketDebuggerUrl, [Threading.CancellationToken]::None).GetAwaiter().GetResult() | Out-Null
        return $session
    } catch {
        Stop-HeadlessBrowser $session
        throw
    }
}

function Stop-HeadlessBrowser($Session) {
    if (-not $Session) {
        return
    }
    if ($Session.Socket) {
        $Session.Socket.Dispose()
    }
    if ($Session.Process -and -not $Session.Process.HasExited) {
        Stop-Process -Id $Session.Process.Id -Force -ErrorAction SilentlyContinue
    }
    # The browser's helper processes hold the profile for a moment.
    for ($attempt = 0; $attempt -lt 20 -and (Test-Path -LiteralPath $Session.ProfileDir); $attempt++) {
        Start-Sleep -Milliseconds 250
        Remove-Item -LiteralPath $Session.ProfileDir -Recurse -Force -ErrorAction SilentlyContinue
    }
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

# Sends one DevTools command without waiting for its reply and returns its id.
function Send-Cdp($Session, [string]$Method, [hashtable]$Params = @{}) {
    $id = $Session.NextId
    $Session.NextId = $id + 1
    $request = @{ id = $id; method = $Method; params = $Params } | ConvertTo-Json -Depth 8 -Compress
    $bytes = [Text.Encoding]::UTF8.GetBytes($request)
    $segment = [ArraySegment[byte]]::new($bytes)
    $Session.Socket.SendAsync($segment, [System.Net.WebSockets.WebSocketMessageType]::Text, $true, [Threading.CancellationToken]::None).GetAwaiter().GetResult() | Out-Null
    return $id
}

# Sends one DevTools command and returns its result. Events that arrive in
# the meantime go to the session's OnEvent; replies to commands sent with
# Send-Cdp are dropped, with a warning when the command failed.
function Invoke-Cdp($Session, [string]$Method, [hashtable]$Params = @{}) {
    $id = Send-Cdp $Session $Method $Params
    while ($true) {
        $message = Receive-CdpMessage $Session.Socket | ConvertFrom-Json -Depth 64
        if ($message.id -eq $id) {
            if ($message.error) {
                throw "$Method failed: $($message.error.message)"
            }
            return $message.result
        }
        if ($message.method) {
            if ($Session.OnEvent) {
                & $Session.OnEvent $message
            }
        } elseif ($message.error) {
            Write-Warning "DevTools command $($message.id) failed: $($message.error.message)"
        }
    }
}

# Runs JavaScript in the page as the body of an async function and returns
# what it returns, passed through JSON.
function Invoke-PageScript($Session, [string]$Body) {
    $result = Invoke-Cdp $Session "Runtime.evaluate" @{
        expression    = "(async () => { $Body })().then(value => JSON.stringify(value === undefined ? null : value))"
        awaitPromise  = $true
        returnByValue = $true
    }
    if ($result.exceptionDetails) {
        $details = $result.exceptionDetails
        $reason = if ($details.exception.description) { $details.exception.description } else { $details.text }
        throw "Page script failed: $reason"
    }
    return $result.result.value | ConvertFrom-Json -Depth 32
}
