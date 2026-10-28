# Fixed, bounded fixtures; each invocation runs in a fresh process. Keep other
# builds/benchmarks idle while measuring. No user databases or network inputs.
param(
    [Parameter(Mandatory = $true)][string]$Binary,
    [Parameter(Mandatory = $true)][string]$OutputPath,
    [Parameter(Mandatory = $true)][string]$SourceRevision,
    [ValidateRange(1, 10)][int]$Repeat = 3
)
$ErrorActionPreference = 'Stop'
$probePath = (Resolve-Path -LiteralPath $Binary).Path
$cases = @(
    @('markers-template', 5000), @('markers-template', 10000), @('markers-template', 20000),
    @('markers-object', 5000), @('markers-object', 10000), @('markers-object', 20000),
    @('attributes', 10000), @('attributes', 20000), @('attributes', 40000),
    @('attribute-block', 20000), @('flat', 5000), @('math', 1000), @('corpus', 1)
)
$report = [ordered]@{
    source = $SourceRevision
    binary_sha256 = (Get-FileHash -LiteralPath $probePath -Algorithm SHA256).Hash
    probe_sha256 = (Get-FileHash -LiteralPath "$PSScriptRoot/../crates/rssr-infra/examples/reader_html_probe.rs" -Algorithm SHA256).Hash
    platform = [System.Runtime.InteropServices.RuntimeInformation]::OSDescription
    records = @()
}
$outputFullPath = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputPath)
New-Item -ItemType Directory -Path (Split-Path $outputFullPath -Parent) -Force | Out-Null
foreach ($case in $cases) {
    foreach ($attempt in 1..$Repeat) {
        $record = & $probePath $case[0] $case[1] | ConvertFrom-Json
        if ($LASTEXITCODE -ne 0) { throw "Probe failed: $($case[0]) $($case[1])" }
        $record | Add-Member -NotePropertyName repeat -NotePropertyValue $attempt
        $report.records += $record
        $report | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $outputFullPath
        $record | Select-Object shape, scale, elapsed_ms, peak_working_set_bytes
    }
}
