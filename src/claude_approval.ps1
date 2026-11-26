# Claude's bidirectional stream keeps the original tool call suspended until control_response.
function Send-Packet($packet){
 $inputWriter.WriteLine(($packet | ConvertTo-Json -Depth 64 -Compress))
 $inputWriter.Flush()
}
function Respond($event,$allowed,$reason){
 $decision=if($allowed){@{behavior='allow';updatedInput=$event.request.input}}else{@{behavior='deny';message=$reason}}
 Send-Packet @{type='control_response';response=@{subtype='success';request_id=$event.request_id;response=$decision}}
}
function Close-Prompt($p){
 [IO.File]::Delete($p.path)
 $p.pipe.Dispose()
 $pending.Remove($p.id)
}
function Cancel-Prompt($requestId){
 [void]$collect.Remove($requestId)
 foreach($group in @($pending.Values)){
  if($group.events.ContainsKey($requestId)){
   $group.events.Remove($requestId)
   if(!$group.events.Count){Close-Prompt $group}
   return
  }
 }
}
# A plan is an assistant-only turn. Commands are never evaluated by this worker.
function Review-Plan($resultEvent){
 $answer=[string]$resultEvent.result
 $planMatch=[regex]::Match($answer,'(?ms)^`{3}kagari-command-plan[^\S\r\n]*\r?\n(.*?)^`{3}[^\S\r\n]*\r?$')
 if(!$planMatch.Success){
  if(!$review.needed -or $resultEvent.is_error){return $false}
  if($review.retried){throw '実行前に必要なコマンドの計画を取得できませんでした。'}
  $review.retried=$true
  Send-Packet @{type='user';message=@{role='user';content=$planRequest}}
  return $true
 }
 if(!$resultEvent.session_id -or $resultEvent.is_error){throw '事前計画のセッションを確認できません。'}
 $items=@(ConvertFrom-Json -InputObject $planMatch.Groups[1].Value -ErrorAction Stop)
 if(!$items.Count -or $items.Count -gt 32){throw '事前計画は1～32件で指定してください。'}
 $lines=@(("今回の依頼のコマンド計画　"+$items.Count+"件"),("実行先　"+$planCwd))
 $keys=@()
 foreach($item in $items){
  if($item.tool -cnotin @('Bash','PowerShell') -or $item.command -isnot [string] -or $item.purpose -isnot [string] -or !$item.command.Trim() -or !$item.purpose.Trim() -or ($item.command+$item.purpose) -match '[\p{Cc}\p{Cf}\p{Zl}\p{Zp}]'){throw '事前計画に表示できない操作があります。'}
  $timeout=$item.timeout
  if($null -ne $timeout -and (($timeout -isnot [int] -and $timeout -isnot [long]) -or $timeout -lt 1 -or $timeout -gt 600000)){throw '事前計画のタイムアウト値が不正です。'}
  $keys+=ConvertTo-Json -Compress -InputObject @($resultEvent.session_id,$planCwd,$item.tool,$item.command,$timeout)
  $lines+=("【"+$keys.Count+"】"+$item.tool+"　"+$item.purpose)
  $lines+=$item.command
  if($null -ne $timeout){$lines+=("タイムアウト　"+$timeout+" ms")}
 }
 $summary=$lines -join "`r`n"
 if($utf.GetByteCount($summary) -gt 24000){throw '事前計画が表示可能な長さを超えました。'}
 $decision=@{allowed=$false}
 Show-Batch @() $summary $decision
 while($pending.Count){Poll-Prompts;Start-Sleep -Milliseconds 100}
 if(!$decision.allowed){throw '利用者が事前計画を拒否しました。'}
 $review.needed=$false
 $review.retried=$false
 foreach($key in $keys){[void]$planKeys.Add($key)}
 Send-Packet @{type='user';message=@{role='user';content='表示した計画が許可されました。計画と同じツール・コマンドを、同じディレクトリで必要な順番に実行してください。追加の実行オプションは必要な場合だけ指定してください。'}}
 return $true
}
function Publish-Prompt($event){
 $r=$event.request
 if($r.subtype -cne 'can_use_tool' -or $r.tool_name -cnotin @('Bash','PowerShell') -or $r.tool_use_id -cnotmatch '^[A-Za-z0-9_-]{1,160}$'){
  Respond $event $false 'この操作はKAGARIのコマンド承認の対象外です。';return
 }
 $path=[IO.Path]::Combine($dir,'context-'+$r.tool_use_id+'.json')
 $ctx=if([IO.File]::Exists($path)){([IO.File]::ReadAllText($path) | ConvertFrom-Json)}
 $inputJson=$r.input | ConvertTo-Json -Compress -Depth 32
 if(!$ctx.cwd -or !$ctx.session_id -or $ctx.tool_name -cne $r.tool_name -or ($ctx.tool_input | ConvertTo-Json -Compress -Depth 32) -cne $inputJson){
  Respond $event $false 'CLIの実行先を検証できません。コマンドは許可されませんでした。';return
 }
 $parameters=[ordered]@{}
 foreach($property in $r.input.PSObject.Properties | Sort-Object Name){
  if($property.Name -cne 'description'){$parameters[$property.Name]=$property.Value}
 }
 $key=ConvertTo-Json -InputObject @($ctx.session_id,$ctx.agent_id,$r.agent_id,$r.parent_tool_use_id,$ctx.permission_mode,$ctx.cwd,$r.tool_name,$r.decision_reason_type,$parameters) -Depth 32 -Compress
 $reusable=!$r.input.dangerouslyDisableSandbox -and !$r.input.run_in_background -and !$ctx.agent_id -and !$r.parent_tool_use_id -and !$r.agent_id
 $plannedKey=ConvertTo-Json -Compress -InputObject @($ctx.session_id,$ctx.cwd,$r.tool_name,$r.input.command,$r.input.timeout)
 # A plan covers foreground commands with only the displayed input in normal mode.
 $planned=$reusable -and @($parameters.Keys | Where-Object {$_ -cnotin @('command','timeout')}).Count -eq 0 -and $parameters.Contains('command') -and $ctx.permission_mode -ceq 'default' -and $planKeys.Contains($plannedKey)
 if($planned -or ($reusable -and $remember.Contains($key))){Respond $event $true '';return}
 $command=[string]$r.input.command
 $summary="実行先　$($ctx.cwd)`r`nツール　$($r.tool_name)`r`n`r`n$command"
 $extra=[ordered]@{}
 foreach($prop in $parameters.Keys){if($prop -cne 'command'){$extra[$prop]=$parameters[$prop]}}
 if($extra.Count){$summary+="`r`n`r`n実行オプション　"+($extra | ConvertTo-Json -Compress -Depth 32)}
 if(!$command -or $utf.GetByteCount($summary) -gt 24000 -or $summary -match '[\x00-\x08\x0b\x0c\x0e-\x1f\u202a-\u202e\u2066-\u2069]'){
  Respond $event $false 'コマンド全体を正しく表示できないため拒否しました。';return
 }
 # If the agent skipped planning, collect the plan before notifying the user.
 # Preserve ordinary per-call review for requests that cannot be represented exactly.
 $timeout=$r.input.timeout
 $validTimeout=$null -eq $timeout -or (($timeout -is [int] -or $timeout -is [long]) -and $timeout -ge 1 -and $timeout -le 600000)
 if($reusable -and $ctx.permission_mode -ceq 'default' -and $ctx.cwd -ceq $planCwd -and
    $validTimeout -and $command -notmatch '[\p{Cc}\p{Cf}\p{Zl}\p{Zp}]' -and
    @($parameters.Keys | Where-Object {$_ -cnotin @('command','timeout')}).Count -eq 0){
  $review.needed=$true
  Respond $event $false $planRequest
  return
 }
 # Freeze each displayed batch. Later requests receive their own review.
 $collect[$event.request_id]=@{event=$event;key=$key;reusable=$reusable;summary=$summary}
 if($collect.Count -eq 1){$batchTimer.Restart()}
}
function Show-Batch($entries,$summary,$decision=$null){
 $id=[Guid]::NewGuid().ToString('N')
 $pipe=[IO.Pipes.NamedPipeServerStream]::new("kagari-approval-$id",[IO.Pipes.PipeDirection]::In,1,[IO.Pipes.PipeTransmissionMode]::Byte,[IO.Pipes.PipeOptions]::Asynchronous,65536,4096)
 $path=Join-Path $dir ("approval-"+$id+".pending")
 $events=[Collections.Generic.Dictionary[string,object]]::new([StringComparer]::Ordinal)
 foreach($entry in $entries){$events.Add($entry.event.request_id,$entry)}
 $p=@{decision=$decision;events=$events;id=$id;summary=$summary;pipe=$pipe;path=$path;client=$pipe.BeginWaitForConnection($null,$null);read=$null;reader=$null}
 $pending.Add($id,$p)
 [IO.File]::WriteAllText($path+'.tmp',$summary,$utf)
 [IO.File]::Move($path+'.tmp',$path)
 Notify-Task 'コマンドの承認待ち' ('~'+$id)
}
function Flush-Batch{
 if(!$collect.Count -or $batchTimer.ElapsedMilliseconds -lt 650){return}
 $entries=@()
 $text=''
 foreach($entry in @($collect.Values)){
  $part="【"+($entries.Count+1)+"】"+$entry.summary
  $combined=if($text){$text+"`r`n`r`n"+$part}else{$part}
  if($entries.Count -and $utf.GetByteCount($combined) -gt 24000){
   Show-Batch $entries $text
   $entries=@()
   $combined=$entry.summary
  }
  if($utf.GetByteCount($combined) -gt 24000){$combined=$entry.summary}
  $entries+= $entry
  $text=$combined
 }
 if($entries.Count){Show-Batch $entries $text}
 $collect.Clear()
}
function Poll-Prompts{
 Flush-Batch
 foreach($p in @($pending.Values)){
  if(!$p.read -and $p.client.IsCompleted){
   $p.pipe.EndWaitForConnection($p.client)
   [uint32]$clientId=0
   $valid=[KagariPipePeer]::GetNamedPipeClientProcessId($p.pipe.SafePipeHandle,[ref]$clientId)
   $peer=if($valid){Get-Process -Id $clientId -ErrorAction SilentlyContinue}
   if(!$peer -or !$peer.Path -or ![String]::Equals($peer.Path,$env:KAGARI_EXE,[StringComparison]::OrdinalIgnoreCase)){
    $p.pipe.Disconnect();$p.client=$p.pipe.BeginWaitForConnection($null,$null);continue
   }
   $p.reader=[IO.StreamReader]::new($p.pipe,$utf)
   $p.read=$p.reader.ReadToEndAsync()
  }
  if($p.read -and $p.read.IsCompleted){
   $answer=$p.read.GetAwaiter().GetResult()
   $allow=[String]::Equals($answer,('A'+$p.summary),[StringComparison]::Ordinal)
   if($null -ne $p.decision){$p.decision.allowed=$allow}
   foreach($entry in $p.events.Values){
    if($allow -and $entry.reusable){[void]$remember.Add($entry.key)}
    Respond $entry.event $allow '利用者が拒否したか、表示内容が一致しませんでした。'
   }
   Close-Prompt $p
  }
 }
}
$argv=@('-p','--input-format','stream-json','--output-format','stream-json','--verbose')
$direct=$env:KAGARI_EDIT -eq '1'
if($direct){
 $argv+=@('--permission-mode','dontAsk')
}else{
 Add-Type -TypeDefinition 'using System.Runtime.InteropServices;using Microsoft.Win32.SafeHandles;public static class KagariPipePeer{[DllImport("kernel32",SetLastError=true)]public static extern bool GetNamedPipeClientProcessId(SafePipeHandle h,out uint pid);}'
 $contextCmd='powershell.exe -NoProfile -NonInteractive -EncodedCommand '+[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes([IO.File]::ReadAllText((Join-Path $dir 'approval-context.ps1'))))
 $settings=@{hooks=@{PreToolUse=@(@{matcher='Bash|PowerShell';hooks=@(@{type='command';command=$contextCmd})})}}
 $configPath=Join-Path $dir 'approval-settings.json'
 [IO.File]::WriteAllText($configPath,($settings|ConvertTo-Json -Depth 8 -Compress),$utf)
 $argv+=@('--permission-prompt-tool','stdio','--settings',$configPath)
}
$argv+=$flags
# Invoke the installed exe or npm launcher without interpolating the model, paths or prompt as shell code.
$launch='$OutputEncoding=[Console]::OutputEncoding=[Console]::InputEncoding=[Text.UTF8Encoding]::new($false);$argv=ConvertFrom-Json $env:KAGARI_CLAUDE_ARGS; & $env:KAGARI_CLI @argv;exit $LASTEXITCODE'
$psi=[Diagnostics.ProcessStartInfo]::new()
$psi.FileName=Join-Path $PSHOME 'powershell.exe'
$psi.Arguments='-NoProfile -NonInteractive -EncodedCommand '+[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($launch))
$psi.WorkingDirectory=(Get-Location).ProviderPath
$psi.EnvironmentVariables['KAGARI_CLAUDE_ARGS']=ConvertTo-Json -InputObject $argv -Compress
$psi.UseShellExecute=$false
$psi.CreateNoWindow=$true
$psi.RedirectStandardInput=$true
$psi.RedirectStandardOutput=$true
$psi.RedirectStandardError=$true
$psi.StandardOutputEncoding=$utf
$psi.StandardErrorEncoding=$utf
$pending=@{}
$collect=[System.Collections.Specialized.OrderedDictionary]::new([StringComparer]::Ordinal)
$batchTimer=[Diagnostics.Stopwatch]::new()
$review=@{needed=$false;retried=$false}
$planRequest='KAGARIの実行前確認です。まだ利用者には問い合わせていません。未許可の操作は実行せず、検証までに必要な残りのコマンドをすべて、ツール・正確なコマンド・目的・指定するtimeoutとともに kagari-command-plan のJSON配列にまとめて返してください。すでに許可済みの操作は除いてください。'
$planCwd=(Get-Location).ProviderPath
$planKeys=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$remember=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$proc=[Diagnostics.Process]::Start($psi)
try{
 $inputWriter=[IO.StreamWriter]::new($proc.StandardInput.BaseStream,$utf)
 $inputWriter.AutoFlush=$true
 $errTask=$proc.StandardError.ReadToEndAsync()
 $inputWriter.WriteLine(($packet|ConvertTo-Json -Compress -Depth 12))
 $read=$proc.StandardOutput.ReadLineAsync()
 while($true){
  if(!$read.Wait(150)){Poll-Prompts;continue}
  $line=$read.GetAwaiter().GetResult()
  if($null -eq $line){break}
  $event=$line|ConvertFrom-Json -ErrorAction SilentlyContinue
  if($event.type -ceq 'rate_limit_event'){
   # Keep one observation per window for the workspace usage pane.
   try{
    $info=$event.rate_limit_info
    if($info.rateLimitType -cmatch '^(five_hour|seven_day)$' -and $info.resetsAt){
     $percent=if($info.status -ceq 'rejected'){100}elseif($null -ne $info.utilization){100.0*[double]$info.utilization}
     if($null -ne $percent -and $percent -ge 0 -and $percent -le 100){
      $observation=@{percent=$percent;reset=$info.resetsAt;sample=[DateTimeOffset]::UtcNow.ToUnixTimeSeconds()}
      $path=Join-Path $dir ('usage-'+$info.rateLimitType+'.json')
      [IO.File]::WriteAllText($path+'.tmp',($observation|ConvertTo-Json -Compress),$utf)
      if([IO.File]::Exists($path)){[IO.File]::Replace($path+'.tmp',$path,$null)}else{[IO.File]::Move($path+'.tmp',$path)}
     }
    }
   }catch{}
  }
  if($event.type -ceq 'control_request'){Publish-Prompt $event}
  elseif($event.type -ceq 'control_cancel_request'){Cancel-Prompt $event.request_id}
  elseif($event.type -ceq 'result'){foreach($p in @($pending.Values)){Close-Prompt $p};if($direct -or !(Review-Plan $event)){$result=$event;$proc.StandardInput.Close()}}
  else{
   try{Update-ChildStatus $event}catch{}
   try{Save-ClaudeProgress $event}catch{}
   if($event.type -eq 'assistant' -and $event.message.content.type -contains 'tool_use'){recordActivity 'tool'}
   elseif($event.type -eq 'user' -and $event.message.content.type -contains 'tool_result'){recordActivity 'tool_done'}
   elseif($event.type){recordActivity 'event'}
  }
  Poll-Prompts
  $read=$proc.StandardOutput.ReadLineAsync()
 }
 $proc.WaitForExit()
 $code=$proc.ExitCode
 [IO.File]::WriteAllText($diag,$errTask.GetAwaiter().GetResult(),$utf)
}finally{
 foreach($p in @($pending.Values)){Close-Prompt $p}
 $proc.StandardInput.Close()
 $proc.Dispose()
 Get-ChildItem -LiteralPath $dir -Filter 'context-*.json' -ErrorAction SilentlyContinue | ForEach-Object{[IO.File]::Delete($_.FullName)}
}
