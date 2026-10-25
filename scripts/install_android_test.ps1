#requires -Version 7.0
<#
.SYNOPSIS
Install a verified RSS-Reader test APK and confirm its observed Xiaomi USB prompt.
.DESCRIPTION
Uses the existing ADB authorization. Does not change device security settings,
grant app permissions, unlock the phone, clear data, or accept other dialogs.
#>
param(
    [Parameter(Mandatory)][string]$DeviceSerial,
    [Parameter(Mandatory)][string]$Apk,
    [Parameter(Mandatory)][ValidatePattern('^[A-Fa-f0-9]{64}$')][string]$ExpectedSha256,
    [ValidateRange(15, 300)][int]$TimeoutSeconds = 90,
    [string]$ArtifactDirectory
)
$ErrorActionPreference = 'Stop'
function Get-RssrXiaomiInstallTap([string]$Focus, [string]$Dump) {
    if (-not $Focus.Contains('com.miui.permcenter.install.AdbInstallActivity')) { return $null }
    $xmlMatch = [regex]::Match($Dump, '(?s)<hierarchy.*</hierarchy>')
    if (-not $xmlMatch.Success) { return $null }
    $tree = [xml]::new()
    $tree.XmlResolver = $null
    try { $tree.LoadXml($xmlMatch.Value) } catch { return $null }
    $nodes = @($tree.SelectNodes('//node[@package="com.miui.securitycenter"]'))
    $continue = @($nodes | Where-Object { $_.text -eq '继续安装' -and $_.enabled -eq 'true' })
    if (($nodes.text -notcontains 'USB安装提示') -or
        ($nodes.text -notcontains 'RSS-Reader') -or
        ($nodes.text -notcontains '正在通过USB安装此应用，是否继续？') -or
        $continue.Count -ne 1) { return $null }
    $bounds = [regex]::Match($continue[0].bounds, '^\[(\d+),(\d+)\]\[(\d+),(\d+)\]$')
    if (-not $bounds.Success) { return $null }
    $left, $top, $right, $bottom = 1..4 | ForEach-Object { [int]$bounds.Groups[$_].Value }
    if ($right -le $left -or $bottom -le $top) { return $null }
    [pscustomobject]@{ X = [int](($left + $right) / 2); Y = [int](($top + $bottom) / 2); Xml = $xmlMatch.Value }
}
$rssrPackage = 'io.github.develata.rssreader'
$rssrApkPath = (Resolve-Path -LiteralPath $Apk).Path
$rssrHash = (Get-FileHash -LiteralPath $rssrApkPath -Algorithm SHA256).Hash
if ($rssrHash -ne $ExpectedSha256) { throw 'APK hash does not match the expected test build.' }
$rssrAdb = (Get-Command adb -ErrorAction Stop).Source
$rssrSdk = if ($env:ANDROID_HOME) { $env:ANDROID_HOME } else { $env:ANDROID_SDK_ROOT }
if (-not $rssrSdk) { throw 'ANDROID_HOME or ANDROID_SDK_ROOT is required to inspect the APK.' }
$rssrAapt = Get-ChildItem -LiteralPath (Join-Path $rssrSdk 'build-tools') -Directory |
    Sort-Object Name -Descending |
    ForEach-Object { Join-Path $_.FullName 'aapt.exe' } |
    Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
