param([string]$OutputDirectory=$PSScriptRoot)
$ErrorActionPreference='Stop'
Push-Location $PSScriptRoot
try {
 cargo build --release --offline
 if($LASTEXITCODE){throw 'Rustビルド失敗'}
 $net=Join-Path $env:SystemRoot 'Microsoft.NET\Framework64\v4.0.30319'
 $res=Get-ChildItem target\release\build\ai-workspace-*\out\app.res | Sort-Object LastWriteTime -Descending | Select-Object -First 1
 $refs=@('WPF\PresentationCore.dll','WPF\PresentationFramework.dll','WPF\WindowsBase.dll','System.Runtime.Serialization.dll','System.Security.dll','System.Xaml.dll','System.Xml.Linq.dll','System.Net.Http.dll','System.Web.Extensions.dll','System.ServiceProcess.dll') | ForEach-Object {'/r:'+(Join-Path $net $_)}
 $version='1.0.2903.40'
 $sdk=Join-Path $PSScriptRoot "target\webview2-$version"
 $null=New-Item -ItemType Directory -Path $sdk -Force
 $zip=Join-Path $sdk 'sdk.zip'
 if(!(Test-Path $zip)){Invoke-WebRequest "https://api.nuget.org/v3-flatcontainer/microsoft.web.webview2/$version/microsoft.web.webview2.$version.nupkg" -UseBasicParsing -OutFile $zip}
 if((Get-FileHash $zip).Hash -ne 'EF128016DD1E51C59178C827ED5B8AA3322C57AFA8675D930F8109505542AD74'){throw 'WebView2 SDK hash mismatch'}
 if(!(Test-Path "$sdk\lib\net462\Microsoft.Web.WebView2.Wpf.dll")){Expand-Archive $zip $sdk -Force}
 foreach($lib in 'Microsoft.Web.WebView2.Core.dll','Microsoft.Web.WebView2.Wpf.dll'){
   $refs+="/r:$sdk\lib\net462\$lib"
   Copy-Item "$sdk\lib\net462\$lib" $OutputDirectory -Force
 }
 Copy-Item "$sdk\runtimes\win-x64\native\WebView2Loader.dll" $OutputDirectory -Force
 # Embed the loose-XAML resource dictionary so the executable also works outside the source tree.
 $main=[IO.File]::ReadAllText((Join-Path $PSScriptRoot 'ui/Main.xaml'))
 $dictionary=[IO.File]::ReadAllText((Join-Path $PSScriptRoot 'csi/kagari_jobs_ui/wpf/JobsResources.xaml'))
 $main=$main.Replace('<ResourceDictionary Source="csi/kagari_jobs_ui/wpf/JobsResources.xaml"/>',$dictionary)
 [IO.File]::WriteAllText((Join-Path $PSScriptRoot 'target/JobsMain.xaml'),$main)
 & "$net\csc.exe" /nologo /target:winexe /resource:target\JobsMain.xaml,KAGARI.Main.xaml /platform:x64 /optimize+ /codepage:65001 "/win32res:$($res.FullName)" "/out:$(Join-Path $OutputDirectory KAGARI.exe)" @refs ui\Desktop.cs ui\MessageRouting.cs ui\Analytics.cs ui\KpiHistory.cs ui\AttachmentPreview.cs ui\Usage.cs ui\GuardHistory.cs ui\GuardDashboard.cs ui\GuardFlow.cs ui\Culture.cs ui\News.cs ui\Jobs.cs ui\JobDetails.cs ui\NativeManagerHost.cs ui\ProjectMap.cs ui\ProjectDocuments.cs
 if($LASTEXITCODE){throw 'UIビルド失敗'}
 Copy-Item target\release\kagari_core.dll (Join-Path $OutputDirectory kagari_core.dll) -Force
} finally {Pop-Location}
