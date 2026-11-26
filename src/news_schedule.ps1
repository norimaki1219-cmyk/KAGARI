param([ValidateSet('get','set')][string]$Mode='get',[ValidateSet('news','jobs')][string]$Feed='news')
$ErrorActionPreference='Stop'
$env:PSModulePath=Join-Path $PSHOME Modules
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
[Console]::InputEncoding=[Text.UTF8Encoding]::new($false)
try{
 $user=[Security.Principal.WindowsIdentity]::GetCurrent()
 $name='KAGARI-AI-News-'+$user.User.Value
 if($Feed -eq 'jobs'){$name='KAGARI-AI-Jobs-'+$user.User.Value}
 $service=New-Object -ComObject Schedule.Service
 $service.Connect()
 $root=$service.GetFolder('\')
 $task=$null
 try{$task=$root.GetTask($name)}catch{if($_.Exception.HResult -ne -2147024894){throw}}
 if($Mode -eq 'set'){
  $c=[Console]::In.ReadToEnd() | ConvertFrom-Json
  if($c.time -notmatch '^([01]\d|2[0-3]):[0-5]\d$'){throw '時刻は HH:mm 形式で入力してください。'}
  if($c.agents -notin 'muse','dots','both'){throw '担当を選択してください。'}
  $exe=(Resolve-Path (Join-Path $PSScriptRoot '..\KAGARI.exe')).Path
  $def=$service.NewTask(0)
  $def.RegistrationInfo.Description='KAGARI AIニュースの定期巡回（日本時間）'
  if($Feed -eq 'jobs'){$def.RegistrationInfo.Description='KAGARI 生成AI案件の定期巡回（日本時間）'}
  $def.Principal.UserId=$user.Name
  $def.Principal.LogonType=3
  $def.Settings.StartWhenAvailable=$true
  $def.Settings.DisallowStartIfOnBatteries=$false
  $def.Settings.StopIfGoingOnBatteries=$false
  $def.Settings.MultipleInstances=2
  $def.Settings.ExecutionTimeLimit='PT1H'
  $def.Settings.Enabled=[bool]$c.enabled
  $trigger=$def.Triggers.Create(2)
  $date=[DateTimeOffset]::UtcNow.ToOffset([TimeSpan]::FromHours(9))
  $next=[DateTimeOffset]::ParseExact(($date.ToString('yyyy-MM-dd')+'T'+$c.time+':00+09:00'),'yyyy-MM-ddTHH:mm:sszzz',[Globalization.CultureInfo]::InvariantCulture)
  if($next -le $date){$next=$next.AddDays(1)}
  $trigger.StartBoundary=$next.ToString('yyyy-MM-ddTHH:mm:sszzz')
  $trigger.DaysInterval=1
  $action=$def.Actions.Create(0)
  $action.Path=$exe
  $action.Arguments='--news-patrol '+$c.agents
  if($Feed -eq 'jobs'){$action.Arguments='--jobs-patrol'}
  $action.WorkingDirectory=[IO.Path]::GetDirectoryName($exe)
  $task=$root.RegisterTaskDefinition($name,$def,6,$user.Name,$null,3)
 }
 $out=@{enabled=$false;time='09:00';agents='both';next=''}
 if($null -ne $task){
  $out.enabled=$task.Enabled
  $out.time=[DateTimeOffset]::Parse($task.Definition.Triggers.Item(1).StartBoundary).ToOffset([TimeSpan]::FromHours(9)).ToString('HH:mm')
  $out.agents=$task.Definition.Actions.Item(1).Arguments.Split(' ')[-1]
  if($out.enabled){$out.next=$task.NextRunTime.ToString('yyyy-MM-dd HH:mm')}
 }
 [Console]::Write(($out|ConvertTo-Json -Compress))
}catch{[Console]::Error.Write($_.Exception.Message);exit 1}
