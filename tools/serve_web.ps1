param(
    [int]$Port = 8080,
    [string]$Root = ""
)

. "$PSScriptRoot/common.ps1"

if (-not $Root) {
    $Root = Join-Path (Get-RepoRoot) "target/web"
}
if (-not (Test-Path -LiteralPath (Join-Path $Root "index.html"))) {
    throw "No web build found in $Root. Run .\tools\build_web.ps1 first."
}
$Root = (Resolve-Path -LiteralPath $Root).Path
$rootPrefix = $Root.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar

# Browsers only stream-compile WebAssembly served as application/wasm, which a
# plain file:// page or many quick servers do not provide.
$contentTypes = @{
    ".html" = "text/html; charset=utf-8"
    ".js"   = "text/javascript; charset=utf-8"
    ".wasm" = "application/wasm"
}

$url = "http://localhost:$Port/"
$listener = [System.Net.HttpListener]::new()
$listener.Prefixes.Add($url)
$listener.Start()
Write-Host "Serving $Root"
Write-Host "Open $url in a browser (add ?stats for frame timings). Press Ctrl+C to stop."

try {
    while ($listener.IsListening) {
        # Wait in short slices instead of blocking in GetContext() so Ctrl+C can stop the loop.
        $pending = $listener.GetContextAsync()
        while (-not $pending.Wait(250)) { }
        $context = $pending.Result
        $response = $context.Response
        try {
            $relative = [Uri]::UnescapeDataString($context.Request.Url.AbsolutePath).TrimStart('/')
            if (-not $relative) {
                $relative = "index.html"
            }
            $path = [IO.Path]::GetFullPath((Join-Path $Root $relative))
            $type = $contentTypes[[IO.Path]::GetExtension($path).ToLowerInvariant()]
            $insideRoot = $path.StartsWith($rootPrefix, [StringComparison]::OrdinalIgnoreCase)
            if ($insideRoot -and $type -and (Test-Path -LiteralPath $path -PathType Leaf)) {
                $bytes = [IO.File]::ReadAllBytes($path)
                $response.ContentType = $type
                $response.Headers["Cache-Control"] = "no-store"
                $response.ContentLength64 = $bytes.Length
                # A HEAD request, such as a check that the server is up, gets
                # the headers only; writing a body to it throws.
                if ($context.Request.HttpMethod -ne "HEAD") {
                    $response.OutputStream.Write($bytes, 0, $bytes.Length)
                }
            } else {
                $response.StatusCode = 404
            }
        } catch {
            # One failed request, such as a client that went away mid-download,
            # must not stop the server.
            Write-Warning "$($context.Request.HttpMethod) $($context.Request.Url.AbsolutePath): $($_.Exception.Message)"
        } finally {
            try { $response.Close() } catch { }
        }
    }
} finally {
    $listener.Stop()
    $listener.Close()
}
