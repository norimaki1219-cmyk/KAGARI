$ErrorActionPreference='Stop'
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$codex=if($env:CODEX_HOME){$env:CODEX_HOME}else{Join-Path $env:USERPROFILE '.codex'}
$claude=if($env:CLAUDE_CONFIG_DIR){$env:CLAUDE_CONFIG_DIR}else{Join-Path $env:USERPROFILE '.claude'}
$titles=@{}
$index=Join-Path $codex 'session_index.jsonl'
if(Test-Path -LiteralPath $index){foreach($line in [IO.File]::ReadLines($index)){try{$row=$line|ConvertFrom-Json; if($row.id){$titles[$row.id]=$row.thread_name}}catch{}}}
$files=@(Get-ChildItem -LiteralPath (Join-Path $codex 'sessions') -Recurse -Filter 'rollout-*.jsonl' -File -ErrorAction SilentlyContinue)
$files+=@(Get-ChildItem -LiteralPath (Join-Path $claude 'projects') -Recurse -Filter '*.jsonl' -File -ErrorAction SilentlyContinue | Where-Object {$_.Directory.Parent.Name -eq 'projects' -and $_.BaseName -match '^[a-f0-9-]{36}$'})
foreach($f in $files | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 160){
 $agent=1
 $id=$f.BaseName
 if($id.StartsWith('rollout-')){$agent=0;$id=$id.Substring($id.Length-36)}
 if($id -notmatch '^[a-f0-9]{8}(-[a-f0-9]{4}){3}-[a-f0-9]{12}$'){continue}
 $title=$titles[$id];$cwd=''
 try{
 $count=0
 foreach($line in [IO.File]::ReadLines($f.FullName)){
 if(++$count -gt 100){break}
 if($line.Length -gt 1000000){continue}
 try{$r=$line|ConvertFrom-Json}catch{continue}
 if($agent -eq 0){
 if($r.type -eq 'session_meta'){$cwd=$r.payload.cwd}
 if(!$title -and $r.type -eq 'event_msg' -and $r.payload.type -eq 'user_message'){$title=$r.payload.message}
 }else{
 if($r.cwd){$cwd=$r.cwd}
 if(!$title -and $r.type -eq 'user' -and !$r.isMeta){
 $body=$r.message.content
 if($body -is [string]){$title=$body}else{$title=($body | Where-Object type -eq 'text' | Select-Object -First 1).text}
 }}
 if($title -and $cwd){break}
 }
 if(!$cwd){continue}
 if(!$title){$title="セッション $id"}
 $title=($title -replace '\s+',' ').Trim()
 $title=$title.Substring(0,[Math]::Min(100,$title.Length))
 [Console]::WriteLine(("{0}`t{1}`t{2}`t{3}`t{4}" -f $agent,$id,$cwd,$title,$f.LastWriteTime.ToString('MM/dd HH:mm')))
 }catch{continue}
}
