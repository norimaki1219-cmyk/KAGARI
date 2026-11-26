param(
    [ValidateSet('Backup','Restore')][string]$Mode = 'Backup',
    [string]$Source = $env:KAGARI_RECOVERY_SOURCE,
    [string]$Destination
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)

function Copy-Record($inputPath, $outputPath) {
    [IO.File]::Copy($inputPath, $outputPath)
    $expected = (Get-FileHash -LiteralPath $inputPath -Algorithm SHA256).Hash
    if ((Get-FileHash -LiteralPath $outputPath -Algorithm SHA256).Hash -ne $expected) {
        throw "Copy verification failed: $inputPath"
    }
    return $expected
}

if ($Mode -eq 'Backup') {
    $workspace = [IO.Path]::GetFullPath($Source)
    $root = [IO.Path]::GetDirectoryName($workspace)
    $points = Join-Path $root 'restore-points'
    [void][IO.Directory]::CreateDirectory($points)
    $Destination = Join-Path $points ((Get-Date -Format 'yyyyMMdd-HHmmss') + '-' + [guid]::NewGuid().ToString('N'))
} else {
    $root = [IO.Path]::GetFullPath($Source)
    $workspace = Join-Path $root 'workspace.tsv'
    if (!$Destination) { throw 'Specify a new restore directory.' }
}
$Destination = [IO.Path]::GetFullPath($Destination)
if (Test-Path -LiteralPath $Destination) { throw "Destination already exists: $Destination" }
$stage = $Destination + '.' + [guid]::NewGuid().ToString('N') + '.pending'
[void][IO.Directory]::CreateDirectory($stage)
try {
    $hashes = @{}
    $hashes['workspace.tsv'] = Copy-Record $workspace (Join-Path $stage 'workspace.tsv')
    $history = Join-Path $root 'kpi-history'
    if (Test-Path -LiteralPath $history) {
        $target = [IO.Directory]::CreateDirectory((Join-Path $stage 'kpi-history'))
        foreach ($file in [IO.Directory]::GetFiles($history, '*.json')) {
            $name = [IO.Path]::GetFileName($file)
            $hashes["kpi-history/$name"] = Copy-Record $file (Join-Path $target.FullName $name)
        }
    }
    if ($Mode -eq 'Backup') {
        $manifest = @{version=1; files=$hashes} | ConvertTo-Json
        [IO.File]::WriteAllText((Join-Path $stage 'manifest.json'), $manifest)
    } else {
        $manifest = [IO.File]::ReadAllText((Join-Path $root 'manifest.json')) | ConvertFrom-Json
        if ($manifest.version -ne 1 -or !$manifest.files) { throw 'Invalid manifest.json' }
        foreach ($record in $manifest.files.PSObject.Properties) {
            if ($hashes[$record.Name] -ne $record.Value -or !$hashes.ContainsKey($record.Name)) {
                throw "Missing or damaged record: $($record.Name)"
            }
            $hashes.Remove($record.Name)
        }
        if ($hashes.Count) { throw "Unlisted records: $($hashes.Keys -join ', ')" }
    }
    [IO.Directory]::Move($stage, $Destination)
    $Destination
} finally {
    if ([IO.Directory]::Exists($stage) -and [IO.Path]::GetDirectoryName($stage) -eq [IO.Path]::GetDirectoryName($Destination)) {
        [IO.Directory]::Delete($stage, $true)
    }
}
