param([ValidateSet('send','read')][string]$Action)
$ErrorActionPreference='Stop'
$env:PSModulePath=Join-Path $PSHOME Modules
[Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12
$utf=[Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding=$utf
[Console]::InputEncoding=$utf
try {
 $job=[Console]::In.ReadToEnd() | ConvertFrom-Json
 $token=$env:SLACK_BOT_TOKEN
 if(!$token){
  $path=Join-Path $env:LOCALAPPDATA 'slack_bot\secrets\bot-token.dpapi'
  $secure=ConvertTo-SecureString ([IO.File]::ReadAllText($path).Trim())
  $ptr=[Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
  try{$token=[Runtime.InteropServices.Marshal]::PtrToStringBSTR($ptr)}finally{[Runtime.InteropServices.Marshal]::ZeroFreeBSTR($ptr)}
 }
 $headers=@{Authorization="Bearer $token"}
 if($job.collector -eq "muse"){$peer="U0CE4BN2E4U"}elseif(!$job.collector -or $job.collector -eq "dots"){$peer="U0CDNM1HTNX"}else{throw "Unknown news collector."}
 function Slack($method,$argsMap){
  if($method -eq 'chat.postMessage'){
   $result=Invoke-RestMethod -Uri "https://slack.com/api/$method" -Method Post -Headers $headers -TimeoutSec 25 -ContentType 'application/json; charset=utf-8' -Body $utf.GetBytes(($argsMap | ConvertTo-Json -Compress))
  }else{
   $query=($argsMap.GetEnumerator() | ForEach-Object {[Uri]::EscapeDataString($_.Key)+'='+[Uri]::EscapeDataString([string]$_.Value)}) -join '&'
   $result=Invoke-RestMethod -Uri "https://slack.com/api/${method}?$query" -Headers $headers -TimeoutSec 25
  }
  if(!$result.ok){throw "$method : $($result.error)"}
  return $result
 }
 if($Action -eq 'send'){
  $response=Slack 'chat.postMessage' @{channel=$peer;text=$job.prompt;unfurl_links=$false;unfurl_media=$false}
  if($response.channel -notmatch '^D[A-Z0-9]+$' -or $response.ts -notmatch '^\d+\.\d+$'){throw 'Slack returned an invalid DM receipt.'}
  $out=@{channel=$response.channel;ts=$response.ts}
 }else{
  if(($job.channel -notmatch '^D[A-Z0-9]+$' -and !($job.channel -ceq 'C0CL6EFE3FE' -and $job.collector -ceq 'dots')) -or $job.ts -notmatch '^\d+\.\d+$'){throw 'Invalid DM receipt.'}
  $query=@{channel=$job.channel;ts=$job.ts;limit=100}
  $messages=@(do {
   $response=Slack 'conversations.replies' $query
   $response.messages | Where-Object { $_.user -ceq $peer -and $_.ts -ne $job.ts }
   $query.cursor=$response.response_metadata.next_cursor
  } while($query.cursor))
  if(!$messages.Count -and $job.channel -cne 'C0CL6EFE3FE'){
   $history=Slack 'conversations.history' @{channel=$job.channel;oldest=$job.ts;limit=40}
   $messages=@($history.messages | Where-Object {$_.user -ceq $peer -and $_.text -like "*$($job.id)*"} | Sort-Object ts)
  }
  $out=@{replies=@($messages | ForEach-Object {$_.text})}
 }
 [Console]::Write(($out | ConvertTo-Json -Depth 5 -Compress))
}catch{[Console]::Error.Write($_.Exception.Message);exit 1}