if (-not $rssrAapt) { throw 'Android build-tools aapt.exe was not found.' }
$rssrBadging = & $rssrAapt dump badging $rssrApkPath
if ($LASTEXITCODE -ne 0 -or
    -not ($rssrBadging -match "^package: name='$([regex]::Escape($rssrPackage))' ") -or
    -not ($rssrBadging -match "^application-label:'RSS-Reader'$")) {
    throw 'This helper only accepts an APK identified as RSS-Reader with the expected package.'
}
$rssrDeviceState = & $rssrAdb -s $DeviceSerial get-state
if ($LASTEXITCODE -ne 0 -or $rssrDeviceState -ne 'device') {
    throw 'The selected device is not connected and authorized. Unlock and authorize it first.'
}
if (-not $ArtifactDirectory) {
    $ArtifactDirectory = Join-Path (Split-Path $PSScriptRoot) (
        'target/android-install-' + [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmssfff'))
}
$rssrArtifacts = [IO.Path]::GetFullPath($ArtifactDirectory)
[void][IO.Directory]::CreateDirectory($rssrArtifacts)
$rssrStart = [Diagnostics.ProcessStartInfo]::new()
$rssrStart.FileName = $rssrAdb
$rssrStart.UseShellExecute = $false
$rssrStart.CreateNoWindow = $true
$rssrStart.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
$rssrStart.RedirectStandardOutput = $true
$rssrStart.RedirectStandardError = $true
foreach ($rssrArgument in @('-s', $DeviceSerial, 'install', '-r', $rssrApkPath)) {
    $rssrStart.ArgumentList.Add($rssrArgument)
}
$rssrProcess = [Diagnostics.Process]::Start($rssrStart)
$rssrOutput = $rssrProcess.StandardOutput.ReadToEndAsync()
$rssrError = $rssrProcess.StandardError.ReadToEndAsync()
$rssrTimer = [Diagnostics.Stopwatch]::StartNew()
$rssrConfirmed = $false
try {
    while (-not $rssrProcess.HasExited) {
        if ($rssrTimer.Elapsed.TotalSeconds -ge $TimeoutSeconds) {
            throw 'Installation timed out. The phone may be locked or showing an unsupported prompt.'
        }
        $rssrFocus = (& $rssrAdb -s $DeviceSerial shell dumpsys window |
            Select-String 'mCurrentFocus=') -join "`n"
        if (-not $rssrConfirmed -and $rssrFocus.Contains('com.miui.permcenter.install.AdbInstallActivity')) {
            $rssrDump = (& $rssrAdb -s $DeviceSerial exec-out uiautomator dump /dev/tty) -join "`n"
            $rssrTap = Get-RssrXiaomiInstallTap $rssrFocus $rssrDump
            if ($rssrTap) {
                $rssrCurrentFocus = (& $rssrAdb -s $DeviceSerial shell dumpsys window |
                    Select-String 'mCurrentFocus=') -join "`n"
                if (-not $rssrProcess.HasExited -and $rssrCurrentFocus.Contains('com.miui.permcenter.install.AdbInstallActivity')) {
                    $rssrTap.Xml | Set-Content (Join-Path $rssrArtifacts 'confirmed-prompt.xml') -Encoding utf8
                    & $rssrAdb -s $DeviceSerial shell input tap $rssrTap.X $rssrTap.Y
                    if ($LASTEXITCODE -ne 0) { throw 'ADB input was refused by the device.' }
                    $rssrConfirmed = $true
                }
            }
        }
        Start-Sleep -Milliseconds 300
    }
    $rssrStdout = $rssrOutput.GetAwaiter().GetResult()
    $rssrStderr = $rssrError.GetAwaiter().GetResult()
    $rssrStdout | Set-Content (Join-Path $rssrArtifacts 'install.stdout.txt') -Encoding utf8
    $rssrStderr | Set-Content (Join-Path $rssrArtifacts 'install.stderr.txt') -Encoding utf8
    if ($rssrProcess.ExitCode -ne 0 -or $rssrStdout -notmatch '(?m)^Success\s*$') {
        throw "Installation failed: $rssrStderr $rssrStdout"
    }
    $rssrInstalled = @(& $rssrAdb -s $DeviceSerial shell pm path $rssrPackage)
    if ($LASTEXITCODE -ne 0 -or $rssrInstalled.Count -ne 1 -or
        $rssrInstalled[0] -notmatch '^package:(/data/app/[A-Za-z0-9_+~=/.-]+/base\.apk)$') {
        throw 'Unable to verify the installed APK path.'
    }
    $rssrInstalledPath = $Matches[1]
    $rssrInstalledHash = ((& $rssrAdb -s $DeviceSerial shell sha256sum $rssrInstalledPath) -split '\s+')[0]
    if ($LASTEXITCODE -ne 0 -or $rssrInstalledHash -ne $ExpectedSha256) {
        throw 'The installed APK does not match the verified test artifact.'
    }
    $rssrReceipt = [pscustomobject]@{
        installedAtUtc = [DateTime]::UtcNow.ToString('o')
        serial = $DeviceSerial
        package = $rssrPackage
        apk = $rssrApkPath
        sha256 = $rssrHash
        installedSha256 = $rssrInstalledHash
        confirmedXiaomiPrompt = $rssrConfirmed
        replaceWithoutClearingData = $true
    }
    $rssrReceipt | ConvertTo-Json | Set-Content (Join-Path $rssrArtifacts 'receipt.json') -Encoding utf8
    $rssrReceipt
}
finally {
    if (-not $rssrProcess.HasExited) { $rssrProcess.Kill(); $rssrProcess.WaitForExit() }
    $rssrProcess.Dispose()
}
