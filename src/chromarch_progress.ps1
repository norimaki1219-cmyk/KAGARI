   try{
    $keyfile=Join-Path $env:LOCALAPPDATA 'progress/secrets/view.dpapi'
    $secret=ConvertTo-SecureString ([IO.File]::ReadAllText($keyfile).Trim())
    $ptr=[Runtime.InteropServices.Marshal]::SecureStringToBSTR($secret)
    try{$auth=@{Authorization='Bearer '+[Runtime.InteropServices.Marshal]::PtrToStringBSTR($ptr)}}finally{[Runtime.InteropServices.Marshal]::ZeroFreeBSTR($ptr)}
    $feed=Invoke-RestMethod 'https://progress.eda-edamap.workers.dev/api/projects' -Headers $auth -MaximumRedirection 0 -TimeoutSec 6
    $projectRows=@($feed.projects|Where-Object {($_.id+' '+$_.name+' '+$_.department) -match '(?i)chromarch|クロマーク|TurfWars'}|Select-Object -First 10 name,status,current_action,issue,next_step,confirmed_at,updated_at)
    if($projectRows.Count){'共有進捗の取得 '+(Get-Date -Format 'yyyy-MM-dd HH:mm zzz')+'。内容は各項目の確認・更新日時点。';ConvertTo-Json -InputObject $projectRows -Depth 4 -Compress}else{'共有進捗に一致する項目なし。'}
   }catch{'共有進捗の今回の照会は未取得。保存済み資料の日付を明記してください。'}