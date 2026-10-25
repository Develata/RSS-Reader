param(
    [Parameter(Mandatory = $true)][int]$AppProcessId,
    [Parameter(Mandatory = $true)][string]$ExpectedExecutable
)
$ErrorActionPreference = 'Stop'
$rssrExpected = (Resolve-Path -LiteralPath $ExpectedExecutable).Path
$rssrProcesses = @(Get-CimInstance Win32_Process)
$rssrRoot = $rssrProcesses | Where-Object ProcessId -eq $AppProcessId
if (-not $rssrRoot -or $rssrRoot.ExecutablePath -ne $rssrExpected) {
    throw 'The requested PID does not match the isolated test executable.'
}
$rssrIds = [System.Collections.Generic.HashSet[uint32]]::new()
[void]$rssrIds.Add([uint32]$AppProcessId)
$rssrById = @{}
foreach ($rssrProcess in $rssrProcesses) { $rssrById[[uint32]$rssrProcess.ProcessId] = $rssrProcess }
do {
    $rssrAdded = $false
    foreach ($rssrProcess in $rssrProcesses) {
        # A PID can be reused after an older parent exits. A surviving process
        # older than this parent instance is not part of the new app's tree.
        if ($rssrIds.Contains([uint32]$rssrProcess.ParentProcessId) -and
            $rssrProcess.CreationDate -ge $rssrById[[uint32]$rssrProcess.ParentProcessId].CreationDate) {
            $rssrAdded = $rssrIds.Add([uint32]$rssrProcess.ProcessId) -or $rssrAdded
        }
    }
} while ($rssrAdded)
$rssrCounters = @(Get-CimInstance Win32_PerfRawData_PerfProc_Process | Where-Object { $rssrIds.Contains([uint32]$_.IDProcess) })
$rssrRows = @($rssrProcesses | Where-Object { $rssrIds.Contains([uint32]$_.ProcessId) } | ForEach-Object {
    $rssrProcess = $_
    $rssrCounter = $rssrCounters | Where-Object IDProcess -eq $rssrProcess.ProcessId | Select-Object -First 1
    [PSCustomObject]@{
        pid = $rssrProcess.ProcessId
        parentPid = $rssrProcess.ParentProcessId
        name = $rssrProcess.Name
        createdAt = $rssrProcess.CreationDate.ToUniversalTime().ToString('o')
        counterAvailable = $null -ne $rssrCounter
        privateBytes = if ($rssrCounter) { [long]$rssrCounter.PrivateBytes } else { $null }
        privateWorkingSetBytes = if ($rssrCounter) { [long]$rssrCounter.WorkingSetPrivate } else { $null }
        workingSetBytes = if ($rssrCounter) { [long]$rssrCounter.WorkingSet } else { $null }
    }
})
[PSCustomObject]@{
    sampledAt = [DateTime]::UtcNow.ToString('o')
    rootPid = $AppProcessId
    expectedExecutable = $rssrExpected
    complete = @($rssrRows | Where-Object { -not $_.counterAvailable }).Count -eq 0
    rows = $rssrRows
    privateBytes = ($rssrRows | Measure-Object privateBytes -Sum).Sum
    privateWorkingSetBytes = ($rssrRows | Measure-Object privateWorkingSetBytes -Sum).Sum
    workingSetSumBytes = ($rssrRows | Measure-Object workingSetBytes -Sum).Sum
    note = 'Process tree snapshot. Private bytes are committed memory; private working set is resident private memory. Summed working sets double-count shared pages. Not a peak or leak measurement.'
} | ConvertTo-Json -Depth 5
