param(
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-fA-F]{32}$')][string]$TaskId,
    [Parameter(Mandatory)][ValidateSet('todo','working','review','done')][string]$Status,
    [ValidateRange(0,300)][int]$WaitSeconds = 15
)
$ErrorActionPreference = 'Stop'
$root = Join-Path $env:LOCALAPPDATA 'RustAIWorkspace'
$state = Join-Path $root 'workspace.tsv'
$id = $TaskId.ToLowerInvariant()
$states = @('todo','working','review','done')
$target = [array]::IndexOf($states,$Status.ToLowerInvariant())
function Get-TaskRow {
    $matchesById = @(foreach ($line in [IO.File]::ReadAllLines($state)) {
        if ($line.StartsWith("T`t")) {
            $fields = $line.Split([char]9)
            if ($fields.Count -ge 21 -and $fields[20] -eq $id) { ,$fields }
        }
    })
    if ($matchesById.Count -ne 1) { throw "Expected one saved task for $id; found $($matchesById.Count)" }
    return ,$matchesById[0]
}
$row = Get-TaskRow
$old = [int]$row[4]
$key = [Guid]::NewGuid().ToString('N')
$inbox = Join-Path $root 'queue-inbox'
$null = [IO.Directory]::CreateDirectory($inbox)
$prefix = Join-Path $inbox $key
[IO.File]::WriteAllText("$prefix.tmp", "$key`t$id`t$old`t$target", [Text.UTF8Encoding]::new($false))
[IO.File]::Move("$prefix.tmp","$prefix.status")
$timer = [Diagnostics.Stopwatch]::StartNew()
$mutex = [Threading.Mutex]::new($false,'Local\RustAIWorkspace_v1')
try {
    do {
        if ([IO.File]::Exists("$prefix.receipt")) { break }
        try { $owner = $mutex.WaitOne(0) } catch [Threading.AbandonedMutexException] { $owner = $true }
        if ($owner) {
            try {
                if (-not ('KagariBackgroundStatus' -as [type])) {
                    $dll = (Join-Path $PSScriptRoot 'kagari_core.dll').Replace('"','""')
                    Add-Type "using System.Runtime.InteropServices; public static class KagariBackgroundStatus { [DllImport(@`"$dll`")] public static extern int kagari_apply_pending_offline(); }"
                }
                if ([KagariBackgroundStatus]::kagari_apply_pending_offline() -ne 0) { throw "Could not apply pending updates. Request $key retained for retry." }
            } finally { $mutex.ReleaseMutex() }
        }
        if (![IO.File]::Exists("$prefix.receipt") -and $timer.Elapsed.TotalSeconds -lt $WaitSeconds) { Start-Sleep -Milliseconds 200 }
    } while ($timer.Elapsed.TotalSeconds -lt $WaitSeconds)
} finally { $mutex.Dispose() }
if (![IO.File]::Exists("$prefix.receipt")) {
    [pscustomobject]@{Result='queued';TaskId=$id;Request=$key;Receipt="$prefix.receipt"}
    return
}
$receipt = [IO.File]::ReadAllText("$prefix.receipt")
if ($receipt -ne 'saved') {throw "$key $receipt"}
$row = Get-TaskRow
if ([int]$row[4] -ne $target) {throw "Request $key was applied but a later change is present"}
[pscustomobject]@{Result='saved';TaskId=$id;Status=$states[$target];Request=$key}
