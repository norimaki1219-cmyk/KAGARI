$ErrorActionPreference='Stop'
Push-Location $PSScriptRoot
try {
 cargo build --release --offline
 if($LASTEXITCODE){throw 'Rustビルド失敗'}
 $net=Join-Path $env:SystemRoot 'Microsoft.NET\Framework64\v4.0.30319'
 $res=Get-ChildItem target\release\build\ai-workspace-*\out\app.res | Sort-Object LastWriteTime -Descending | Select-Object -First 1
 $refs=@('WPF\PresentationCore.dll','WPF\PresentationFramework.dll','WPF\WindowsBase.dll','System.Xaml.dll','System.Xml.Linq.dll') | ForEach-Object {'/r:'+(Join-Path $net $_)}
 & "$net\csc.exe" /nologo /target:winexe /platform:x64 /optimize+ /codepage:65001 "/win32res:$($res.FullName)" /out:KAGARI.exe @refs ui\Desktop.cs
 if($LASTEXITCODE){throw 'UIビルド失敗'}
 Copy-Item target\release\kagari_core.dll kagari_core.dll -Force
} finally {Pop-Location}
