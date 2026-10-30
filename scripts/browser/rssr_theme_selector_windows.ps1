# UI Automation snapshot of one test host and its descendants. Optional startup visibility
# only shows that host without activation; this helper never sends global keyboard input.
param([Parameter(Mandatory)][int]$RootProcessId,[Parameter(Mandatory)][string]$OutputPrefix,[switch]$ShowWithoutActivating)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient,UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class ThemePopupWindowProbe {
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr window, int command);
}
'@
$owned = [Collections.Generic.HashSet[int]]::new()
[void]$owned.Add($RootProcessId)
$processes = @(Get-CimInstance Win32_Process)
do {
    $added = $false
    foreach ($item in $processes) {
        if ($owned.Contains([int]$item.ParentProcessId) -and $owned.Add([int]$item.ProcessId)) { $added = $true }
    }
} while ($added)
$windows = [System.Windows.Automation.AutomationElement]::RootElement.FindAll(
    [System.Windows.Automation.TreeScope]::Children,[System.Windows.Automation.Condition]::TrueCondition)
if ($ShowWithoutActivating) {
    foreach ($window in $windows) {
        if ($window.Current.ProcessId -eq $RootProcessId -and $window.Current.Name -eq 'RSS-Reader') {
            [void][ThemePopupWindowProbe]::ShowWindow([IntPtr]$window.Current.NativeWindowHandle,4)
            Start-Sleep -Milliseconds 200
        }
    }
}
$rows = @()
foreach ($window in $windows) {
    if (-not $owned.Contains($window.Current.ProcessId)) { continue }
    $nodes = $window.FindAll([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.Condition]::TrueCondition)
    $controls = @()
    foreach ($node in $nodes) {
        $current = $node.Current
        if ($current.ControlType.ProgrammaticName -in @('ControlType.ComboBox','ControlType.List','ControlType.ListItem','ControlType.Menu','ControlType.MenuItem')) {
            $controls += [ordered]@{name=$current.Name;type=$current.ControlType.ProgrammaticName;framework=$current.FrameworkId;class=$current.ClassName;nativeHandle=$current.NativeWindowHandle;offscreen=$current.IsOffscreen;enabled=$current.IsEnabled;focused=$current.HasKeyboardFocus;bounds=$current.BoundingRectangle.ToString()}
        }
    }
    $rows += [ordered]@{name=$window.Current.Name;class=$window.Current.ClassName;process=$window.Current.ProcessId;handle=$window.Current.NativeWindowHandle;bounds=$window.Current.BoundingRectangle.ToString();controls=$controls}
}
$foregroundPid = [uint32]0
[void][ThemePopupWindowProbe]::GetWindowThreadProcessId([ThemePopupWindowProbe]::GetForegroundWindow(),[ref]$foregroundPid)
$result = [ordered]@{rootProcess=$RootProcessId;ownedProcesses=@($owned);foregroundOwned=$owned.Contains([int]$foregroundPid);windows=$rows}
[IO.File]::WriteAllText($OutputPrefix+'.json',($result|ConvertTo-Json -Depth 8),[Text.UTF8Encoding]::new($false))
