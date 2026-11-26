# Only capture the CLI's per-call working directory; the CLI retains its permission checks.
$ErrorActionPreference='Stop'
[Console]::InputEncoding=[Text.UTF8Encoding]::new($false)
$event=[Console]::In.ReadToEnd() | ConvertFrom-Json
if($event.tool_use_id -cmatch '^[A-Za-z0-9_-]{1,160}$'){
 $path=[IO.Path]::Combine($env:KAGARI_RUN,'context-'+$event.tool_use_id+'.json')
 [IO.File]::WriteAllText($path,($event | Select-Object cwd,tool_name,tool_input,session_id,agent_id,permission_mode | ConvertTo-Json -Depth 32 -Compress),[Text.UTF8Encoding]::new($false))
}
