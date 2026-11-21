$ErrorActionPreference='Stop'
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$codex=if($env:CODEX_HOME){$env:CODEX_HOME}else{Join-Path $env:USERPROFILE '.codex'}
$claude=if($env:CLAUDE_CONFIG_DIR){$env:CLAUDE_CONFIG_DIR}else{Join-Path $env:USERPROFILE '.claude'}
function Normalize-ProjectPath([string]$path) {
 if($path.StartsWith('\\?\UNC\',[StringComparison]::OrdinalIgnoreCase)){$path='\\'+$path.Substring(8)}
 elseif($path.StartsWith('\\?\')){$path=$path.Substring(4)}
 return [IO.Path]::GetFullPath($path).TrimEnd('\')
}
$project=Normalize-ProjectPath $env:KAGARI_HISTORY_PROJECT
$company=([IO.Path]::GetFileName($project) -ieq '.company')
$prefix=$project+'\'
$parent=[IO.Path]::GetDirectoryName($project)
$department=if($parent -and [IO.Path]::GetFileName($parent) -ieq '.company'){[IO.Path]::GetFileName($project)}
function Test-Department([string]$request,[string]$sessionTitle) {
 $name=[regex]::Escape($department)
 $opening=$request.TrimStart([char]0xfeff).TrimStart()
 if($opening -match '^\[Slack\s+#([\w-]+)'){return $Matches[1] -ieq $department}
 $explicit="^(?:#|【|\[)?$name(?![a-z0-9_-])"
 return $opening -match $explicit -or $sessionTitle -match $explicit
}
function Get-ClaudeTitle($file) {
 $auto=''; $custom=''
 foreach($line in [IO.File]::ReadLines($file)){
  if($line -notmatch '^\s*\{\s*"type"\s*:\s*"(ai-title|custom-title)"'){continue}
  try{$r=$line | ConvertFrom-Json -ErrorAction Stop}catch{continue}
  if($r.type -eq 'custom-title'){$custom=$r.customTitle}else{$auto=$r.aiTitle}
 }
 if($custom){return $custom}
 return $auto
}
function Test-ClaudeProjectTool($record) {
 if($record.type -ne 'assistant' -or $record.isSidechain){return $false}
 foreach($part in $record.message.content){
  if($part.type -ne 'tool_use'){continue}
  foreach($field in 'file_path','notebook_path','path'){
   $value=$part.input.$field
   if($value -isnot [string] -or ![IO.Path]::IsPathRooted($value)){continue}
   try{$target=Normalize-ProjectPath $value}catch{continue}
   if($target -ieq $project -or $target.StartsWith($prefix,[StringComparison]::OrdinalIgnoreCase)){return $true}
  }
 }
 return $false
}
$countShown=0
$titles=@{}
$index=Join-Path $codex 'session_index.jsonl'
if(Test-Path -LiteralPath $index){foreach($line in [IO.File]::ReadLines($index)){try{$row=$line|ConvertFrom-Json; if($row.id){$titles[$row.id]=$row.thread_name}}catch{}}}
$files=@(Get-ChildItem -LiteralPath (Join-Path $codex 'sessions') -Recurse -Filter 'rollout-*.jsonl' -File -ErrorAction SilentlyContinue)
$files+=@(Get-ChildItem -LiteralPath (Join-Path $claude 'projects') -Recurse -Filter '*.jsonl' -File -ErrorAction SilentlyContinue | Where-Object {$_.Directory.Parent.Name -eq 'projects' -and $_.BaseName -match '^[a-f0-9-]{36}$'})
foreach($f in $files | Sort-Object LastWriteTimeUtc -Descending){
 $agent=1
 $id=$f.BaseName
 if($id.StartsWith('rollout-')){$agent=0;$id=$id.Substring($id.Length-36)}
 if($id -notmatch '^[a-f0-9]{8}(-[a-f0-9]{4}){3}-[a-f0-9]{12}$'){continue}
 $title=$titles[$id];$cwd='';$request='';$sharedRoot=$false;$matched=$false
 try{
 $count=0
 foreach($line in [IO.File]::ReadLines($f.FullName)){
 if(++$count -gt 100 -and !$sharedRoot){break}
 if($line.Length -gt 1000000){continue}
 if($sharedRoot -and $line.IndexOf($department,[StringComparison]::OrdinalIgnoreCase) -lt 0){continue}
 try{$r=$line|ConvertFrom-Json}catch{continue}
 if($agent -eq 0){
 if($r.type -eq 'session_meta'){$cwd=$r.payload.cwd}
 if(!$request -and $r.type -eq 'event_msg' -and $r.payload.type -eq 'user_message'){$request=$r.payload.message}
 }else{
 if($r.cwd){$cwd=$r.cwd}
 if(!$request -and $r.type -eq 'user' -and !$r.isMeta){
 $body=$r.message.content
 if($body -is [string]){$request=$body}else{$request=($body | Where-Object type -eq 'text' | Select-Object -First 1).text}
 }}
 if(!$title -and $request){$title=$request}
 if($cwd){
  $path=Normalize-ProjectPath $cwd
  $sharedRoot=$department -and $path -ieq $parent
  if($sharedRoot){
   if($agent -eq 1 -and (Test-ClaudeProjectTool $r)){$matched=$true;break}
   if(!$request){continue}
   if(!(Test-Department $request $titles[$id])){$request='';continue}
   $matched=$true
   if(!$titles[$id]){$title=$request}
   break
  }
  if($path -ine $project -and ($company -or !$path.StartsWith($prefix,[StringComparison]::OrdinalIgnoreCase))){$cwd='';break}
  if($title){break}
 }
 }
 if(!$cwd){continue}
 if($sharedRoot -and !$matched -and !(Test-Department "" $titles[$id])){continue}
 if($agent -eq 1){
  $nativeTitle=Get-ClaudeTitle $f.FullName
  if($nativeTitle){$title=$nativeTitle}
 }
 if(!$title){$title="セッション $id"}
 $title=($title -replace '\s+',' ').Trim()
 $title=$title.Substring(0,[Math]::Min(100,$title.Length))
 [Console]::WriteLine(("{0}`t{1}`t{2}`t{3}`t{4}" -f $agent,$id,$cwd,$title,$f.LastWriteTime.ToString('MM/dd HH:mm')))
 if(++$countShown -ge 160){break}
 }catch{continue}
}
