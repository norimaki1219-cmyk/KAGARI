# State belongs to one CLI invocation. Do not store child messages or prompts.
$children=@{}
$taskLinks=@{}
$stats=@{last=''}
function Set-ChildState($id,$model,$active) {
 if(!$id){return}
 if(!$children.ContainsKey($id)){$children[$id]=@{model='モデル不明';active=$true;background=$false}}
 $child=$children[$id]
 if($model -and $model -ne 'inherit' -and $model -ne '<synthetic>'){$child.model=$model}
 if($null -ne $active){$child.active=$active}
}
function Get-CodexChildModel([string]$id) {
 # UUIDv7 contains the creation date of the child's rollout; inspect only that thread.
 if($id -notmatch '^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[0-9a-f]{4}-[0-9a-f]{12}$'){return}
 $time=[DateTimeOffset]::FromUnixTimeMilliseconds([Convert]::ToInt64($id.Replace('-','').Substring(0,12),16))
 $homeDir=if($env:CODEX_HOME){$env:CODEX_HOME}else{Join-Path $env:USERPROFILE '.codex'}
 foreach($day in @($time.UtcDateTime,$time.LocalDateTime) | ForEach-Object {$_.ToString('yyyy/MM/dd')} | Select-Object -Unique){
  $folder=Join-Path $homeDir "sessions/$day"
  $file=Get-ChildItem -LiteralPath $folder -Filter "rollout-*-$id.jsonl" -File -ErrorAction SilentlyContinue | Select-Object -First 1
  if(!$file){continue}
  foreach($line in (Get-Content -LiteralPath $file.FullName -Encoding UTF8 -TotalCount 24 -ErrorAction SilentlyContinue)){
   if($line -notmatch '"turn_context"'){continue}
   $row=$line | ConvertFrom-Json -ErrorAction SilentlyContinue
   if($row.type -eq 'turn_context' -and $row.payload.model){return $row.payload.model}
  }
 }
}
function Update-ChildStatus($event) {
 if(!$event){return}
 if($agent -eq 'codex'){
  $item=$event.item
  if($item.type -ne 'collab_tool_call'){return}
  foreach($id in $item.receiver_thread_ids){
   if(!$id){continue}
   $state=$item.agents_states.$id
   if(!$children.ContainsKey($id) -and $item.tool -ne 'spawn_agent'){continue}
   $model=$state.model
   if(!$model -and (!$children.ContainsKey($id) -or $children[$id].model -eq 'モデル不明')){$model=Get-CodexChildModel $id}
   $active=switch($state.status){
    {$_ -in 'completed','errored','interrupted','shutdown','not_found'} {$false;break}
    {$_ -in 'running','pending_init'} {$true;break}
    default {if($item.tool -eq 'close_agent' -and $item.status -eq 'completed'){$false}}
   }
   Set-ChildState $id $model $active
  }
 }else{
  if($event.type -eq 'assistant'){
   $parent=$event.parent_tool_use_id
   if($parent -and $children.ContainsKey($parent)){Set-ChildState $parent $event.message.model $null}
   foreach($block in $event.message.content){
    if($block.type -ne 'tool_use' -or $block.name -notin 'Agent','Task' -or !$block.id){continue}
    Set-ChildState $block.id $block.input.model $null
    $children[$block.id].background=($block.input.run_in_background -eq $true)
   }
  }elseif($event.type -eq 'user'){
   foreach($block in $event.message.content){
    $id=$block.tool_use_id
    if($block.type -ne 'tool_result' -or !$id -or !$children.ContainsKey($id)){continue}
    if(!$children[$id].background -or $block.is_error){Set-ChildState $id $null $false}
   }
  }elseif($event.type -eq 'system' -and $event.task_id){
   $id=$event.tool_use_id
   if(!$id){$id=$taskLinks[$event.task_id]}
   if(!$id -or !$children.ContainsKey($id)){return}
   $taskLinks[$event.task_id]=$id
   if($event.subtype -eq 'task_notification' -and $event.status -in 'completed','failed','stopped'){Set-ChildState $id $null $false}
  }else{return}
 }
 if(!$children.Count){return}
 $groups=$children.Values | Group-Object { $_.model } | Sort-Object Name
 $counts=foreach($group in $groups){
  $live=@($group.Group | Where-Object {$_.active}).Count
  "$($group.Name)　$live/$($group.Count)"
 }
 $activeCount=@($children.Values | Where-Object {$_.active}).Count
 $summary="サブエージェント　稼働 $activeCount / 合計 $($children.Count)　｜　"+($counts -join '　・　')
 if($summary -ne $stats.last){
  [IO.File]::WriteAllText((Join-Path $dir 'subagents.txt'),$summary,$utf)
  $stats.last=$summary
 }
}
