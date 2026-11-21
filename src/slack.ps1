$ErrorActionPreference='Stop'
# Use the Windows PowerShell modules that match the helper process.
$env:PSModulePath=Join-Path $PSHOME 'Modules'
$utf=[Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding=$utf
[Console]::InputEncoding=$utf
try {
 $channels=(Get-Content -LiteralPath $env:KAGARI_SLACK_CONFIG -Raw -Encoding UTF8 | ConvertFrom-Json).channels.PSObject.Properties
 if($env:KAGARI_SLACK_ACTION -eq 'channels'){
  foreach($entry in $channels | Sort-Object {$_.Value.name}){
   if($entry.Name -cmatch '^[CG][A-Z0-9]+$' -and $entry.Value.name -and $entry.Value.name -notmatch '[\t\r\n]'){
    [Console]::WriteLine($entry.Name+[char]9+$entry.Value.name)
   }
  }
  exit 0
 }
 $channel=$env:KAGARI_SLACK_CHANNEL
 if($env:KAGARI_SLACK_ACTION -ne 'post' -or $channel -cnotmatch '^[CG][A-Z0-9]+$' -or $channel -cnotin @($channels.Name)){throw '送信先を選び直してください。'}
 $text=[Console]::In.ReadToEnd()
 if([string]::IsNullOrWhiteSpace($text) -or $text.Length -gt 40000){throw '本文を1〜40000文字で入力してください。'}
 $token=$env:SLACK_BOT_TOKEN
 if(!$token){
  $path=Join-Path $env:LOCALAPPDATA 'slack_bot\secrets\bot-token.dpapi'
  if(!(Test-Path -LiteralPath $path)){throw 'slack_bot のBotトークンを設定してください。'}
  $secret=ConvertTo-SecureString ([IO.File]::ReadAllText($path).Trim())
  $ptr=[Runtime.InteropServices.Marshal]::SecureStringToBSTR($secret)
  try{$token=[Runtime.InteropServices.Marshal]::PtrToStringBSTR($ptr)}finally{[Runtime.InteropServices.Marshal]::ZeroFreeBSTR($ptr)}
 }
 $body=@{channel=$channel;text=$text;unfurl_links=$false;unfurl_media=$false} | ConvertTo-Json -Compress
 [Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12
 try{
  $result=Invoke-RestMethod -Method Post -Uri 'https://slack.com/api/chat.postMessage' -MaximumRedirection 0 -TimeoutSec 20 -Headers @{Authorization="Bearer $token"} -ContentType 'application/json; charset=utf-8' -Body $utf.GetBytes($body)
 }catch{throw '送信結果を確認できません。再送前にSlackを確認してください。'}
 if(!$result.ok){throw ("Slack: "+$result.error)}
 if(!$result.ts -or $result.channel -cne $channel){throw '送信結果を確認できません。Slackを確認してください。'}
}catch{[Console]::Write($_.Exception.Message);exit 1}
