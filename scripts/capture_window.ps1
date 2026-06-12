# Capture the Proscenium window into a PNG. Forces the window topmost first
# (PrintWindow can't capture transparent windows with d3d11 swapchains, so
# this copies from the actual screen).
param([string]$OutFile = "$env:TEMP\proscenium-window.png")

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Win32Capture {
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hWnd, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
"@

$proc = Get-Process proscenium -ErrorAction Stop | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
# HWND_TOPMOST = -1; SWP_NOMOVE|SWP_NOSIZE|SWP_NOACTIVATE = 0x0013
[Win32Capture]::SetWindowPos($proc.MainWindowHandle, [IntPtr](-1), 0, 0, 0, 0, 0x0013) | Out-Null
Start-Sleep -Milliseconds 800
$rect = New-Object Win32Capture+RECT
[Win32Capture]::GetWindowRect($proc.MainWindowHandle, [ref]$rect) | Out-Null
$width = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top
$bmp = New-Object System.Drawing.Bitmap($width, $height)
$gfx = [System.Drawing.Graphics]::FromImage($bmp)
$gfx.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bmp.Size)
$bmp.Save($OutFile, [System.Drawing.Imaging.ImageFormat]::Png)
$gfx.Dispose(); $bmp.Dispose()
# Drop topmost again (HWND_NOTOPMOST = -2).
[Win32Capture]::SetWindowPos($proc.MainWindowHandle, [IntPtr](-2), 0, 0, 0, 0, 0x0013) | Out-Null
Write-Output "saved $OutFile ($width x $height)"
