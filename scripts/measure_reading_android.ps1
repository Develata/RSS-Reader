param(
    [Parameter(Mandatory = $true)][string]$ArtifactRoot,
    [Parameter(Mandatory = $true)][string]$Serial,
    [Parameter(Mandatory = $true)][string]$BaselineApk,
    [Parameter(Mandatory = $true)][string]$BaselineApkSha256,
    [Parameter(Mandatory = $true)][string]$CandidateApk,
    [Parameter(Mandatory = $true)][string]$CandidateApkSha256,
    [Parameter(Mandatory = $true)][string]$BaselineSha,
    [Parameter(Mandatory = $true)][string]$CandidateSha,
    [string]$RunPrefix = 'android',
    [int]$Port = 9252,
    [int]$Samples = 5,
    [int]$ReaderSamples = 5,
    [string[]]$RunOrder = @('a', 'b', 'b', 'a', 'a', 'b')
)
# Explicit test-device runner. Caller must verify both APKs have the same
# expected package/signing certificate and back up the existing fixture first.
# Only install -r is used; no uninstall, pm clear, permission changes, or data copy.
$ErrorActionPreference = 'Stop'
$rssrRoot = (Resolve-Path -LiteralPath $ArtifactRoot).Path
$rssrRepo = Split-Path -Parent $PSScriptRoot
$rssrPackage = 'io.github.develata.rssreader'
$rssrActivity = "$rssrPackage/dev.dioxus.main.MainActivity"
$rssrApks = @{a=(Resolve-Path -LiteralPath $BaselineApk).Path; b=(Resolve-Path -LiteralPath $CandidateApk).Path}
$rssrHashes = @{a=$BaselineApkSha256; b=$CandidateApkSha256}
$rssrSources = @{a=$BaselineSha; b=$CandidateSha}
foreach ($rssrVariant in @('a','b')) {
    if ((Get-FileHash -LiteralPath $rssrApks[$rssrVariant] -Algorithm SHA256).Hash -ne $rssrHashes[$rssrVariant]) {
        throw "APK identity mismatch: $rssrVariant"
    }
}
if (@($RunOrder | Where-Object { $_ -notin @('a','b') }).Count) { throw 'RunOrder accepts only a and b.' }
function Invoke-RssrAdb {
    $rssrArguments = @($args)
    $rssrResult = & adb -s $Serial @rssrArguments
    if ($LASTEXITCODE -ne 0) { throw "ADB failed: $($rssrArguments -join ' ')" }
    return $rssrResult
}
function Start-RssrMeasuredApp {
    param([string]$Folder)
    Invoke-RssrAdb shell am force-stop $rssrPackage | Out-Null
    $rssrTime = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    Invoke-RssrAdb shell am start -W -a android.intent.action.MAIN -c android.intent.category.LAUNCHER -f 0x10200000 -n $rssrActivity |
        Set-Content -LiteralPath (Join-Path $Folder 'launch.txt')
    $rssrDevicePid = (Invoke-RssrAdb shell pidof $rssrPackage).Trim()
    if ($rssrDevicePid -notmatch '^\d+$') { throw 'Expected exactly one app process.' }
    Invoke-RssrAdb forward "tcp:$Port" "localabstract:webview_devtools_remote_$rssrDevicePid" | Out-Null
    $rssrReady = $false
    $rssrDeadline = [DateTime]::UtcNow.AddSeconds(25)
    while ([DateTime]::UtcNow -lt $rssrDeadline) {
        try {
            if (@(Invoke-RestMethod "http://127.0.0.1:$Port/json/list" -TimeoutSec 1).Count -gt 0) { $rssrReady = $true; break }
        } catch { }
        Start-Sleep -Milliseconds 100
    }
    if (-not $rssrReady) { throw 'Android WebView endpoint did not become ready.' }
    return @{pid=$rssrDevicePid; launchedAt=$rssrTime}
}
$rssrInstalled = $null
$rssrRuns = @()
try {
    for ($rssrIndex=0; $rssrIndex -lt $RunOrder.Count; $rssrIndex++) {
        $rssrVariant = $RunOrder[$rssrIndex]
        $rssrRun = Join-Path $rssrRoot ('{0}-{1:d2}-{2}' -f $RunPrefix, ($rssrIndex+1), $rssrVariant)
        if (Test-Path -LiteralPath $rssrRun) { throw "Refusing an existing run directory: $rssrRun" }
        New-Item -ItemType Directory -Path $rssrRun | Out-Null
        if ($rssrInstalled -ne $rssrVariant) {
            Invoke-RssrAdb shell am force-stop $rssrPackage | Out-Null
            $rssrInstall = Invoke-RssrAdb install -r $rssrApks[$rssrVariant]
            $rssrInstall | Set-Content -LiteralPath (Join-Path $rssrRun 'install.txt')
            if ($rssrInstall -notcontains 'Success') { throw 'APK install did not report success.' }
            $rssrInstalled = $rssrVariant
            # Establish the same saved search/grouping before the cold process run.
            $rssrWarm = Join-Path $rssrRun 'prepare'
            New-Item -ItemType Directory -Path $rssrWarm | Out-Null
            $rssrPrep = Start-RssrMeasuredApp $rssrWarm
            & node (Join-Path $rssrRepo 'scripts/browser/reading_native_baseline.mjs') --cdp-base "http://127.0.0.1:$Port" `
                --artifact-dir $rssrWarm --source $rssrSources[$rssrVariant] --artifact-sha $rssrHashes[$rssrVariant] `
                --platform android --app-pid $rssrPrep.pid --serial $Serial --prepare-only true *> (Join-Path $rssrWarm 'run.log')
            if ($LASTEXITCODE -ne 0) { throw "Preparation failed; see $rssrWarm/run.log" }
        }
        Invoke-RssrAdb shell dumpsys battery | Set-Content -LiteralPath (Join-Path $rssrRun 'battery-before.txt')
        $rssrLaunch = Start-RssrMeasuredApp $rssrRun
        $rssrReceipt = @{variant=$rssrVariant;source=$rssrSources[$rssrVariant];sha256=$rssrHashes[$rssrVariant];
            pid=$rssrLaunch.pid;launchedAt=$rssrLaunch.launchedAt;serial=$Serial}
        $rssrReceipt | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $rssrRun 'identity.json')
        & node (Join-Path $rssrRepo 'scripts/browser/reading_native_baseline.mjs') --cdp-base "http://127.0.0.1:$Port" `
            --artifact-dir $rssrRun --source $rssrSources[$rssrVariant] --artifact-sha $rssrHashes[$rssrVariant] `
            --platform android --app-pid $rssrLaunch.pid --serial $Serial --samples $Samples --reader-samples $ReaderSamples --launched-at $rssrLaunch.launchedAt `
            *> (Join-Path $rssrRun 'run.log')
        if ($LASTEXITCODE -ne 0) { throw "Measurement failed; see $rssrRun/run.log" }
        Invoke-RssrAdb logcat -d --pid=$($rssrLaunch.pid) -v brief | Set-Content -LiteralPath (Join-Path $rssrRun 'process.log')
        Invoke-RssrAdb shell dumpsys battery | Set-Content -LiteralPath (Join-Path $rssrRun 'battery-after.txt')
        $rssrRuns += $rssrReceipt
        Write-Output "Completed $RunPrefix-$($rssrIndex+1)-$rssrVariant"
        Start-Sleep -Seconds 3
    }
} finally {
    if ($rssrInstalled -eq 'a') {
        Invoke-RssrAdb shell am force-stop $rssrPackage | Out-Null
        Invoke-RssrAdb install -r $rssrApks.b | Set-Content -LiteralPath (Join-Path $rssrRoot "$RunPrefix-restore-install.txt")
    }
    $rssrRuns | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $rssrRoot "$RunPrefix-runs.json")
}
