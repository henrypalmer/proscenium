# Live experiment: make the WRY_WEBVIEW child layered (alpha 255) so it
# stops clipping the video host beneath it, then verify with a capture.
Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public class LayerExp {
    [DllImport("user32.dll")] public static extern IntPtr GetTopWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr h, uint cmd);
    [DllImport("user32.dll")] public static extern int GetClassName(IntPtr h, StringBuilder sb, int max);
    [DllImport("user32.dll")] public static extern IntPtr GetWindowLongPtr(IntPtr h, int idx);
    [DllImport("user32.dll")] public static extern IntPtr SetWindowLongPtr(IntPtr h, int idx, IntPtr val);
    [DllImport("user32.dll")] public static extern bool SetLayeredWindowAttributes(IntPtr h, uint key, byte alpha, uint flags);
}
"@

$main = (Get-Process proscenium | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1).MainWindowHandle
$child = [LayerExp]::GetTopWindow($main)
$webview = [IntPtr]::Zero
while ($child -ne [IntPtr]::Zero) {
    $sb = New-Object System.Text.StringBuilder 256
    [LayerExp]::GetClassName($child, $sb, 256) | Out-Null
    if ($sb.ToString() -eq "WRY_WEBVIEW") { $webview = $child }
    $child = [LayerExp]::GetWindow($child, 2)
}
if ($webview -eq [IntPtr]::Zero) { throw "WRY_WEBVIEW not found" }

# GWL_EXSTYLE = -20, WS_EX_LAYERED = 0x80000, LWA_ALPHA = 2
$ex = [LayerExp]::GetWindowLongPtr($webview, -20)
[LayerExp]::SetWindowLongPtr($webview, -20, [IntPtr]([long]$ex -bor 0x80000)) | Out-Null
[LayerExp]::SetLayeredWindowAttributes($webview, 0, 255, 2) | Out-Null
Write-Output "webview $webview made layered"
