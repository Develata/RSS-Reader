param(
    [Parameter(Mandatory = $true)][string]$ArtifactRoot,
    [Parameter(Mandatory = $true)][string]$BaselineSha,
    [Parameter(Mandatory = $true)][string]$CandidateSha,
    [Parameter(Mandatory = $true)][string]$BaselineExeSha256,
    [Parameter(Mandatory = $true)][string]$CandidateExeSha256,
    [string]$RunPrefix = 'windows',
    [int]$Port = 9251,
    [int]$Samples = 5,
    [string[]]$RunOrder = @('a', 'b', 'b', 'a', 'a', 'b')
)
$ErrorActionPreference = 'Stop'
$rssrRoot = (Resolve-Path -LiteralPath $ArtifactRoot).Path
$rssrRepo = Split-Path -Parent $PSScriptRoot
$rssrOldArgs = $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
$rssrOldProfile = $env:WEBVIEW2_USER_DATA_FOLDER
$rssrRuns = @()
$rssrHashes = @{a=$BaselineExeSha256; b=$CandidateExeSha256}
foreach ($rssrVariant in @('a','b')) {
    $rssrInput = Join-Path $rssrRoot "$rssrVariant/rssr-app.exe"
    if ((Get-FileHash -LiteralPath $rssrInput -Algorithm SHA256).Hash -ne $rssrHashes[$rssrVariant]) {
        throw "Executable identity mismatch: $rssrVariant"
    }
}
# Three launches per artifact, with reversed pairs to limit order bias.
$rssrOrder = $RunOrder
if (@($rssrOrder | Where-Object { $_ -notin @('a','b') }).Count) { throw 'RunOrder accepts only a and b.' }
try {
    for ($rssrIndex = 0; $rssrIndex -lt $rssrOrder.Count; $rssrIndex++) {
        $rssrVariant = $rssrOrder[$rssrIndex]
        $rssrRun = Join-Path $rssrRoot ('{0}-{1:d2}-{2}' -f $RunPrefix, ($rssrIndex + 1), $rssrVariant)
        if (Test-Path -LiteralPath $rssrRun) { throw "Refusing an existing run directory: $rssrRun" }
        if (Get-Process cargo,rustc -ErrorAction SilentlyContinue) { throw 'Wait for builds to finish before measuring.' }
        New-Item -ItemType Directory -Path (Join-Path $rssrRun 'RSS-Reader') | Out-Null
        $rssrExe = Join-Path $rssrRun 'rssr-app.exe'
        Copy-Item -LiteralPath (Join-Path $rssrRoot "$rssrVariant/rssr-app.exe") -Destination $rssrExe
        foreach ($rssrFile in @('rss-reader.db', 'rss-reader-content.db')) {
            Copy-Item -LiteralPath (Join-Path $rssrRoot "seed-matched/$rssrFile") -Destination (Join-Path $rssrRun 'RSS-Reader')
        }
        $rssrSha = if ($rssrVariant -eq 'a') { $BaselineSha } else { $CandidateSha }
        $rssrHash = (Get-FileHash -LiteralPath $rssrExe -Algorithm SHA256).Hash
        if ($rssrHash -ne $rssrHashes[$rssrVariant]) { throw "Copied executable identity mismatch: $rssrVariant" }
        $rssrCpu = (Get-CimInstance Win32_PerfFormattedData_PerfOS_Processor -Filter "Name='_Total'").PercentProcessorTime
        $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
        $env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $rssrRun 'webview'
        $rssrLaunchedAt = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
        $rssrApp = Start-Process -FilePath $rssrExe -WorkingDirectory $rssrRun -WindowStyle Hidden -PassThru
        $rssrReceipt = [ordered]@{variant=$rssrVariant; source=$rssrSha; sha256=$rssrHash; pid=$rssrApp.Id;
            executable=$rssrExe; launchedAt=$rssrLaunchedAt; systemCpuBeforePercent=$rssrCpu}
        $rssrReceipt | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $rssrRun 'identity.json')
        try {
            $rssrReady = $false
            $rssrDeadline = [DateTime]::UtcNow.AddSeconds(25)
            while ([DateTime]::UtcNow -lt $rssrDeadline) {
                try {
                    $rssrTargets = Invoke-RestMethod "http://127.0.0.1:$Port/json/list" -TimeoutSec 1
                    if (@($rssrTargets | Where-Object type -eq 'page').Count -gt 0) { $rssrReady = $true; break }
                } catch { }
                Start-Sleep -Milliseconds 50
            }
            if (-not $rssrReady) { throw 'Native debug endpoint did not become ready.' }
            & node (Join-Path $rssrRepo 'scripts/browser/reading_native_baseline.mjs') --cdp-base "http://127.0.0.1:$Port" `
                --artifact-dir $rssrRun --source $rssrSha --artifact-sha $rssrHash --platform windows `
                --app-pid $rssrApp.Id --exe $rssrExe --samples $Samples --launched-at $rssrLaunchedAt `
                *> (Join-Path $rssrRun 'run.log')
            if ($LASTEXITCODE -ne 0) { throw "Measurement failed; see $rssrRun/run.log" }
            $rssrRuns += $rssrReceipt
            Write-Output "Completed $RunPrefix-$($rssrIndex + 1)-$rssrVariant"
        } finally {
            $rssrLive = Get-CimInstance Win32_Process -Filter "ProcessId=$($rssrApp.Id)"
            if ($rssrLive -and $rssrLive.ExecutablePath -eq $rssrExe) {
                [void]$rssrApp.CloseMainWindow()
                if (-not $rssrApp.WaitForExit(3000)) { Stop-Process -Id $rssrApp.Id }
            }
        }
        Start-Sleep -Seconds 3
    }
} finally {
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $rssrOldArgs
    $env:WEBVIEW2_USER_DATA_FOLDER = $rssrOldProfile
    $rssrRuns | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $rssrRoot "$RunPrefix-runs.json")
}
