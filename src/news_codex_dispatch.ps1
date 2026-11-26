param(
 [Parameter(Mandatory=$true)][ValidateSet('prepare','register')][string]$Action,
 [string]$Day,[string]$Channel,[string]$MessageTs
)
$ErrorActionPreference='Stop'
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$assembly=[Reflection.Assembly]::LoadFrom((Resolve-Path (Join-Path $PSScriptRoot '..\KAGARI.exe')).Path)
$type=$assembly.GetType('Desktop',$true)
$flags=[Reflection.BindingFlags]'NonPublic,Static'
$today=[DateTimeOffset]::UtcNow.ToOffset([TimeSpan]::FromHours(9)).ToString('yyyy-MM-dd')
if(!$Day){$Day=$today}
$parsed=[datetime]::ParseExact($Day,'yyyy-MM-dd',[Globalization.CultureInfo]::InvariantCulture)
if($Day -gt $today){throw '将来日の依頼は登録できません。'}
$target='C0CL6EFE3FE'
$key=$Day+'/dots'
$id='KAGARI-NEWS-DOTS-'+$Day
$state=$type.GetMethod('LoadNews',$flags).Invoke($null,@())
if($Action -eq 'prepare'){
 $request=$type.GetMethod('NewsRequest',$flags)
 $params=New-Object 'object[]' 2
 $params[0]='dots';$params[1]=$state.sources
 $spec=$request.Invoke($null,$params)
 $out=@{day=$Day;id=$id;channel=$target;registered=($state.lastPatrol['dots'] -eq $Day -or $state.patrolReceipts.ContainsKey($key));message=("<@U0CDNM1HTNX> KAGARIの定期巡回 "+$id+"。登録ソースの新着で前回と重複しない内容を調査し、このスレッドに結果JSONを返信してください。依頼IDも返信に添えてください。"+[Environment]::NewLine+$spec)}
 [Console]::Write(($out|ConvertTo-Json -Compress));exit 0
}
if($Channel -cne $target -or $MessageTs -notmatch '^\d{10,}\.\d{6}$'){throw 'Slack送信結果の #news チャンネルIDと親投稿tsを指定してください。'}
$new=[Activator]::CreateInstance($type.GetNestedType('NewsReceipt'))
$new.channel=$target;$new.ts=$MessageTs;$new.id=$id
$update={param($s)
 if($s.patrolReceipts.ContainsKey($key)){
  if($s.patrolReceipts[$key].ts -cne $MessageTs){throw '同日の別スレッドが登録済みです。'}
  return
 }
 if($s.lastPatrol['dots'] -eq $Day){return}
 $s.patrolReceipts[$key]=$new
 if(!$s.lastPatrol['dots'] -or $s.lastPatrol['dots'] -lt $Day){$s.lastPatrol['dots']=$Day}
}
$dt=[Action[int]].GetGenericTypeDefinition().MakeGenericType($type.GetNestedType('NewsState'))
$cb=[System.Management.Automation.LanguagePrimitives]::ConvertTo($update,$dt)
$result=$type.GetMethod('UpdateNews',$flags).Invoke($null,@($cb))
if(!$result.patrolReceipts.ContainsKey($key) -and $result.lastPatrol['dots'] -ne $Day){throw '依頼記録の保存を確認できません。'}
if($result.patrolReceipts.ContainsKey($key)){
 Start-Process -FilePath (Join-Path $PSScriptRoot '..\KAGARI.exe') -ArgumentList @('--news-receive',$Day) -WindowStyle Hidden
}
[Console]::Write('registered')
