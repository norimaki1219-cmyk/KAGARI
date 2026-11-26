[CmdletBinding()]
param(
    [Parameter(Mandatory, Position=0)][string]$Executable,
    [string[]]$ArgumentList = @()
)
$ErrorActionPreference = 'Stop'
$path = (Resolve-Path -LiteralPath $Executable).ProviderPath
if ([IO.Path]::GetExtension($path) -ne '.exe') { throw '検証用の exe を指定してください' }
if (-not ('KagariVerification.ErrorMode' -as [type])) {
    Add-Type @"
using System.Runtime.InteropServices;
namespace KagariVerification {
    public static class ErrorMode {
        [DllImport("kernel32.dll")] public static extern uint GetErrorMode();
        [DllImport("kernel32.dll")] public static extern uint SetErrorMode(uint mode);
    }
}
"@
}
$previous = [KagariVerification.ErrorMode]::GetErrorMode()
Push-Location -LiteralPath ([IO.Path]::GetDirectoryName($path))
try {
    # The child inherits SEM_NOGPFAULTERRORBOX. Keep its original output and exit code.
    [void][KagariVerification.ErrorMode]::SetErrorMode($previous -bor 2)
    & $path @ArgumentList | Out-Host
    $result = $LASTEXITCODE
} finally {
    [void][KagariVerification.ErrorMode]::SetErrorMode($previous)
    Pop-Location
}
exit $result
