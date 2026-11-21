$ErrorActionPreference='Stop'
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$id=$env:KAGARI_SESSION
if($id -notmatch '^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$'){return}
$codex=$env:KAGARI_AGENT -eq '0'
if($codex){
 $root=if($env:CODEX_HOME){$env:CODEX_HOME}else{Join-Path $env:USERPROFILE '.codex'}
 $folder=Join-Path $root 'sessions'
 $pattern="rollout-*-$id.jsonl"
}else{
 $root=if($env:CLAUDE_CONFIG_DIR){$env:CLAUDE_CONFIG_DIR}else{Join-Path $env:USERPROFILE '.claude'}
 $folder=Join-Path $root 'projects'
 $pattern="$id.jsonl"
}
$file=Get-ChildItem -LiteralPath $folder -Filter $pattern -File -Recurse -ErrorAction SilentlyContinue | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1
if(!$file){return}
$reply='';$previous='';$stamp=0
foreach($line in [IO.File]::ReadLines($file.FullName)){
 if($line -notmatch '"(assistant|agent_message|task_complete)"'){continue}
 try{$r=$line | ConvertFrom-Json -ErrorAction Stop}catch{continue}
 $body=$null
 if($codex){
  $p=$r.payload
  if($r.type -eq 'response_item' -and $p.type -eq 'message' -and $p.role -eq 'assistant' -and $p.phase -ne 'commentary'){
   $body=($p.content | Where-Object type -eq 'output_text' | ForEach-Object {$_.text}) -join "`n"
  }elseif($r.type -eq 'event_msg' -and $p.type -eq 'task_complete'){$body=$p.last_agent_message}
  elseif($r.type -eq 'event_msg' -and $p.type -eq 'agent_message' -and $p.phase -ne 'commentary'){$body=$p.message}
 }elseif($r.type -eq 'assistant' -and !$r.isSidechain -and !$r.isApiErrorMessage -and $r.message.stop_reason -ne 'tool_use'){
  $body=($r.message.content | Where-Object type -eq 'text' | ForEach-Object {$_.text}) -join "`n"
 }
 if([string]::IsNullOrWhiteSpace($body)){continue}
 if($body -cne $reply){$previous=$reply}
 $reply=$body
 try{$stamp=([DateTimeOffset]::Parse([string]$r.timestamp)).ToUnixTimeMilliseconds()}catch{$stamp=0}
}
if($reply){[Console]::Write(("{0}`n{1}{2}{3}" -f $stamp,$reply.Replace([string][char]0,''),[char]0,$previous.Replace([string][char]0,'')))}
