<#
.SYNOPSIS
Checks that the native app's sound reaches the speakers: starts the app,
plays a control click and a sledgehammer swing in its window, and reads the
app's own level meter in the Windows mixer.

.DESCRIPTION
Nobody has to listen. The script opens realistic_physics.exe (its window
shows for a few seconds), waits for the sound to load, and sends the window
key presses and a swing as window messages, so the real mouse and keyboard
are left alone. Meanwhile it samples the peak level of the app's audio
session (the meter in the Windows volume mixer). It then closes the app and
reports each phase's loudest peak:

  idle         nothing happening: silent
  click        the H key: a control's soft click
  swing        a sledgehammer swing into the chest: a whoosh and a thud
  muted swing  the same after M: silent

It exits nonzero if a phase is out of its range. Read-only apart from the app
it starts; it never touches other processes or windows.

.PARAMETER Exe
The app to check. Defaults to the repository root's realistic_physics.exe.
#>
param(
    [string]$Exe = ""
)

. "$PSScriptRoot\common.ps1"

if (-not $Exe) {
    $Exe = Join-Path (Get-RepoRoot) "realistic_physics.exe"
}
if (-not (Test-Path -LiteralPath $Exe)) {
    throw "No app at $Exe. Build it with .\tools\build_app.ps1 first."
}

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public static class SoundCheck
{
    [ComImport, Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IMMDeviceEnumerator
    {
        int EnumAudioEndpoints();
        [PreserveSig] int GetDefaultAudioEndpoint(int dataFlow, int role, out IMMDevice device);
    }

    [ComImport, Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IMMDevice
    {
        [PreserveSig] int Activate(ref Guid iid, int context, IntPtr parameters, [MarshalAs(UnmanagedType.IUnknown)] out object activated);
    }

    [ComImport, Guid("77AA99A0-1BD6-484F-8BC7-2C654C9A9B6F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IAudioSessionManager2
    {
        int GetAudioSessionControl();
        int GetSimpleAudioVolume();
        [PreserveSig] int GetSessionEnumerator(out IAudioSessionEnumerator sessions);
    }

    [ComImport, Guid("E2F5BB11-0570-40CA-ACDD-3AA01277DEE8"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IAudioSessionEnumerator
    {
        [PreserveSig] int GetCount(out int count);
        [PreserveSig] int GetSession(int index, [MarshalAs(UnmanagedType.IUnknown)] out object session);
    }

    [ComImport, Guid("bfb7ff88-7239-4fc9-8fa2-07c950be9c6d"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IAudioSessionControl2
    {
        // IAudioSessionControl
        int GetState();
        int GetDisplayName();
        int SetDisplayName();
        int GetIconPath();
        int SetIconPath();
        int GetGroupingParam();
        int SetGroupingParam();
        int RegisterAudioSessionNotification();
        int UnregisterAudioSessionNotification();
        // IAudioSessionControl2
        int GetSessionIdentifier();
        int GetSessionInstanceIdentifier();
        [PreserveSig] int GetProcessId(out uint processId);
    }

    [ComImport, Guid("C02216F6-8C67-4B5B-9D00-D008E73E0064"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IAudioMeterInformation
    {
        [PreserveSig] int GetPeakValue(out float peak);
    }

    [ComImport, Guid("BCDE0395-E52F-467C-8E3D-C4579291692E")]
    class MMDeviceEnumerator { }

    [StructLayout(LayoutKind.Sequential)]
    public struct Rect { public int Left, Top, Right, Bottom; }

    [DllImport("user32.dll")] static extern bool PostMessage(IntPtr window, uint message, IntPtr wParam, IntPtr lParam);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr window, out Rect rect);
    // Millisecond sleeps, for sampling the meter and pacing the swing.
    [DllImport("winmm.dll")] public static extern uint timeBeginPeriod(uint milliseconds);
    [DllImport("winmm.dll")] public static extern uint timeEndPeriod(uint milliseconds);

    /// The level meter of the audio session the process plays through on
    /// the default output, or null while it has none.
    public static object Meter(int processId)
    {
        var enumerator = (IMMDeviceEnumerator)new MMDeviceEnumerator();
        IMMDevice device;
        if (enumerator.GetDefaultAudioEndpoint(0, 1, out device) != 0) return null; // render, multimedia
        var managerId = typeof(IAudioSessionManager2).GUID;
        object activated;
        if (device.Activate(ref managerId, 23, IntPtr.Zero, out activated) != 0) return null; // CLSCTX_ALL
        IAudioSessionEnumerator sessions;
        if (((IAudioSessionManager2)activated).GetSessionEnumerator(out sessions) != 0) return null;
        int count;
        sessions.GetCount(out count);
        for (int i = 0; i < count; i++)
        {
            object session;
            if (sessions.GetSession(i, out session) != 0) continue;
            uint owner;
            if (((IAudioSessionControl2)session).GetProcessId(out owner) == 0 && owner == processId)
                return session;
        }
        return null;
    }

    public static float Peak(object meter)
    {
        float peak;
        return ((IAudioMeterInformation)meter).GetPeakValue(out peak) == 0 ? peak : -1f;
    }

    const uint KeyDown = 0x0100, KeyUp = 0x0101, MouseMove = 0x0200, ButtonDown = 0x0201, ButtonUp = 0x0202;

    /// A key press: miniquad reads the key from the scan code in lParam.
    public static void Key(IntPtr window, int virtualKey, int scanCode)
    {
        PostMessage(window, KeyDown, (IntPtr)virtualKey, (IntPtr)(1 | (scanCode << 16)));
        PostMessage(window, KeyUp, (IntPtr)virtualKey, (IntPtr)(1 | (scanCode << 16) | (1 << 30) | unchecked((int)0x80000000)));
    }

    public static void Move(IntPtr window, int x, int y, bool down)
    {
        PostMessage(window, MouseMove, (IntPtr)(down ? 1 : 0), (IntPtr)((y << 16) | (x & 0xFFFF)));
    }

    public static void Button(IntPtr window, bool down)
    {
        PostMessage(window, down ? ButtonDown : ButtonUp, (IntPtr)(down ? 1 : 0), IntPtr.Zero);
    }
}
"@

$app = Start-Process -FilePath $Exe -PassThru
[void][SoundCheck]::timeBeginPeriod(1)
try {
    $deadline = [DateTime]::Now.AddSeconds(15)
    while ($app.MainWindowHandle -eq 0 -and [DateTime]::Now -lt $deadline) {
        Start-Sleep -Milliseconds 100
        $app.Refresh()
    }
    if ($app.MainWindowHandle -eq 0) {
        throw "The app opened no window within 15 s."
    }
    $window = $app.MainWindowHandle
    # The sound bank loads on the first frames; give the app a moment.
    Start-Sleep -Milliseconds 1500
    $meter = [SoundCheck]::Meter($app.Id)
    if (-not $meter) {
        throw "The app has no audio session on the default output device, so its sound cannot reach the speakers."
    }
    $client = New-Object SoundCheck+Rect
    [void][SoundCheck]::GetClientRect($window, [ref]$client)
    $width = $client.Right - $client.Left
    $height = $client.Bottom - $client.Top

    # The meter is read every couple of milliseconds, also while an action
    # waits between its steps, since a meter reading covers only the last
    # few milliseconds of sound.
    $script:loudest = 0.0
    function Wait-Sampling([int]$Milliseconds) {
        $clock = [Diagnostics.Stopwatch]::StartNew()
        while ($clock.ElapsedMilliseconds -lt $Milliseconds) {
            $script:loudest = [Math]::Max($script:loudest, [SoundCheck]::Peak($meter))
            [Threading.Thread]::Sleep(2)
        }
    }
    function Get-LoudestPeak([int]$Milliseconds, [scriptblock]$Action = {}) {
        $script:loudest = 0.0
        $clock = [Diagnostics.Stopwatch]::StartNew()
        & $Action
        Wait-Sampling ([Math]::Max(0, $Milliseconds - $clock.ElapsedMilliseconds))
        return $script:loudest
    }

    # A sledgehammer swing from the left of the body into the chest, as
    # hammer_firm_swing plays it in a 1280x720 window: 8 steps of 1/60 s.
    $swing = {
        $y = [int]($height * 0.358)
        $from = [int]($width * 0.366)
        $to = [int]($width * 0.52)
        [SoundCheck]::Move($window, $from, $y, $false)
        Wait-Sampling 600
        [SoundCheck]::Button($window, $true)
        foreach ($step in 1..8) {
            Wait-Sampling 17
            [SoundCheck]::Move($window, [int]($from + ($to - $from) * $step / 8), $y, $true)
        }
        Wait-Sampling 500
        [SoundCheck]::Button($window, $false)
    }

    $idle = Get-LoudestPeak 1000
    $click = Get-LoudestPeak 500 { [SoundCheck]::Key($window, 0x48, 0x23) } # H
    $hit = Get-LoudestPeak 2000 $swing
    [SoundCheck]::Key($window, 0x52, 0x13) # R: a fresh body
    [SoundCheck]::Key($window, 0x4D, 0x32) # M: mute
    Start-Sleep -Milliseconds 400
    $muted = Get-LoudestPeak 2000 $swing
} finally {
    [void][SoundCheck]::timeEndPeriod(1)
    if (-not $app.HasExited) {
        [void]$app.CloseMainWindow()
        if (-not $app.WaitForExit(3000)) {
            $app.Kill()
        }
    }
}

$checks = @(
    @{ Name = "idle"; Peak = $idle; Pass = $idle -lt 0.001; Expect = "silent" },
    @{ Name = "click"; Peak = $click; Pass = $click -gt 0.01; Expect = "a soft click" },
    @{ Name = "swing"; Peak = $hit; Pass = $hit -gt 0.1; Expect = "a whoosh and a thud" },
    @{ Name = "muted swing"; Peak = $muted; Pass = $muted -lt 0.001; Expect = "silent" }
)
Write-Host ""
Write-Host "App audio session peaks (1.0 is full scale), $width x $height window:"
foreach ($check in $checks) {
    $verdict = if ($check.Pass) { "ok" } else { "FAIL" }
    Write-Host ("  {0,-12} {1,7:N4}  {2,-4} expected {3}" -f $check.Name, $check.Peak, $verdict, $check.Expect)
}
if ($checks | Where-Object { -not $_.Pass }) {
    Write-Host "FAIL: the app's sound did not behave as expected."
    exit 1
}
Write-Host "PASS: the app's sound reaches the speakers and mutes."
