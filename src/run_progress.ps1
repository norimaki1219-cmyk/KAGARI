# Preserve visible messages and completed tool results in the request's own history.
function Save-ProgressText($label, [string]$value, [bool]$tool){
 if([string]::IsNullOrWhiteSpace($value)){$value='（出力なし）'}
 [IO.File]::AppendAllText((Join-Path $dir 'partial.txt'),($label+"　"+$value+[Environment]::NewLine),$utf)
 if($tool){
  $line=($value.Substring(0,[Math]::Min(240,$value.Length)) -replace '\s+',' ').Trim()
  [IO.File]::WriteAllText((Join-Path $dir 'last-tool.txt'),($label+"　"+$line),$utf)
 }
}
function Save-CodexProgress($event){
 if($event.type -cne 'item.completed'){return}
 $item=$event.item
 switch($item.type){
  'agent_message' {Save-ProgressText 'AI' $item.text $false}
  'command_execution' {
   if($item.aggregated_output -match '(?im)^\s*(?:Failed to create unified exec process:\s*|windows sandbox failed:\s*)(registered Core launch requires an installed app|app runtime provisioning service is unavailable)'){
    $script:runnerFailure=$Matches[1]
   }elseif($item.exit_code -eq 0){$script:runnerFailure=$null}
   Save-ProgressText ("コマンド終了（終了コード "+$item.exit_code+"）") $item.aggregated_output $true
  }
  {$_ -in @('mcp_tool_call','file_change','web_search','collab_tool_call')} {
   Save-ProgressText $item.type ($item | ConvertTo-Json -Depth 32 -Compress) $true
  }
 }
}
function Save-ClaudeProgress($event){
 foreach($part in $event.message.content){
  if($event.type -ceq 'assistant' -and $part.type -ceq 'text'){
   Save-ProgressText 'AI' $part.text $false
  }elseif($event.type -ceq 'user' -and $part.type -ceq 'tool_result'){
   $value=if($part.content -is [string]){$part.content}else{ConvertTo-Json -InputObject $part.content -Compress -Depth 32}
   $label=if($part.is_error){'ツールエラー'}else{'ツール結果'}
   Save-ProgressText $label $value $true
  }
 }
}
