$queueBlock=[regex]::Match($reply,'(?ms)^```kagari-queue[^\S\r\n]*\r?\n(.*?)^\x60{3}[^\S\r\n]*\r?$')
if($queueBlock.Success){
 $parsed=ConvertFrom-Json -InputObject $queueBlock.Groups[1].Value -ErrorAction Stop
 $items=@($parsed)
 if($items.Count -lt 1 -or $items.Count -gt 300){throw '作業キューの件数が範囲外です（1～300件）'}
 $rows=@($env:KAGARI_PROJECT,$env:KAGARI_AGENT)
 if($env:KAGARI_HISTORY_KEY){$rows+=("parent:"+$env:KAGARI_HISTORY_KEY)}
 foreach($item in $items){
  if($item.title -isnot [string] -or $item.action -isnot [string]){$rows+="invalid";continue}
  $title=($item.title -replace '[\t\r\n]+',' ').Trim()
  $action=($item.action -replace '[\t\r\n]+',' ').Trim()
  if(!$title -or !$action -or $title.Length -gt 300 -or $action.Length -gt 12000){$rows+="invalid";continue}
  $condition=([string]$item.condition -replace '[\t\r\n]+',' ').Trim()
  if($condition.Length -gt 240){$rows+="invalid";continue}
  $rows+=($title+[char]9+$action+[char]9+$condition)
 }
 $inbox=Join-Path $env:LOCALAPPDATA 'RustAIWorkspace\queue-inbox'
 [void][IO.Directory]::CreateDirectory($inbox)
 $id=[Guid]::NewGuid().ToString()
 $tmpQueue=Join-Path $inbox ($id+'.tmp')
 [IO.File]::WriteAllLines($tmpQueue,[string[]]$rows,$utf)
 [IO.File]::Move($tmpQueue,(Join-Path $inbox ($id+'.queue')))
 $reply=$reply.Remove($queueBlock.Index,$queueBlock.Length).TrimEnd()+[Environment]::NewLine+"作業キューへの登録受付　$($items.Count)件"
}

# The host supplies the task identity and expected state; the model can only signal completion.
if($env:KAGARI_COMPLETE_ID -cmatch '^[a-f0-9]{32}$' -and $reply -cmatch '(?m)^KAGARI_TASK_COMPLETE\s*\z'){
 $inbox=Join-Path $env:LOCALAPPDATA 'RustAIWorkspace\queue-inbox'
 [void][IO.Directory]::CreateDirectory($inbox)
 $key=[Guid]::NewGuid().ToString('N')
 $attempt=Split-Path -Leaf $dir
 $origin=$env:KAGARI_HISTORY_KEY+'/'+$attempt
 $fields=@($key,$env:KAGARI_COMPLETE_ID,$env:KAGARI_EXPECTED_STATE,'3',$origin)
 $tmp=Join-Path $inbox ($key+'.tmp')
 [IO.File]::WriteAllText($tmp,($fields -join [char]9),$utf)
 [IO.File]::Move($tmp,(Join-Path $inbox ($key+'.status')))
 $reply=($reply -creplace '(?m)^KAGARI_TASK_COMPLETE\s*\z','').TrimEnd()+[Environment]::NewLine+'KAGARIに完了更新を依頼しました。'
}

if($env:KAGARI_DOCS_ID -cmatch '^[a-f0-9]{32}$'){
 $docsBlock=[regex]::Match($reply,'(?ms)^\x60{3}kagari-docs[^\S\r\n]*\r?\n(.*?)^\x60{3}[^\S\r\n]*\r?$')
 if($docsBlock.Success){
  $docs=ConvertFrom-Json -InputObject $docsBlock.Groups[1].Value -ErrorAction Stop
  $fields=@($env:KAGARI_DOCS_ID)
  foreach($slot in 'spec','log','result'){
   $value=$docs.$slot
   if($null -ne $value -and ($value -isnot [string] -or $value -match '[\x00-\x1f]')){throw '資料のパスが無効です。'}
   $fields+=[string]$value
  }
  if(@($fields[1..3]|Where-Object {$_}).Count -eq 0){throw '登録する資料が指定されていません。'}
  $inbox=Join-Path $env:LOCALAPPDATA 'RustAIWorkspace\queue-inbox'
  [void][IO.Directory]::CreateDirectory($inbox)
  $prefix=Join-Path $inbox ([Guid]::NewGuid().ToString('N'))
  [IO.File]::WriteAllText("$prefix.tmp",($fields -join [char]9),$utf)
  [IO.File]::Move("$prefix.tmp","$prefix.docs")
  $reply=$reply.Remove($docsBlock.Index,$docsBlock.Length).TrimEnd()+[Environment]::NewLine+'KAGARIに資料登録を依頼しました。'
 }
}
