param([string]$Action)
$ErrorActionPreference='Stop'
[Console]::InputEncoding=[Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$env:PYTHONUTF8='1'
$OutputEncoding=[Console]::OutputEncoding
[Console]::In.ReadToEnd() | & python.exe (Join-Path $PSScriptRoot jobs_public.py)
exit $LASTEXITCODE
