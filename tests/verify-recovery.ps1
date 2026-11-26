param([Parameter(Mandatory=$true)][string]$SourceDirectory)
$ErrorActionPreference = 'Stop'
$tool = Join-Path $PSScriptRoot '../src/recovery.ps1'
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')
$lab = Join-Path $tempRoot ('kagari-recovery-' + [guid]::NewGuid().ToString('N'))
[void][IO.Directory]::CreateDirectory($lab)
try {
    $source = Join-Path $lab 'workspace.tsv'
    [IO.File]::Copy((Join-Path $SourceDirectory 'workspace.tsv'), $source)
    $records = @('workspace.tsv')
    $history = Join-Path $SourceDirectory 'kpi-history'
    if (Test-Path -LiteralPath $history) {
        [void][IO.Directory]::CreateDirectory((Join-Path $lab 'kpi-history'))
        foreach ($file in [IO.Directory]::GetFiles($history, '*.json')) {
            $relative = 'kpi-history/' + [IO.Path]::GetFileName($file)
            [IO.File]::Copy($file, (Join-Path $lab $relative))
            $records += $relative
        }
    }
    $point = & $tool -Mode Backup -Source $source
    $target = & $tool -Mode Restore -Source $point -Destination (Join-Path $lab 'restored')
    $memos = 0
    foreach ($record in $records) {
        $restored = Join-Path $target $record
        if ((Get-FileHash -LiteralPath (Join-Path $lab $record)).Hash -ne (Get-FileHash -LiteralPath $restored).Hash) {
            throw "Restored content differs: $record"
        }
        if ($record.EndsWith('.json')) {
            $observation = [IO.File]::ReadAllText($restored) | ConvertFrom-Json
            if ($null -eq $observation.Received -or $null -eq $observation.All -or
                $null -eq $observation.Projects -or $null -eq $observation.Agents) { throw "Invalid KPI: $record" }
            if ($observation.Memo) { $memos++ }
        }
    }
    $rows = [IO.File]::ReadAllLines((Join-Path $target 'workspace.tsv'))
    $tasks = @($rows | Where-Object { $_.StartsWith("T`t") })
    [pscustomobject]@{
        SourceModified = [IO.File]::GetLastWriteTime($source).ToString('o')
        Tasks = $tasks.Count
        StatusCounts = @($tasks | Group-Object { ($_ -split "`t")[4] } | Select-Object Name, Count)
        ParentLinks = @($tasks | Where-Object { ($_ -split "`t")[21] }).Count + @($rows | Where-Object { $_.StartsWith("TP`t") }).Count
        PrimarySessions = @($tasks | Where-Object { ($_ -split "`t")[14] }).Count
        AdditionalSessions = @($rows | Where-Object { $_.StartsWith("S`t") }).Count
        Observations = $records.Count - 1
        NonemptyMemos = $memos
    } | ConvertTo-Json -Depth 3
} finally {
    if ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($lab)) -eq $tempRoot) {
        [IO.Directory]::Delete($lab, $true)
    }
}
