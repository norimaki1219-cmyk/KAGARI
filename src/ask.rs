use super::*;
use std::time::{SystemTime, UNIX_EPOCH};
pub(super) const DEFAULT: &str = "CLIの既定";
#[cfg(test)]
thread_local! {pub(super) static TEST_ACTION:RefCell<Option<fn(H)>>=const{RefCell::new(None)};}
const WORKER: &str = concat!(r#"
$ErrorActionPreference='Stop'
$dir=$env:KAGARI_RUN
$utf=[Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding=$utf
$OutputEncoding=$utf
$file='error.txt'
function Notify-Task($title,$suffix){
 if($env:KAGARI_NOTIFY -ne '1'){return}
 try {
  $appId='KAGARI.Workspace'
  $protocol=[Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Software\Classes\kagari-task')
  try{
   $protocol.SetValue('','URL:KAGARI task')
   $protocol.SetValue('URL Protocol','')
   $command=$protocol.CreateSubKey('shell\open\command')
   try{$command.SetValue('',('"'+$env:KAGARI_EXE+'" --open-task "%1"'))}finally{$command.Dispose()}
  }finally{$protocol.Dispose()}
  $key=[Microsoft.Win32.Registry]::CurrentUser.CreateSubKey("Software\Classes\AppUserModelId\$appId")
  try {
   $key.SetValue('DisplayName','KAGARI')
   if($env:KAGARI_ICON){$key.SetValue('IconUri',$env:KAGARI_ICON)}
  }finally{$key.Dispose()}
  $null=[Windows.UI.Notifications.ToastNotificationManager,Windows.UI.Notifications,ContentType=WindowsRuntime]
  $null=[Windows.UI.Notifications.ToastNotification,Windows.UI.Notifications,ContentType=WindowsRuntime]
  $null=[Windows.Data.Xml.Dom.XmlDocument,Windows.Data.Xml.Dom.XmlDocument,ContentType=WindowsRuntime]
  $xml=[Windows.Data.Xml.Dom.XmlDocument]::new()
  $xml.LoadXml('<toast activationType="protocol"><visual><binding template="ToastGeneric"><text/><text/></binding></visual></toast>')
  $xml.DocumentElement.SetAttribute('launch',('kagari-task://open/'+$env:KAGARI_HISTORY_KEY+$suffix))
  $lines=$xml.GetElementsByTagName('text')
  $null=$lines.Item(0).AppendChild($xml.CreateTextNode($title))
  $null=$lines.Item(1).AppendChild($xml.CreateTextNode($env:KAGARI_TASK_TITLE))
  $toast=[Windows.UI.Notifications.ToastNotification]::new($xml)
  [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier($appId).Show($toast)
 }catch{
  [IO.File]::WriteAllText((Join-Path $dir 'notification-error.txt'),$_.Exception.Message,$utf)
 }
}
function recordActivity($stage) {
 [IO.File]::WriteAllText((Join-Path $dir 'activity.txt'),$stage,$utf)
}
"#, include_str!("subagent_status.ps1"), "\n", include_str!("run_progress.ps1"), r#"
try {
 $agent=$env:KAGARI_AGENT
 $cli=$env:KAGARI_CLI
 $flags=@()
 if($env:KAGARI_MODEL){$flags+=@('--model',$env:KAGARI_MODEL)}
 $prompt=[IO.File]::ReadAllText((Join-Path $dir 'prompt.txt'))
 if($env:KAGARI_PROJECT_ALIAS -eq 'chromarch'){
  $extra=& ([scriptblock]::Create([IO.File]::ReadAllText((Join-Path $dir 'chromarch-progress.ps1'))))
  $prompt+=([Environment]::NewLine+($extra -join [Environment]::NewLine))
  [IO.File]::WriteAllText((Join-Path $dir 'prompt.txt'),$prompt,$utf)
 }
 $assets=Join-Path $dir 'images'
 $images=@(if(Test-Path -LiteralPath $assets){Get-ChildItem -LiteralPath $assets -File | Sort-Object Name})
 $diag=Join-Path $dir 'diagnostic.tmp'
 $ErrorActionPreference='Continue'
 if($agent -eq 'codex'){
 $last=Join-Path $dir 'last.tmp'
 if($env:KAGARI_EFFORT){$flags+=@('-c',('model_reasoning_effort='+$env:KAGARI_EFFORT))}
 $argv=@('exec')
 if($env:KAGARI_EDIT -eq '1'){$argv+=@('--sandbox','workspace-write','-c','sandbox_workspace_write.network_access=true','-c','approval_policy=never')}
 # The Pages publisher refreshes Wrangler OAuth and uses npm's cached CLI.
 if($env:KAGARI_EDIT -eq '1' -and $env:KAGARI_COMPANY_ROOT){
  $publishers=@((Join-Path $env:KAGARI_COMPANY_ROOT 'tekisei_quest'),(Join-Path $env:KAGARI_COMPANY_ROOT 'politics'),(Join-Path $env:KAGARI_COMPANY_ROOT 'eda'))
  $projectPath=$env:KAGARI_PROJECT -replace '^\\\\\?\\',''
  if($publishers -contains $projectPath){
   $roots=@((Join-Path $env:USERPROFILE '.wrangler'),(Join-Path $env:LOCALAPPDATA 'npm-cache'))
   $tomlRoots=@(foreach($root in $roots){if(Test-Path -LiteralPath $root -PathType Container){if(!$root.Contains("'")){"'"+$root.Replace('\','/')+"'"}}})
   if($tomlRoots.Count){
    $values=$tomlRoots -join ','
    $argv+=@('-c',('sandbox_workspace_write.writable_roots=['+$values+']'))
   }
  }
 }
 $session=$env:KAGARI_SESSION
 $fork=$env:KAGARI_FORK -eq '1'
 if($session){
  $argv+= $(if($fork){'fork'}else{'resume'})
 }
 foreach($img in $images){$flags+=@('--image',$img.FullName)}
 $argv+=$flags
 $argv+=@('--skip-git-repo-check','-o',$last)
 if($session){$argv+=$session}
 $argv+=@('--json','-')
 $prompt | & $cli @argv 2>$diag | ForEach-Object {
  $event=$_ | ConvertFrom-Json -ErrorAction SilentlyContinue
  if($fork -and $event.type -eq 'thread.started' -and $event.thread_id -match '^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$'){
   $ErrorActionPreference='Stop'
   $route=$env:KAGARI_BRANCH_FILE
   $tmpRoute=Join-Path $dir 'branch.tmp'
   [IO.File]::WriteAllText($tmpRoute,($event.thread_id+"`n"+$session),$utf)
   if([IO.File]::Exists($route)){[IO.File]::Replace($tmpRoute,$route,(Join-Path $dir 'branch.previous'))}else{[IO.File]::Move($tmpRoute,$route)}
   [IO.File]::WriteAllText((Join-Path $dir 'codex-child.txt'),$event.thread_id,$utf)
   $ErrorActionPreference='Continue'
  }
  try {Update-ChildStatus $event}catch{}
  try {Save-CodexProgress $event}catch{}
  if($event.type -eq 'item.started'){recordActivity 'item'}
  elseif($event.type -eq 'item.completed'){recordActivity 'item_done'}
  elseif($event.type){recordActivity 'event'}
  if($event.type -eq 'error'){$reply=$event.message}
 }
 $code=$LASTEXITCODE
 if($fork -and $code -eq 0 -and ![IO.File]::Exists((Join-Path $dir 'codex-child.txt'))){throw '分岐したセッションIDを取得できませんでした。'}
 if($code -eq 0){$reply=[IO.File]::ReadAllText($last)}
 }else{
 if($env:KAGARI_EFFORT){$flags+=@('--effort',$env:KAGARI_EFFORT)}
 if($env:KAGARI_SESSION){$flags+=@('--resume',$env:KAGARI_SESSION)}
 if($env:KAGARI_EDIT -eq '1'){
 $runnerHint='利用者が依頼した範囲のコマンドは直接実行してください。'
 }else{
 $runnerHint='KAGARIではBash・PowerShellの実際のツール呼び出しで承認が必要になると、タスク詳細に実行先とコマンドが表示されます。利用者から実行を依頼された操作は、過去の非対話実行での拒否やallow設定の有無だけで実行不能と判断せず、今回の依頼の範囲でツールを呼んでください。永続許可の追加を前提にしないでください。コマンド承認が予想される場合は実行前に、検証や後続の照会も含めて分かっている操作をまとめ、ツールを呼ばずに最終回答として Markdown の kagari-command-plan コードブロックを返してください。内容は JSON 配列で各要素は tool（Bash または PowerShell）、command（現在のディレクトリで実行する正確な1行）、purpose（目的と影響）、必要な場合だけ timeout（ミリ秒）。計画時と実行時でオプションを一致させてください。同じプロジェクトで順番に行う操作も個別の要素にします。KAGARIが確認してこのセッションに結果を返すので、許可後に順序どおり進めてください。コマンドが不要な場合は計画を出さず通常どおり対応してください。依存する操作を並列化したり、複数コマンドを連結して承認を省いたりしないでください。拒否を受けた操作は中止してください。'
 }
 if($env:KAGARI_COMPANY_ROOT){
 $flags+=@('--add-dir',$env:KAGARI_COMPANY_ROOT)
 $runnerHint+=(' 会社全体の進捗を確認する際は、追加の作業ディレクトリ '+$env:KAGARI_COMPANY_ROOT+' 配下を Read、Glob、Grep で参照できます。書き込み対象のプロジェクトは '+$env:KAGARI_PROJECT+'。それ以外のプロジェクトは参照用です。')
 }
 $flags+=@('--append-system-prompt',$runnerHint)
 if($env:KAGARI_PROJECT_EDIT_RULE){$flags+=@('--allowedTools',$env:KAGARI_PROJECT_EDIT_RULE,'Bash','PowerShell','WebFetch','WebSearch')}
 if($images.Count){
 $mime=@{'.png'='image/png';'.jpg'='image/jpeg';'.gif'='image/gif';'.webp'='image/webp'}
 $parts=@(@{type='text';text=$prompt})
 foreach($img in $images){
 $parts+=@{type='image';source=@{type='base64';media_type=$mime[$img.Extension];data=[Convert]::ToBase64String([IO.File]::ReadAllBytes($img.FullName))}}
 }
 $packet=@{type='user';message=@{role='user';content=$parts}}
 }else{$packet=@{type='user';message=@{role='user';content=$prompt}}}
 if((& $cli --help 2>$null | Select-String -SimpleMatch '--forward-subagent-text' -Quiet)){$flags+='--forward-subagent-text'}
 $ErrorActionPreference='Stop'
 $result=$null
 . ([scriptblock]::Create([IO.File]::ReadAllText((Join-Path $dir 'claude-approval.ps1'))))
 if($result.is_error -and $result.result){throw $result.result}
 if($result){$reply=$result.result}
 }
 $ErrorActionPreference='Stop'
 if($code -ne 0){
  $detail=@($reply; (Get-Content -LiteralPath $diag -Tail 4 -ErrorAction SilentlyContinue)) -join [Environment]::NewLine
  $detail=$detail.Trim()
  if(!$detail){$detail='CLI からエラーの詳細は返されませんでした。'}
  throw ($detail+[Environment]::NewLine+'CLI exit '+$code)
 }
 if($script:runnerFailure){throw ("ローカル実行環境を起動できませんでした。コマンドは未実行です。 "+$script:runnerFailure+[Environment]::NewLine+$reply)}
 if([string]::IsNullOrWhiteSpace($reply)){throw 'AIの回答を受け取れませんでした。'}
 . ([scriptblock]::Create([IO.File]::ReadAllText((Join-Path $dir 'queue-result.ps1'))))
 $file='reply.md'
}catch{$reply=$_.Exception.Message}
$tmp=Join-Path $dir 'result.tmp'
[IO.File]::WriteAllText($tmp,$reply,$utf)
[IO.File]::Move($tmp,(Join-Path $dir $file))
if($file -eq 'reply.md'){[IO.File]::Delete((Join-Path $dir 'prompt.txt'))}
foreach($leaf in @('diagnostic.tmp','last.tmp','approval-settings.json','chromarch-progress.ps1','approval-context.ps1','claude-approval.ps1')){[IO.File]::Delete((Join-Path $dir $leaf))}
$title=if($file -eq 'reply.md'){'AIの回答が届きました'}else{'AIへの依頼がエラーで終了しました'}
Notify-Task $title ''
"#);
fn unique_dir(parent: &Path) -> Result<PathBuf> {
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    for n in 0..100 {
        let path = parent.join(format!("{stamp:020}-{n:02}"));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("履歴フォルダーを作成できません".into())
}
fn base() -> Result<PathBuf> {
    Ok(state_path()?.with_file_name("history"))
}
pub(super) fn draft_image_root() -> Result<PathBuf> {
    Ok(state_path()?.with_file_name("draft-images"))
}
pub(super) fn latest_reply(task: &Task) -> Option<Reply> {
    if task.history_key.is_empty() {
        return None;
    }
    let entries = fs::read_dir(base().ok()?.join(&task.history_key)).ok()?;
    let mut replies: Vec<_> = entries.flatten().filter_map(|entry| {
        let path = message_path(&entry.path(), "reply");
        let stamp = fs::metadata(&path).ok()?.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
        Some((stamp.as_millis(), path))
    }).collect();
    replies.sort_unstable_by(|a,b| b.cmp(a));
    let mut answers = replies.into_iter().filter_map(|(time,path)| Some((time,fs::read_to_string(path).ok()?)));
    let (time,text) = answers.next()?;
    let previous = answers.find(|(_,answer)| answer != &text).map(|(_,answer)|answer).unwrap_or_default();
    Some(Reply{time,text,previous})
}
fn message_path(dir: &Path, stem: &str) -> PathBuf {
    let md = dir.join(format!("{stem}.md"));
    if md.is_file() {
        md
    } else {
        dir.join(format!("{stem}.txt"))
    }
}
fn save_request(dir: &Path, text: &str, images: &[PathBuf]) -> Result<()> {
    let mut markdown = text.to_owned();
    let assets = dir.join("images");
    if !images.is_empty() {
        fs::create_dir(&assets).map_err(|e| e.to_string())?;
    }
    for (i, path) in images.iter().enumerate() {
        let mut file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut header = [0u8; 12];
        file.read_exact(&mut header)
            .map_err(|_| "画像を読み取れません")?;
        let ext = if header.starts_with(b"\x89PNG\r\n\x1a\n") {
            "png"
        } else if header.starts_with(&[0xff, 0xd8, 0xff]) {
            "jpg"
        } else if header.starts_with(b"GIF87a") || header.starts_with(b"GIF89a") {
            "gif"
        } else if header.starts_with(b"RIFF") && &header[8..] == b"WEBP" {
            "webp"
        } else {
            return Err(format!("未対応の画像形式: {}", path.display()));
        };
        file.rewind().map_err(|e| e.to_string())?;
        let name = format!("{:04}.{ext}", i + 1);
        let mut copy = fs::File::create(assets.join(&name)).map_err(|e| e.to_string())?;
        std::io::copy(&mut file, &mut copy).map_err(|e| e.to_string())?;
        markdown.push_str(&format!("\n\n![添付画像 {}](images/{name})", i + 1));
    }
    fs::write(dir.join("request.md"), markdown).map_err(|e| e.to_string())
}
fn snippet(p: PathBuf, limit: usize) -> String {
    let Ok(f) = fs::File::open(p) else {
        return String::new();
    };
    let mut b = Vec::new();
    if f.take((limit * 4 + 4) as u64).read_to_end(&mut b).is_err() {
        return String::new();
    }
    let s = String::from_utf8_lossy(&b);
    let mut chars = s.chars();
    let mut result: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        result.push('…')
    }
    result
}
fn recent(folder: &Path) -> Vec<PathBuf> {recent_n(folder,5)}
fn recent_n(folder: &Path,limit:usize) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut paths: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("meta.txt").is_file())
        .collect();
    paths.sort_unstable_by(|a, b| b.cmp(a));
    paths.truncate(limit);
    paths.reverse();
    paths
}
pub(super) fn valid_run_key(key: &str) -> bool {
    let valid = |s: &str| !s.is_empty() && s.len() <= 64 && s.bytes().all(|c| c.is_ascii_digit() || c == b'-');
    key.split_once('/').is_some_and(|(folder,run)| valid(folder) && valid(run))
}

pub(super) fn legacy_action_run(history: Option<&String>, action: Option<&String>) -> Option<String> {
    let (history, action) = (history?, action?);
    if action.is_empty() || !valid_run_key(&format!("{history}/0")) { return None; }
    let dir = recent(&base().ok()?.join(history)).pop()?;
    let request = fs::read_to_string(message_path(&dir, "request")).ok()?;
    let tail = request.strip_prefix(action)?;
    if !tail.is_empty() && !(dir.join("images").is_dir() && tail.starts_with("\n\n![添付画像 ")) { return None; }
    let key = format!("{history}/{}", dir.file_name()?.to_str()?);
    valid_run_key(&key).then_some(key)
}

pub(super) fn action_completed(task: &Task) -> bool {
    if task.submitted_action_run.is_empty() { return false; }
    base().is_ok_and(|root| {
        let dir = root.join(&task.submitted_action_run);
        message_path(&dir,"reply").is_file() && !dir.join("error.txt").is_file()
    })
}

// A completed AI request counts as a follow-up only while the task is "未着手".
pub(super) fn awaiting_followup(task: &Task) -> bool {
    if task.status != 0 || (task.history_key.is_empty() && task.other_sessions.is_empty()) {
        return false;
    }
    base().is_ok_and(|root| awaiting_followup_in(task, &root))
}

fn awaiting_followup_in(task: &Task, root: &Path) -> bool {
    let runs: Vec<_> = std::iter::once(&task.history_key)
        .chain(task.other_sessions.iter().map(|session| &session.history_key))
        .filter(|key| !key.is_empty())
        .flat_map(|key| recent(&root.join(key)))
        .collect();
    let Some(latest) = runs.iter().max_by(|a, b| a.file_name().cmp(&b.file_name())) else {
        return false;
    };
    !latest.join("error.txt").is_file()
        && message_path(latest, "reply").is_file()
        && !runs.iter().any(|run| running(run))
}
#[link(name="kernel32")]
extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> H;
    fn GetProcessTimes(process: H, created: *mut u64, ended: *mut u64, kernel: *mut u64, user: *mut u64) -> i32;
    fn CloseHandle(handle: H) -> i32;
    fn WaitForSingleObject(handle: H, milliseconds: u32) -> u32;
}
fn process_creation(pid: u32) -> Option<u64> {
    live_creation(pid).ok().flatten()
}
fn live_creation(pid: u32) -> std::io::Result<Option<u64>> {
    unsafe {
        let handle = OpenProcess(0x1000 | 0x00100000, 0, pid);
        if handle == 0 {
            let err = std::io::Error::last_os_error();
            return if err.raw_os_error() == Some(87) { Ok(None) } else { Err(err) };
        }
        let (mut created, mut ended, mut kernel, mut user) = (0, 0, 0, 0);
        let ok = GetProcessTimes(handle, &mut created, &mut ended, &mut kernel, &mut user);
        let result = if ok == 0 { Err(std::io::Error::last_os_error()) } else {
            match WaitForSingleObject(handle, 0) {
                0 => Ok(None),           // process signaled
                258 => Ok(Some(created)), // WAIT_TIMEOUT
                _ => Err(std::io::Error::last_os_error()),
            }
        };
        CloseHandle(handle);
        result
    }
}
fn worker_live(dir: &Path) -> Option<bool> {
    let marker = fs::read_to_string(dir.join("worker.id")).ok()?;
    let (pid, created) = marker.trim().split_once(' ')?;
    let created = created.parse().ok()?;
    Some(live_creation(pid.parse().ok()?).ok()? == Some(created))
}
fn running(dir: &Path) -> bool {
    if message_path(dir, "reply").is_file() || dir.join("error.txt").is_file() { return false; }
    worker_live(dir) == Some(true)
}
fn writer_conflict(error: &str) -> bool {
    error.to_ascii_lowercase().contains("already has an active writer")
}
fn session_id(id: &str) -> bool {
    id.len() == 36 && id.bytes().enumerate().all(|(i,b)| match i {
        8 | 13 | 18 | 23 => b == b'-',
        _ => b.is_ascii_hexdigit(),
    })
}
struct CodexRoute {
    file: Option<PathBuf>,
    target: String,
    parent: Option<String>,
}
impl CodexRoute {
    fn load(folder: Option<&Path>, original: &str) -> Self {
        let file = folder.filter(|_| session_id(original))
            .map(|p| p.join(format!("codex-branch-{original}.txt")));
        let saved = file.as_ref().and_then(|p| fs::read_to_string(p).ok()).unwrap_or_default();
        let mut ids = saved.trim().lines();
        let child = ids.next().filter(|id| session_id(id));
        Self {
            file,
            target: child.unwrap_or(original).into(),
            // Older route files contain only the destination.
            parent: child.map(|_| ids.next().filter(|id| session_id(id)).unwrap_or("不明（旧履歴）").into()),
        }
    }
    fn needs_fork(&self, last: Option<&Path>) -> bool {
        let Some(run) = last else { return false };
        if self.file.is_none() || !writer_conflict(&snippet(run.join("error.txt"), 1600)) { return false; }
        let attempted = fs::read_to_string(run.join("codex-child.txt"))
            .or_else(|_| fs::read_to_string(run.join("codex-target.txt")));
        attempted.map_or(true, |id| id == self.target)
    }
    fn description(&self) -> String {
        format!("分岐元：{}\n現在の送信先：{}", self.parent.as_deref().unwrap_or("なし"),
            if self.target.is_empty() { "新規セッション" } else { &self.target })
    }
    fn configure(&self, worker: &mut Command, fork: bool) {
        worker.env("KAGARI_FORK", if fork { "1" } else { "0" })
            .env("KAGARI_BRANCH_FILE", self.file.as_deref().unwrap_or(Path::new("")))
            .env("KAGARI_SESSION", &self.target);
    }
}
pub(super) fn reply_session(task: &Task) -> String {
    if task.agent != 0 || task.history_key.is_empty() { return task.session.clone(); }
    let folder = base().ok().map(|root| root.join(&task.history_key));
    CodexRoute::load(folder.as_deref(), &task.session).target
}
struct RequestStatus {
    label: &'static str,
    detail: String,
    send: &'static str,
    needs_action: bool,
}
impl RequestStatus {
    fn idle() -> Self {
        Self { label: "未依頼", detail: String::new(), send: "依頼を送信", needs_action: false }
    }
    fn problem(label: &'static str, cause: &str, event: &Path, send: &'static str) -> Self {
        let at = fs::metadata(event).and_then(|m| m.modified()).ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .and_then(|d| editor::local_datetime(d.as_millis())).unwrap_or_else(|| "不明".into());
        let detail = format!("発生 {at}\r\n原因　{cause}");
        Self { label, detail, send, needs_action: true }
    }
    fn guidance(&self, retry: bool, fork: bool) -> String {
        if !self.needs_action {return self.detail.clone();}
        let action = if self.label == "承認待ち" {"［今回だけ許可］または［拒否］".into()}
            else if self.label == "状態不明" {"［設定・履歴］で途中結果と保存先を確認".into()}
            else if !retry {"依頼内容を確認して［依頼を送信］".into()}
            else {format!("［設定・履歴］で［{}］",if fork {"分岐して再送"} else {self.send})};
        let when = if self.label == "使用枠到達" {"枠の回復後に"} else {""};
        format!("次の操作　{when}{action}\r\n{}",self.detail)
    }
}
fn request_status(dir: &Path, live: bool, approval: Option<&Approval>) -> RequestStatus {
    let mut state = request_state(dir, live, approval);
    let tool = snippet(dir.join("last-tool.txt"), 160);
    if !tool.is_empty() {
        state.detail.push_str(&format!("　最後のツール結果：{tool}"));
    }
    state
}
fn request_state(dir: &Path, live: bool, approval: Option<&Approval>) -> RequestStatus {
    let start = fs::metadata(dir.join("meta.txt")).and_then(|m| m.modified()).ok();
    let duration = |end: SystemTime| start.and_then(|t| end.duration_since(t).ok())
        .map(|t| format!("経過 {}分{:02}秒", t.as_secs()/60, t.as_secs()%60)).unwrap_or_default();
    let normal = |label, detail, send| RequestStatus { label, detail, send, needs_action: false };
    if let Ok(meta) = fs::metadata(message_path(dir,"reply")) {
        return normal("回答受信", meta.modified().ok().map(duration).unwrap_or_default(), "依頼を送信");
    }
    let file = dir.join("error.txt");
    if file.is_file() {
        let error = snippet(file.clone(), 1600);
        let lower = error.to_ascii_lowercase();
        if writer_conflict(&error) {
            return RequestStatus::problem("セッション使用中","別の画面がこのCodexセッションに書き込み中です。",&file,"再送する");
        }
        if ["5-hour limit","usage limit","usage_limit_reached","insufficient_quota","quota exceeded","weekly limit"].iter().any(|s| lower.contains(s)) {
            let cause = error.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
            return RequestStatus::problem("使用枠到達",&format!("AIの使用枠に到達しました。 {cause}"),&file,"再送する");
        }
        let cause = error.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("CLIからエラーの詳細が返されませんでした。");
        return RequestStatus::problem("実行失敗",cause,&file,"再送する");
    }
    if !live {
        let (label, cause) = if worker_live(dir) == Some(false) {
            ("中断", "回答を受信する前に実行プロセスが終了しました。")
        } else {
            ("状態不明", "回答が未受信で、実行プロセスを確認できません。")
        };
        return RequestStatus { label, detail: format!("原因　{cause}"), send: "再送する", needs_action: true };
    }
    if let Some(a) = approval {
        return RequestStatus::problem("承認待ち","コマンドの実行に承認が必要です。",&a.path,"実行中…");
    }
    let now = SystemTime::now();
    let activity = dir.join("activity.txt");
    let stage = match fs::read_to_string(&activity).unwrap_or_default().as_str() {
        "tool" => "ツール呼び出し", "tool_done" => "ツール結果受信",
        "item" => "処理開始", "item_done" => "処理完了",
        "event" => "CLI通知受信", _ => "最初の通知待ち",
    };
    let age = fs::metadata(activity).and_then(|m|m.modified()).ok()
        .and_then(|t|now.duration_since(t).ok()).map(|d|format!("（{}秒前）",d.as_secs())).unwrap_or_default();
    normal("実行中",format!("{}　最終状況：{stage}{age}",duration(now)),"実行中…")
}
fn selected_run(task: &Task) -> Option<(Option<usize>, PathBuf, bool)> {
    let root = base().ok()?;
    let mut latest: Option<(Option<usize>, PathBuf, bool)> = None;
    for (session,key) in std::iter::once((None,&task.history_key))
        .chain(task.other_sessions.iter().enumerate().map(|(i,s)|(Some(i),&s.history_key))) {
        if key.is_empty() { continue; }
        let mut runs = recent(&root.join(key));
        if let Some(dir) = runs.iter().rev().find(|dir| running(dir)) {
            return Some((session,dir.clone(),true));
        }
        if let Some(dir) = runs.pop() {
            if latest.as_ref().is_none_or(|(_, previous, _)| dir.file_name() > previous.file_name()) {
                latest = Some((session, dir, false));
            }
        }
    }
    latest
}
pub(super) fn active_session(task: &Task) -> Option<usize> {
    let (session, _, live) = selected_run(task)?;
    live.then_some(session).flatten()
}
pub(super) fn queue_filter(task: &Task) -> QueueFilter {
    let run = selected_run(task);
    if run.as_ref().is_some_and(|(_, _, live)| *live) {
        return QueueFilter::AiRunning;
    }
    match task.status {
        2 => QueueFilter::Reviewing,
        1 if run.is_some_and(|(_, dir, _)| {
            message_path(&dir, "reply").is_file() && !dir.join("error.txt").is_file()
        }) => QueueFilter::Reviewing,
        1 => QueueFilter::Working,
        _ => QueueFilter::All,
    }
}
pub(super) fn queue_status(task: &Task) -> (String,String) {
    let Some((session,dir,live)) = selected_run(task) else {return ("AI 未依頼".into(),String::new())};
    let approval = if live {Approval::pending(&dir)} else {None};
    let state = request_status(&dir,live,approval.as_ref());
    let agent = session.map_or(task.agent,|i|task.other_sessions[i].agent);
    let session_id = session.map_or(task.session.as_str(),|i|task.other_sessions[i].id.as_str());
    let retry = !task.submitted_action_run.is_empty() && dir.ends_with(&task.submitted_action_run);
    let fork = state.label=="セッション使用中" && agent==0 && CodexRoute::load(dir.parent(),session_id).needs_fork(Some(&dir));
    (format!("{} {}",AGENTS[agent],state.label),state.guidance(retry,fork))
}
fn transcript(folder: &Path, completed_only: bool) -> String {
    render_transcript(recent(folder),completed_only)
}
fn render_transcript(paths:Vec<PathBuf>,completed_only:bool)->String{
    let mut blocks = Vec::new();
    for p in paths {
        let reply = snippet(message_path(&p, "reply"), 650);
        if reply.is_empty() && completed_only {
            continue;
        }
        let meta = snippet(p.join("meta.txt"), 140);
        let request = snippet(message_path(&p, "request"), 360);
        let result = if reply.is_empty() {
            let err = snippet(p.join("error.txt"), 500);
            let live = running(&p);
            let approval = if live { Approval::pending(&p) } else { None };
            let state = request_status(&p, live, approval.as_ref());
            let detail = &state.detail;
            let label = state.label;
            if matches!(label, "実行中" | "承認待ち") { label.into() }
            else {
                let partial = snippet(p.join("partial.txt"), 650);
                let partial = if partial.is_empty() {"途中結果の記録なし"} else {&partial};
                let mut files = vec![message_path(&p, "request")];
                files.extend(["prompt.txt", "images", "partial.txt"].map(|name| p.join(name)));
                let names = files.iter().filter(|path| path.exists())
                    .filter_map(|path| path.file_name()?.to_str()).collect::<Vec<_>>().join("、");
                format!("{label}　{detail}\n{err}\n途中結果　{partial}\n保存先　{}\n記録ファイル　{names}", p.display())
            }
        } else {
            format!("AI　{reply}")
        };
        let mut routing = String::new();
        if !completed_only {
            if let Ok(id) = fs::read_to_string(p.join("codex-target.txt")) {
                routing = match fs::read_to_string(p.join("codex-child.txt")) {
                    Ok(child) => format!("\n分岐元：{id}\n送信先：{child}"),
                    Err(_) if p.join("codex-fork").exists() => format!("\n分岐要求元：{id}（分岐先未確認）"),
                    Err(_) => format!("\n送信先：{id}"),
                };
            }
        }
        blocks.push(format!("{meta}{routing}\nあなた　{request}\n{result}"));
    }
    blocks.join("\n\n")
}
// Claude checks Edit(path) for both the Write and Edit tools.
fn project_edit_rule(project: &Path, cwd: &Path) -> Option<String> {
    let project = project.canonicalize().ok()?;
    let cwd = cwd.canonicalize().ok()?;
    let name = project.file_name()?;
    if name.eq_ignore_ascii_case(".company") { return None; }
    let path = project.to_str()?.strip_prefix(r"\\?\")?;
    let scope = if let Some((drive, rest)) = path.split_once(':') {
        if drive.len()!=1 || !drive.as_bytes()[0].is_ascii_alphabetic() {return None;}
        format!("//{drive}{}", rest.replace('\\', "/"))
    } else {
        let relative = project.strip_prefix(cwd).ok()?.to_str()?;
        if relative.is_empty() { ".".into() } else { format!("./{}",relative.replace('\\', "/")) }
    };
    if scope.contains(['*','?','[',']','(',')',',','{','}','!', '\n','\r']) { return None; }
    Some(format!("Edit({scope}/**)"))
}
pub(super) fn execution_dir(task: &Task, agent: usize) -> &Path {
    let project = Path::new(&task.project);
    let cwd = Path::new(task.working_dir());
    if (agent == 0 || project_alias(project).is_some()) && !cwd.starts_with(project) {project} else {cwd}
}
fn company_read_root(cwd: &Path) -> Option<String> {
    let company = cwd.ancestors().find(|p| p.file_name().is_some_and(|name| name.eq_ignore_ascii_case(".company")))?;
    let path = company.to_str()?;
    // The CLI expects a normal drive or UNC path, while Rust stores canonical Windows paths.
    if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        Some(format!(r"\\{unc}"))
    } else {
        Some(path.strip_prefix(r"\\?\").unwrap_or(path).to_owned())
    }
}

fn project_context(tasks: &[Task], task: &Task, request: &str) -> String {
    use std::fmt::Write;
    let root=Path::new(&task.project);
    let alias=project_alias(root);
    let name=alias.or_else(||root.file_name()?.to_str()).unwrap_or(&task.project);
    let mut context=format!("対象プロジェクト: {name}（作業先 {}）",task.project);
    if alias==Some("chromarch") {
        context.push_str("\n外部の新着確認先: Slack #chromarch（C0CDZKXA37W）。接続できればチャンネル内を日時降順でスレッド返信も含めて照会し、報告時刻と出典を付ける。以下の記録は各資料の時点。");
        if let Ok(state)=state_path() {
            let saved=snippet(state.with_file_name("project-context").join("chromarch/external-status.md"),3500);
            if !saved.is_empty(){let _=write!(context,"\n{saved}");}
        }
        if let Some(drive)=root.ancestors().last(){
            let mut reports=Vec::new();
            if let Ok(entries)=fs::read_dir(drive){
                for bundle in entries.flatten().filter(|e|e.file_name().to_string_lossy().starts_with("CHROMARCH-")){
                    let Ok(files)=fs::read_dir(bundle.path()) else{continue};
                    for file in files.flatten(){
                        if chromarch_report(drive,&file.path()) {
                            if let Ok(time)=file.metadata().and_then(|m|m.modified()){reports.push((time,file.path()));}
                        }
                    }
                }
            }
            reports.sort_unstable_by(|a,b|b.cmp(a));
            let city_queue=reports.iter().find(|(_,p)|p.file_name().is_some_and(|n|n.to_string_lossy().starts_with("QUEUE-ALL-CITIES-"))).cloned();
            if let Some((_,path))=&city_queue {
                let index=snippet(path.clone(),16000);
                let _=write!(context,"\n都市別作業票一覧 {}\n{}",path.display(),index);
            }
            for (_,path) in reports.into_iter().filter(|(_,p)|city_queue.as_ref().is_none_or(|(_,q)|p!=q)).take(4){
                let excerpt=snippet(path.clone(),1600);
                let _=write!(context,"\nローカル検証資料（冒頭抜粋） {}\n{}",path.display(),excerpt);
            }
        }
    }
    if !["進捗","進行","状況","タスク","未完了","progress","status"].iter()
        .any(|word| request.contains(word) || task.title.contains(word)) {return context;}
    let mut counts=[0usize;5];
    let mut titles=Vec::new();
    for item in tasks.iter().filter(|t| Path::new(&t.project).as_os_str().eq_ignore_ascii_case(root.as_os_str())
        || (root.file_name().is_some_and(|n|n.eq_ignore_ascii_case(".company"))
            && Path::new(&t.project).ancestors().any(|p|p.as_os_str().eq_ignore_ascii_case(root.as_os_str())))) {
        let state=task_counts::progress_state(item);
        counts[state]+=1;
        if titles.len()<80 { titles.push(format!("- [{}] {}",task_counts::PROGRESS_STATES[state],item.title.lines().next().unwrap_or(""))); }
    }
    let _=write!(context,"\nKAGARI登録タスク（送信時点・絞り込み前）: 合計{}件、未着手{}・次の作業依頼待ち{}・作業中{}・確認待ち{}・完了{}。\n{}",
        counts.iter().sum::<usize>(),counts[0],counts[4],counts[1],counts[2],counts[3],titles.join("\n"));
    if counts.iter().sum::<usize>()>titles.len(){context.push_str("\n一覧は先頭80件。集計は全件。");}
    let _=write!(context,"\n開発全体の集計は別に示すこと。進行状況ダッシュボードを利用できる場合はプロジェクト名またはID「{name}」で照会し、記録日時と対象範囲を明記。");
    if alias==Some("chromarch") {
        if let Some(parent)=root.parent(){
            let path=parent.join("csi/未完了項目.md");
            let excerpt=snippet(path.clone(),3000);
            if !excerpt.is_empty(){
                let _=write!(context,"\n参考資料（記載日時点の抜粋） {}\n{}",path.display(),excerpt);
            }
        }
    }
    context
}
// Carry a bounded record of outcomes across sessions, separately from the optional transcript.
fn reported_outcomes(root:&Path,task:&Task)->String {
    let mut runs=Vec::new();
    for key in std::iter::once(&task.history_key).chain(task.other_sessions.iter().map(|s|&s.history_key)) {
        if !valid_run_key(&format!("{key}/0")) {continue;}
        if let Ok(entries)=fs::read_dir(root.join(key)){
            runs.extend(entries.flatten().map(|e|e.path()).filter(|dir|message_path(dir,"reply").is_file()));
        }
    }
    runs.sort_unstable_by(|a,b|b.file_name().cmp(&a.file_name()).then_with(||b.cmp(a)));
    runs.dedup();
    let mut used=std::collections::HashSet::new();
    let mut blocks=Vec::new();
    for dir in runs.into_iter().take(20) {
        let reply=snippet(message_path(&dir,"reply"),16000);
        let mut lines=Vec::new();
        for (i,line) in reply.lines().map(str::trim).filter(|s|!s.is_empty()).enumerate(){
            if i>0 && !["完了","済み","通知","受領","承知","公開","残件","未確認","訂正","集計","件","送信"].iter().any(|w|line.contains(w)){continue;}
            let excerpt:String=line.chars().take(320).collect();
            if used.insert(excerpt.clone()){lines.push(excerpt);}
            if lines.len()==5{break;}
        }
        if !lines.is_empty(){blocks.push(format!("{}\n{}",snippet(dir.join("meta.txt"),90),lines.join("\n")));}
        if blocks.len()==5{break;}
    }
    blocks.join("\n\n")
}
fn prompt(t: &Task, request: &str, previous: &str) -> String {
    format!("回答はMarkdown形式で記述してください。着手前に、依頼全体（検証を含む）で追加の許可が必要なコマンドを整理してください。利用者の判断が必要なものは、処理を開始する前に、各コマンド・実行ディレクトリ・目的・影響を列挙して一度に確認してください。許可不要のタスクはそのまま進めてください。承認待ちの実行要求をまとめて提示できる環境では実際の承認画面を使用し、チャットで同じ確認を重ねないでください。先行結果に依存する操作は順序を守り、後から実行先や内容が変わった場合は追加分だけ確認してください。作業の依頼では、依頼された範囲で実行可能な作業と検証をすべて行ってから最終回答を返してください。途中経過は進捗として伝え、そのまま作業を続けてください。調査や計画の後に実装・実行・検証が残っていれば、利用者の継続指示を待たずに行ってください。失敗時は原因を調べ、安全に再試行できる工程と独立して進められる工程を済ませてください。同じ作業票の前回報告と今回の証拠を照合し、通常は追加・変更・訂正だけを報告してください。変化が確認できない場合は確認対象と結果を一文で示し、過去の集計表を再掲しないでください。全件の再掲を明示された場合はその依頼に従ってください。通知や公開は対象の依頼番号・日付・宛先を照合し、受領返信や完了記録がある対象を再送したり残件に戻したりしないでください。確認不能は未実施と区別し、新しい証拠があれば過去回答を訂正してください。最終回答には今回実施したこと、検証結果、なお残ったこととその具体的な理由を記載してください。残件がある場合は「## 次にすること」を設け、優先順に1項目1行の箇条書きで、各残件の担当（AI・利用者など）、対象、次に取る具体的な操作、完了を判断する証拠を簡潔に示してください。前提が不足する場合は何をどこで確認すれば再開できるかを書いてください。実行可能な作業は先に実施し、残件がなければこの節は不要です。実行環境の障害は原因を報告し、通常の手順や再試行の判断を利用者に差し戻さないでください。依頼範囲を超える操作や権限の承認など、利用者の判断がなければ進めない場合に限り、ID・決めること・選択肢の列を持つMarkdown表に項目をまとめ、各候補を同じセル内で<br>で分けてください。自由入力だけの項目は選択肢を空欄にしてください。\n\nタスク: {}\n仕様書: {}\n作業ログ: {}\n結果: {}\n\nこれまでのやり取り（参考）:\n{}\n\n今回の依頼:\n{}",t.title,t.docs[0],t.docs[1],t.docs[2],previous,request)
}
#[derive(PartialEq, Eq)]
pub(super) struct Approval {
    path: PathBuf,
    pub(super) id: String,
    pub(super) summary: String,
    pid: u32,
}
impl Approval {
    fn pending(dir: &Path) -> Option<Self> {
        Self::all(dir).into_iter().next()
    }
    fn all(dir: &Path) -> Vec<Self> {
        let Ok(marker) = fs::read_to_string(dir.join("worker.id")) else {return vec![]};
        let Some(pid) = marker.split_whitespace().next().and_then(|p|p.parse().ok()) else {return vec![]};
        let Ok(entries) = fs::read_dir(dir) else {return vec![]};
        let mut entries = entries.flatten().collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        entries.into_iter().filter_map(|entry| {
            let name = entry.file_name();
            let id = name.to_str()?.strip_prefix("approval-")?.strip_suffix(".pending")?;
            if id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()) { return None; }
            if entry.metadata().ok()?.len() > 24000 { return None; }
            let summary = fs::read_to_string(entry.path()).ok()?;
            Some(Self { path: entry.path(), id: id.into(), summary, pid })
        }).collect()
    }
    pub(super) fn respond(&self, allow: bool) -> Result<()> {
        use std::io::Write;
        use std::os::windows::io::AsRawHandle;
        #[link(name="kernel32")]
        extern "system" { fn GetNamedPipeServerProcessId(handle: *mut std::ffi::c_void, pid: *mut u32) -> i32; }
        if !self.path.parent().is_some_and(running) || fs::read_to_string(&self.path).ok().as_ref() != Some(&self.summary) {
            return Err("この承認要求は変更または終了しました。".into());
        }
        let mut pipe = fs::OpenOptions::new().write(true).open(format!(r"\\.\pipe\kagari-approval-{}", self.id)).map_err(|e| e.to_string())?;
        let mut pid = 0;
        if unsafe { GetNamedPipeServerProcessId(pipe.as_raw_handle(), &mut pid) } == 0 || pid != self.pid {
            return Err("依頼の実行プロセスを確認できません。".into());
        }
        let answer = format!("{}{}", if allow {"A"} else {"D"}, self.summary);
        pipe.write_all(answer.as_bytes()).map_err(|e| e.to_string())
    }
}
pub(super) fn task_approvals(task: &Task) -> Vec<(String, Approval)> {
    let Ok(root) = base() else { return vec![] };
    let mut result = Vec::new();
    for key in std::iter::once(&task.history_key).chain(task.other_sessions.iter().map(|s| &s.history_key)) {
        if key.is_empty() { continue; }
        let Ok(runs) = fs::read_dir(root.join(key)) else { continue };
        for run in runs.flatten().map(|entry|entry.path()).filter(|dir|running(dir)) {
            for prompt in Approval::all(&run) {
                result.push((format!("{key}~{}",prompt.id),prompt));
            }
        }
    }
    result
}
#[derive(Default)]
pub(super) struct Ask {
    codex_session: Option<String>,
    folder: Option<PathBuf>,
    last: String,
    history_limit:usize,
    older:Option<std::sync::mpsc::Receiver<(String,bool)>>,

    pub(super) images: Vec<PathBuf>,
    retired_images: Vec<PathBuf>,
    picked: std::collections::HashMap<PathBuf, PathBuf>,
    paste_number: usize,
    paste_dir: Option<PathBuf>,
    pub(super) running: bool,
    submitted: Option<(String,String,Vec<PathBuf>)>,
    approval: Option<Approval>,
    answered: bool,
    pub(super) failed: bool,
}
type PreviewCallback = extern "system" fn(H, H, Ptr) -> i32;
thread_local! {static PREVIEW: std::cell::Cell<Option<PreviewCallback>> = const {std::cell::Cell::new(None)};}
#[no_mangle]
pub extern "system" fn kagari_attachment_preview(callback: PreviewCallback) {
    PREVIEW.with(|slot| slot.set(Some(callback)));
}
impl Ask {
    pub(super) fn clear_images(&mut self) {
        self.retired_images.append(&mut self.images);
        self.picked.clear();
        if let Some(dir) = self.paste_dir.take() {
            let _ = fs::remove_dir_all(dir);
        }
    }
    pub(super) fn release_unused_images(&mut self, app: &App) {
        for path in self.retired_images.drain(..) {
            if !app.tasks.iter().any(|task| task.draft_images.contains(&path)) {
                let _ = fs::remove_file(&path);
                if let Some(dir) = path.parent() { let _ = fs::remove_dir(dir); }
            }
        }
        if let Some(dir) = self.paste_dir.take() { let _ = fs::remove_dir_all(dir); }
    }
    fn add_image(&mut self, source: PathBuf) -> Result<()> {
        if self.picked.contains_key(&source) || self.images.contains(&source) { return Ok(()); }
        let name = source.file_name().ok_or("画像名が不正です")?;
        let dir = unique_dir(&draft_image_root()?)?;
        let path = dir.join(name);
        if let Err(err) = fs::copy(&source, &path) {
            let _ = fs::remove_file(&path);
            let _ = fs::remove_dir(dir);
            return Err(err.to_string());
        }
        self.picked.insert(source, path.clone());
        self.images.push(path);
        Ok(())
    }
    fn paste_image(&mut self) -> Result<()> {
        let sequence = unsafe { GetClipboardSequenceNumber() };
        if self.paste_dir.is_none() {
            self.paste_dir = Some(unique_dir(&std::env::temp_dir().join("KagariClipboard"))?);
        }
        let dir = self.paste_dir.as_ref().unwrap();
        self.paste_number += 1;
        let path = dir.join(format!("貼り付け-{:03}.png", self.paste_number));
        let shell =
            PathBuf::from(std::env::var_os("SystemRoot").ok_or("Windows の場所が不明です")?)
                .join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
        let result = Command::new(shell)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-STA",
                "-Command",
                include_str!("clipboard_image.ps1"),
            ])
            .env("KAGARI_CLIPBOARD_PNG", &path)
            .creation_flags(0x08000000)
            .status()
            .map_err(|e| e.to_string())?;
        if !result.success() {
            return Err(
                "画像を貼り付けできませんでした。もう一度コピーしてお試しください。".into(),
            );
        }
        if sequence != unsafe { GetClipboardSequenceNumber() } {
            return Err("クリップボードが更新されました。もう一度貼り付けてください。".into());
        }
        self.add_image(path)
    }
    pub(super) fn refresh_attachments(&self, h: H) {
        let paths = self.images.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>().join("\n");
        let host = unsafe { GetDlgItem(h,444) };
        PREVIEW.with(|slot| {if let Some(cb)=slot.get(){cb(h,host,wide(&paths).as_ptr());}});
        let names: Vec<_> = self
            .images
            .iter()
            .filter_map(|p| p.file_name())
            .map(|n| n.to_string_lossy())
            .collect();
        set(
            h,
            413,
            &if names.is_empty() {
                String::new()
            } else {
                format!("{}枚　{}", names.len(), names.join("、"))
            },
        );
        unsafe {
            EnableWindow(GetDlgItem(h, 412), (!names.is_empty()) as i32);
        }
    }
    pub(super) fn track_submission(&mut self, task: &Task) {
        self.submitted = (!task.submitted_action_run.is_empty()).then(||
            (task.submitted_action_run.clone(), task.next_action.clone(), task.draft_images.clone()));
    }
    pub(super) fn submitted_run(&self, request: &str) -> Option<&str> {
        self.submitted.as_ref().filter(|(_,text,images)| text == request && images == &self.images)
            .map(|(key,_,_)| key.as_str())
    }
    fn retry_dir(&self, key: &str) -> Option<PathBuf> {
        let (history, attempt) = key.split_once('/')?;
        let folder = self.folder.as_ref()?;
        if folder.file_name()?.to_str()? != history { return None; }
        let dir = folder.join(attempt);
        (!message_path(&dir,"reply").is_file() && (dir.join("error.txt").is_file() ||
            (dir.join("worker.id").is_file() && !running(&dir)))).then_some(dir)
    }
    pub(super) fn update_send(&self,h:H){
        let request = get(h,403);
        let submitted = self.submitted_run(&request);
        let ready = !self.running && submitted.is_none() && (!request.trim().is_empty() || !self.images.is_empty());
        let retry = submitted.and_then(|key| self.retry_dir(key));
        set(h,404,if self.running {"実行中…"} else if submitted.is_some() {"送信済み"} else {"依頼を送信"});
        unsafe{
            EnableWindow(GetDlgItem(h,404),ready as i32);
            EnableWindow(GetDlgItem(h,445),(!self.running && retry.is_some()) as i32);
        }
    }
    fn busy(&self) -> bool {
        self.folder.as_ref().is_some_and(|p| recent(p).iter().any(|dir| running(dir)))
    }
    pub(super) fn approval_controls(&self, h: H, summary_page: bool) {
        let visible = summary_page && self.approval.is_some();
        unsafe {
            ShowWindow(GetDlgItem(h,445),if !summary_page || self.failed {5} else {0});
            for id in [440,441,442,443] { ShowWindow(GetDlgItem(h,id), if visible {5} else {0}); }
            for id in [414,420,435] { ShowWindow(GetDlgItem(h,id), if summary_page && !visible {5} else {0}); }
            for id in [442,443] { EnableWindow(GetDlgItem(h,id), (visible && !self.answered) as i32); }
        }
    }
    pub(super) fn decide(&mut self,h:H,allow:bool) -> Result<()> {
        if let Some(request) = &self.approval {
            if !self.answered { request.respond(allow)?; self.answered = true; }
            self.approval_controls(h,true);
        }
        Ok(())
    }
    pub(super) fn show(&mut self, h: H) -> bool {
        let runs = self.folder.as_ref().map(|p| recent(p)).unwrap_or_default();
        let active = runs.iter().rev().find(|p| running(p));
        let busy = active.is_some();
        let approval = active.and_then(|dir| Approval::pending(dir));
        if self.approval != approval {
            set(h,440,approval.as_ref().map(|r| r.summary.as_str()).unwrap_or(""));
            self.approval = approval;
            self.answered = false;
        }
        let state = active.or(runs.last()).map(|p| request_status(p, busy, self.approval.as_ref()))
            .unwrap_or_else(RequestStatus::idle);
        self.failed = state.needs_action && !busy;
        self.running=busy;
        let children = active.and_then(|p| fs::read_to_string(p.join("subagents.txt")).ok()).unwrap_or_default();
        if get(h,434) != children { set(h,434,&children); }
        unsafe { ShowWindow(GetDlgItem(h,434), if children.is_empty() {0} else {5}); }
        self.update_send(h);
        let route = self.codex_session.as_ref().map(|id| CodexRoute::load(self.folder.as_deref(), id));
        let retry_dir = self.submitted_run(&get(h,403)).and_then(|key| self.retry_dir(key));
        let fork = route.as_ref().is_some_and(|r| r.needs_fork(retry_dir.as_deref()));
        set(h,445,if fork {"分岐して再送"} else {"再送する"});
        let detail = state.guidance(retry_dir.is_some(),fork);
        let status = if detail.is_empty() {state.label.into()} else {format!("{}　{detail}",state.label)};
        let header=format!("タスク：{}　│　AI：{}",STATES[choice(h,109).min(3)],state.label);
        if get(h,447)!=header {set(h,447,&header);}
        if get(h,433)!=status {set(h,433,&status);}
        let value = self
            .folder
            .as_ref()
            .map(|p| transcript(p, false))
            .unwrap_or_default();
        let mut view = if value.is_empty() {
            "このタスクの履歴はまだありません".into()
        } else {
            value.replace("\r\n", "\n").replace('\n', "\r\n")
        };
        if let Some(route) = &route {
            view.push_str("\r\n\r\n");
            view.push_str(&route.description().replace('\n', "\r\n"));
        }
        if self.last != view {
            if self.history_limit>5 {
                self.queue_history(h,self.history_limit);
                self.last=view;
                return true;
            }
            set(h,432,"直近のやり取り（最大5件）");
            let first=msg(h,405,0xce,0,0);
            set(h,405,&view);
            msg(h,405,0xb6,0,first);
            unsafe{EnableWindow(GetDlgItem(h,453),self.folder.as_ref().is_some_and(|p|recent_n(p,6).len()>5) as i32);}
            self.last=view;
            return true;
        }
        if let Some((text,more))=self.older.as_ref().and_then(|rx|rx.try_recv().ok()){
            self.older=None;
            set(h,432,"やり取りの履歴");
            let previous=get(h,405);
            let mut text=text.replace("\r\n","\n").replace('\n',"\r\n");
            if let Some(route)=route{text.push_str("\r\n\r\n");text.push_str(&route.description().replace('\n',"\r\n"));}
            let first=msg(h,405,0xce,0,0);
            let extra=text.find(&previous).map_or(0,|at|text[..at].lines().count());
            set(h,405,&text);
            msg(h,405,0xb6,0,first+extra as isize);
            unsafe{EnableWindow(GetDlgItem(h,453),more as i32);}
        }
        false
    }
    pub(super) fn execute(&mut self, h: H, app: &mut App, index: &mut Option<usize>, draft: &mut Task, retry: bool) -> Result<()> {
        if self.busy() { return Ok(()); }
        let request = get(h,403);
        let (prior_request_id, retry_dir) = match (retry,self.submitted_run(&request)) {
            (false,None) if !request.trim().is_empty() || !self.images.is_empty() => (None,None),
            (true,Some(key)) => {
                let Some(dir) = self.retry_dir(key) else { return Ok(()) };
                let id = match fs::read_to_string(dir.join("request.id")) {
                    Ok(id) => id,
                    Err(e) if e.kind()==std::io::ErrorKind::NotFound => key.to_owned(),
                    Err(e) => return Err(e.to_string()),
                };
                (Some(id), Some(dir))
            }
            _ => return Ok(()),
        };
        let a = choice(h, 108);
        let model = get(h, 402);
        let model = model.trim();
        if model != DEFAULT
            && (model.len() > 128
                || !model
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-._:/".contains(&c))
                || model.starts_with('-'))
        {
            return Err("モデル名を確認してください".into());
        }
        let effort = efforts(a)[choice(h, 408)];
        let cli = client::find(a)?;
        let root = base()?;
        let folder = match &self.folder {
            Some(p) => p.clone(),
            None => unique_dir(&root)?,
        };
        let mut next = app.clone();
        let i = index.unwrap_or(next.tasks.len());
        if index.is_none() { next.tasks.push(draft.clone()); } else { next.tasks[i] = draft.clone(); }
        next.task = Some(i);
        let mut context=project_context(&next.tasks,&next.tasks[i],&request);
        context.push_str("\nSlackへの投稿を依頼されたときは、KAGARIで利用者が確認して送信します。送信ツールを呼ばず、回答に ```kagari-slack のコードブロックを1つ含めてください。ブロックは1行目に送信先チャンネルID、2行目に返信先の親投稿のts（新規投稿だけは -）、3行目以降に投稿本文だけを書きます。メンションには確認したSlackのユーザーIDを <@ID> として使ってください。宛先や返信先が不明な場合は確認してください。下書きは未送信です。利用者が「Slackに共有…」から確認して送信します。");
        if !next.tasks[i].completion_condition.is_empty(){context.push_str(&format!("\nこのタスクの完了条件：{}",next.tasks[i].completion_condition));}
        let t = &mut next.tasks[i];
        t.history_key = folder.file_name().unwrap().to_string_lossy().into();
        t.agent = a;
        t.effort = if effort == DEFAULT {
            String::new()
        } else {
            effort.to_owned()
        };
        t.model = if model == DEFAULT {
            String::new()
        } else {
            model.to_owned()
        };
        let prev = if msg(h, 406, 0xf0, 0, 0) == 1 {
            transcript(&folder, true)
        } else {
            String::new()
        };
        let outcomes=reported_outcomes(&root,t);
        if !outcomes.is_empty(){
            context.push_str("\n\nこの作業票で報告済みの内容（新しい順、過去回答の抜粋。最新の実測値ではありません）\n");
            context.push_str(&outcomes);
        }
        let allow_completion=request.lines().any(|line|line.trim()==editor::CONTINUE_OR_COMPLETE);
        let completion=if allow_completion {"\nこの依頼の完了はKAGARI本体が反映します。元の作業票の全条件を実測または保存済み成果物で確認できた場合のみ、根拠を本文に記載し、最後の独立した行に KAGARI_TASK_COMPLETE を出力してください。GUI操作や状態ファイルの編集は不要です。未完了・保留・未検証の場合はこの行を出力しないでください。"}else{""};
        let allow_docs=["資料","仕様書","作業ログ","結果"].iter().any(|s|request.contains(s)) &&
            ["登録","関連付け","紐づけ","ひも付け"].iter().any(|s|request.contains(s));
        let docs_hint=if allow_docs {"\nこのタスクへの資料登録を依頼された場合、対象のMarkdownが作業プロジェクト内に実在することを確認し、回答にkagari-docsのコードブロックを1つ出力してください。中身はspec、log、resultをキー、プロジェクトからの相対パスを値とするJSONオブジェクトです。登録しない欄は省略し、GUIや保存ファイルを直接操作しないでください。"}else{""};
        let unity_check = if Path::new(&t.project).join("ProjectSettings/ProjectVersion.txt").is_file() {
            "\nUnityの実行・動作検証が残る依頼では、今回対象のプロジェクト・シーンまたは指定のビルドを実際に起動し、検証項目と今回得た結果を記録してください。Editorでの確認が必要なら対象シーンを開いて確認してください。過去のログだけを確認した場合や起動できなかった場合は、未検証の項目を明記して作業中のままにしてください。"
        } else { "" };
        let body = format!("{context}\n\n{}\n\n{}",prompt(t, &request, &prev),queue_import::AGENT_INSTRUCTIONS)+docs_hint+completion+unity_check;
        let route = CodexRoute::load(Some(&folder), &t.session);
        let fork = a == 0 && route.needs_fork(retry_dir.as_deref());
        let dir = unique_dir(&folder)?;
        t.submitted_action_run = format!("{}/{}", t.history_key, dir.file_name().unwrap().to_string_lossy());
        let request_id = prior_request_id.unwrap_or_else(|| t.submitted_action_run.clone());
        let mut now = [0u16; 8];
        unsafe { GetLocalTime(&mut now) };
        let meta = format!(
            "{:04}/{:02}/{:02} {:02}:{:02}　{} / {}",
            now[0], now[1], now[3], now[4], now[5], AGENTS[a], model
        );
        let write =
            |name: &str, value: &str| fs::write(dir.join(name), value).map_err(|e| e.to_string());
        let mut prepare = || -> Result<()> {
            if a == 1 {
                write("claude-approval.ps1", include_str!("claude_approval.ps1"))?;
                write("approval-context.ps1", include_str!("approval_context.ps1"))?;
            }
            if project_alias(Path::new(&draft.project))==Some("chromarch") {
                write("chromarch-progress.ps1",include_str!("chromarch_progress.ps1"))?;
            }
            write("queue-result.ps1",include_str!("queue_result.ps1"))?;
            write("request.id",&request_id)?;
            save_request(&dir, &request, &self.images)?;
            write("prompt.txt", &body)?;
            write("meta.txt", &meta)?;
            if a == 0 && !route.target.is_empty() {
                write("codex-target.txt", &route.target)?;
                if fork { write("codex-fork", "")?; }
            }
            next.save()
        };
        if let Err(e) = prepare() {
            let _ = fs::remove_dir_all(&dir);
            return Err(e);
        }
        draft.history_key = next.tasks[i].history_key.clone();
        draft.submitted_action_run.clone_from(&next.tasks[i].submitted_action_run);
        self.track_submission(draft);
        *app = next;
        *index = Some(i);
        self.folder = Some(folder);
        let shell =
            PathBuf::from(std::env::var_os("SystemRoot").ok_or("Windows の場所が不明です")?)
                .join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
        let project = Path::new(&app.tasks[i].project);
        let cwd = execution_dir(&app.tasks[i], a);
        let edit_rule = project_edit_rule(project, cwd);
        let mut worker = Command::new(shell);
        if a == 0 {
            route.configure(&mut worker, fork);
            if client::standalone_codex().as_ref() == Some(&cli) {
                // The standalone CLI initializes its own Windows sandbox outside the desktop app.
                worker.env_remove("CODEX_WINDOWS_REGISTERED_CORE");
                worker.env_remove("CODEX_WINDOWS_SANDBOX_PACKAGE_FAMILY");
            }
        }
        else { worker.env("KAGARI_SESSION", &app.tasks[i].session); }
        let result = worker
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .current_dir(cwd)
            .creation_flags(0x08000000)
            .args(["-NoProfile", "-NonInteractive", "-Command", WORKER])
            .env("KAGARI_AGENT", if a == 0 { "codex" } else { "claude" })
            .env("KAGARI_MODEL", &app.tasks[i].model)
            .env("KAGARI_EFFORT", &app.tasks[i].effort)
            .env("KAGARI_COMPANY_ROOT", company_read_root(cwd).unwrap_or_default())
            .env("KAGARI_PROJECT_ALIAS", project_alias(project).unwrap_or(""))
            .env("KAGARI_PROJECT", project)
            .env("KAGARI_EDIT", if edit_rule.is_some() { "1" } else { "0" })
            .env("KAGARI_PROJECT_EDIT_RULE", edit_rule.unwrap_or_default())
            .env("KAGARI_CLI", cli)
            .env("KAGARI_NOTIFY", if cfg!(test) { "0" } else { "1" })
            .env("KAGARI_EXE", std::env::current_exe().unwrap_or_default())
            .env("KAGARI_HISTORY_KEY", &app.tasks[i].history_key)
            .env("KAGARI_DOCS_ID",if allow_docs {app.tasks[i].id.to_string()}else{String::new()})
            .env("KAGARI_COMPLETE_ID",if allow_completion {app.tasks[i].id.to_string()}else{String::new()})
            .env("KAGARI_EXPECTED_STATE",app.tasks[i].status.max(1).to_string())
            .env("KAGARI_TASK_TITLE", &app.tasks[i].title)
            .env("KAGARI_ICON", std::env::current_exe().unwrap_or_default().with_file_name("kagari.ico"))
            .env("KAGARI_RUN", &dir)
            .spawn();
        match result {
            Ok(child) => {
                let pid = child.id();
                let worker_id = process_creation(pid)
                    .map(|created| write("worker.id", &format!("{pid} {created}")));
                if draft.status == 0 {
                    draft.status = 1;
                    app.tasks[i].status = draft.status;
                    msg(h, 109, 0x14e, draft.status, 0);
                    app.save()?;
                }
                worker_id.transpose()?;
            }
            Err(e) => {
                write("error.txt", &e.to_string())?;
                return Err(e.to_string());
            }
        }
        self.update_send(h);
        self.show(h);
        Ok(())
    }
}
pub(super) fn efforts(agent: usize) -> &'static [&'static str] {
    if agent == 0 {
        &[DEFAULT, "low", "medium", "high", "xhigh", "max", "ultra"]
    } else {
        &[DEFAULT, "low", "medium", "high", "xhigh", "max"]
    }
}
pub(super) fn models(h: H, agent: usize) {
    options(h, 408, efforts(agent));
    if agent == 0 {
        options(
            h,
            402,
            &[DEFAULT, "gpt-6.2-sol", "gpt-6.2-astra", "gpt-6.1-terra"],
        )
    } else {
        options(h, 402, &[DEFAULT, "sonnet", "opus", "haiku"])
    }
}
pub fn open(owner: H, app: &mut App) -> Result<()> {
    editor::open_at(owner, app, false, true)
}
pub(super) const IMAGE_PASTE: u32 = 0x8000 + 111;
pub(super) const IMAGE_REMOVE: u32 = 0x8000 + 113;
type Subclass = extern "system" fn(H, u32, usize, isize, usize, usize) -> isize;
#[link(name = "comctl32")]
extern "system" {
    fn SetWindowSubclass(h: H, callback: Subclass, id: usize, data: usize) -> i32;
    fn DefSubclassProc(h: H, m: u32, w: usize, l: isize) -> isize;
}
#[link(name = "user32")]
extern "system" {
    fn IsClipboardFormatAvailable(format: u32) -> i32;
    fn RegisterClipboardFormatW(format: Ptr) -> u32;
    fn GetClipboardSequenceNumber() -> u32;
}
extern "system" fn request_edit(
    h: H,
    m: u32,
    w: usize,
    l: isize,
    _id: usize,
    owner: usize,
) -> isize {
    // Multiline Enter edits the draft. Ctrl+Enter invokes the same enabled send button.
    if m==0x102 && w==10 {
        #[link(name="user32")] extern "system"{fn GetKeyState(key:i32)->i16;fn IsWindowEnabled(h:H)->i32;}
        if unsafe{GetKeyState(0x11)}<0 {
            let button=unsafe{GetDlgItem(owner as H,404)};
            if unsafe{IsWindowEnabled(button)}!=0 {msg(owner as H,404,0xf5,0,0);}
            return 0;
        }
    }
    // Ctrl+V arrives as WM_CHAR; menu paste uses WM_PASTE.
    if m == 0x302 || (m == 0x102 && w == 22) {
        let png = unsafe { RegisterClipboardFormatW(wide("PNG").as_ptr()) };
        if [png, 2, 8, 17]
            .iter()
            .any(|&f| f != 0 && unsafe { IsClipboardFormatAvailable(f) } != 0)
        {
            return unsafe { SendMessageW(owner as H, IMAGE_PASTE, 0, 0) };
        }
    }
    unsafe { DefSubclassProc(h, m, w, l) }
}
impl Ask {
    pub(super) fn more_history(&mut self,h:H){
        if self.older.is_none(){self.queue_history(h,self.history_limit.max(5)+5);}
    }
    fn queue_history(&mut self,h:H,limit:usize){
        let Some(folder)=self.folder.clone() else{return};
        self.history_limit=limit;
        let (tx,rx)=std::sync::mpsc::channel();
        self.older=Some(rx);
        unsafe{EnableWindow(GetDlgItem(h,453),0);}
        std::thread::spawn(move||{
            let mut paths=recent_n(&folder,limit+1);
            let more=paths.len()>limit;
            if more{paths.remove(0);}
            let _=tx.send((render_transcript(paths,false),more));
        });
    }
    pub(super) fn bind(&mut self, h: H, task: &Task) {
        self.older=None; self.history_limit=5; self.last.clear();
        self.codex_session = (task.agent == 0).then(|| task.session.clone());
        self.track_submission(task);
        self.folder = if task.history_key.is_empty() { None } else { base().ok().map(|p|p.join(&task.history_key)) };
        models(h, task.agent);
        if let Some(index) = efforts(task.agent).iter().position(|&level| level == task.effort) {
            msg(h, 408, 0x14e, index, 0);
        }
        if !task.model.is_empty() { set(h, 402, &task.model); }
        self.show(h);
    }
    pub(super) fn init(&mut self, h: H, task: &Task) {
        self.images.clone_from(&task.draft_images);
        set(h, 403, &task.next_action);
        self.bind(h, task);
        unsafe { SetWindowSubclass(GetDlgItem(h, 403), request_edit, 1, h as usize); }
        msg(h, 403, 0xc5, 65535, 0);
        self.refresh_attachments(h);
        self.update_send(h);
    }
    pub(super) fn attachment_action(&mut self, h: H, action: u32, index: usize) -> Result<()> {
        match action {
            IMAGE_REMOVE if index < self.images.len() => {
                let removed = self.images.remove(index);
                self.picked.retain(|_, draft| draft != &removed);
                self.retired_images.push(removed);
            }
            IMAGE_PASTE => self.paste_image()?,
            411 => if let Some(path) = picker::image(h)? {
                self.add_image(path)?;
            },
            412 => self.clear_images(),
            _ => return Ok(()),
        }
        self.refresh_attachments(h);
        self.update_send(h);
        Ok(())
    }
}

#[cfg(test)]
mod image_tests {
    #[test]
    fn outcome_context_uses_this_tasks_successful_runs_across_sessions() {
        use super::*;
        let root=unique_dir(&std::env::temp_dir()).unwrap();
        for (folder,run,name,text) in [("1","11","reply.md","同じ集計です"),("1","12","reply.md","同じ集計です"),("2","13","reply.txt","Museが受領済み"),("1","14","error.txt","失敗した実行"),("3","15","reply.md","他のタスク")] {
            let dir=root.join(folder).join(run);fs::create_dir_all(&dir).unwrap();fs::write(dir.join(name),text).unwrap();
        }
        let task=Task{history_key:"1".into(),other_sessions:vec![TaskSession{history_key:"2".into(),..Default::default()}],..Default::default()};
        let text=reported_outcomes(&root,&task);
        assert_eq!(text.matches("同じ集計です").count(),1);
        assert!(text.find("受領済み").unwrap()<text.find("同じ集計です").unwrap());
        assert!(!text.contains("失敗した実行") && !text.contains("他のタスク"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn followup_requires_the_latest_successful_reply_and_no_running_request() {
        use super::*;
        let root = unique_dir(&std::env::temp_dir()).unwrap();
        let primary = root.join("1").join("10");
        let alternate = root.join("2").join("20");
        for run in [&primary, &alternate] {
            fs::create_dir_all(run).unwrap();
            fs::write(run.join("meta.txt"), "test").unwrap();
        }
        let mut task = Task { history_key: "1".into(), ..Default::default() };
        assert!(!awaiting_followup_in(&task, &root));
        fs::write(primary.join("reply.txt"), "answer").unwrap();
        assert!(awaiting_followup_in(&task, &root));
        task.other_sessions.push(TaskSession { history_key: "2".into(), ..Default::default() });
        assert!(!awaiting_followup_in(&task, &root));
        fs::write(alternate.join("error.txt"), "failed").unwrap();
        fs::write(alternate.join("reply.md"), "answer").unwrap();
        assert!(!awaiting_followup_in(&task, &root));
        fs::remove_file(alternate.join("error.txt")).unwrap();
        assert!(awaiting_followup_in(&task, &root));

        let running = root.join("1").join("15");
        fs::create_dir(&running).unwrap();
        fs::write(running.join("meta.txt"), "test").unwrap();
        let pid = std::process::id();
        fs::write(running.join("worker.id"), format!("{pid} {}", process_creation(pid).unwrap())).unwrap();
        assert!(!awaiting_followup_in(&task, &root));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn queue_and_summary_show_four_request_states() {
        use super::*;
        #[link(name="user32")]
        extern "system" {
            fn GetWindowLongW(h:H, index:i32)->i32;
            fn IsWindowEnabled(h:H)->i32;
        }
        thread_local! {static DISPLAY: RefCell<(String,Vec<String>)> = RefCell::default();}
        native::controls();
        unsafe {LoadLibraryW(wide("Msftedit.dll").as_ptr());}
        let folder = unique_dir(&std::env::temp_dir()).unwrap();
        let run = folder.join("1");
        fs::create_dir(&run).unwrap();
        fs::write(run.join("meta.txt"),"test").unwrap();
        let key = folder.file_name().unwrap().to_str().unwrap();
        let task = Task { title:"A1".into(), project:folder.display().to_string(),
            history_key:folder.display().to_string(), next_action:"test".into(),
            submitted_action_run:format!("{key}/1"), session:"12345678-0000-0000-0000-000000000001".into(), ..Default::default() };
        let mut app = App { projects:vec![task.project.clone()],tasks:vec![task],task:Some(0),..Default::default() };
        for (label,error,cause,buttons) in [
            ("セッション使用中","already has an active writer","書き込み中",vec!["分岐して再送"]),
            ("使用枠到達","You've hit your 5-hour limit · resets 4pm","resets 4pm",vec!["再送する"]),
            ("実行失敗","invalid model","invalid model",vec!["再送する"]),
            ("承認待ち","","承認が必要",vec!["今回だけ許可","拒否"]),
        ] {
            let event = if error.is_empty() {
                fs::remove_file(run.join("error.txt")).unwrap();
                let pid = std::process::id();
                fs::write(run.join("worker.id"),format!("{pid} {}",process_creation(pid).unwrap())).unwrap();
                let path = run.join("approval-00000000000000000000000000000000.pending");
                fs::write(&path,"test command").unwrap();
                path
            } else {
                let path = run.join("error.txt");
                fs::write(&path,error).unwrap();
                path
            };
            let at = fs::metadata(event).unwrap().modified().unwrap().duration_since(UNIX_EPOCH).unwrap();
            let timestamp = editor::local_datetime(at.as_millis()).unwrap();
            let (queue,detail) = queue_status(&app.tasks[0]);
            assert!(queue.ends_with(label) && detail.contains(&timestamp) && detail.contains(cause),"{queue} {detail}");
            TEST_ACTION.with(|cb| *cb.borrow_mut()=Some(|h| {
                let enabled = [442,443,445].into_iter().filter(|id|unsafe {
                    let control = GetDlgItem(h,*id);
                    GetWindowLongW(control,-16) & 0x10000000 != 0 && IsWindowEnabled(control)!=0
                }).map(|id|get(h,id)).collect();
                DISPLAY.with(|v|*v.borrow_mut()=(get(h,433),enabled));
                unsafe {EndDialog(h,0);}
            }));
            open(0,&mut app).unwrap();
            DISPLAY.with(|v| {
                let (caption,enabled) = &*v.borrow();
                assert_eq!(caption,&format!("{label}　{detail}"));
                for button in buttons {assert!(detail.contains(button) && enabled.iter().any(|b|b==button),"{caption} {enabled:?}");}
            });
        }
        fs::remove_dir_all(folder).unwrap();
    }
    use super::*;

    #[test]
    fn codex_worker_follows_persisted_branches_after_each_conflict() {
        let folder = unique_dir(&std::env::temp_dir()).unwrap();
        let original = "10000000-0000-0000-0000-000000000001";
        let first = "20000000-0000-0000-0000-000000000002";
        let second = "30000000-0000-0000-0000-000000000003";
        let cli = folder.join("codex.cmd");
        fs::write(&cli, concat!("@echo off\r\n",
            "echo %* > \"%KAGARI_RUN%\\argv.txt\"\r\n",
            "if \"%FIXTURE_EVENT%\"==\"conflict\" (echo already has an active writer 1>&2 & exit /b 1)\r\n",
            "echo {\"type\":\"thread.started\",\"thread_id\":\"%FIXTURE_EVENT%\"}\r\n",
            "echo done > \"%KAGARI_RUN%\\last.tmp\"\r\nexit /b 0\r\n")).unwrap();
        for (operation, target, event) in [
            ("resume", original, "conflict"), ("fork", original, first),
            ("resume", first, first), ("resume", first, "conflict"),
            ("fork", first, second), ("resume", second, second),
        ] {
            // Reconstruct from disk as a freshly opened task would.
            let route = CodexRoute::load(Some(&folder), original);
            let fork = route.needs_fork(recent(&folder).last().map(PathBuf::as_path));
            let dir = unique_dir(&folder).unwrap();
            for (file, body) in [("meta.txt", "Codex"), ("prompt.txt","test"),
                ("codex-target.txt", &route.target), ("queue-result.ps1","")] {
                fs::write(dir.join(file), body).unwrap();
            }
            let mut worker = Command::new("powershell.exe");
            route.configure(&mut worker, fork);
            let output = worker.args(["-NoProfile","-NonInteractive","-Command",WORKER])
                .env("KAGARI_RUN",&dir).env("KAGARI_AGENT","codex").env("KAGARI_CLI",&cli)
                .env("KAGARI_PROJECT_ALIAS","").env("KAGARI_NOTIFY","0").env("FIXTURE_EVENT",event)
                .output().unwrap();
            assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
            let args = fs::read_to_string(dir.join("argv.txt")).unwrap();
            assert!(args.split_whitespace().any(|arg| arg == operation), "{args}");
            assert!(args.contains(&format!("{target} --json -")), "{args}");
            if event == "conflict" {
                assert!(writer_conflict(&snippet(dir.join("error.txt"), 1600)));
            } else {
                assert!(dir.join("reply.md").exists(), "{}", snippet(dir.join("error.txt"), 500));
            }
            if operation == "fork" {
                let persisted = CodexRoute::load(Some(&folder), original);
                assert_eq!(persisted.parent.as_deref(), Some(target));
                assert_eq!(persisted.target, event);
                assert!(transcript(&folder, false).contains(&format!("分岐元：{target}\n送信先：{event}")));
            }
        }
        let legacy = folder.join(format!("codex-branch-{original}.txt"));
        fs::write(legacy, first).unwrap();
        assert_eq!(CodexRoute::load(Some(&folder), original).target, first);
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn progress_context_uses_project_alias_and_scoped_counts() {
        let task=Task{project:r"D:\.company\dots\game_creator\TurfWars".into(),title:"進捗を確認".into(),status:1,..Default::default()};
        let other=Task{project:r"D:\.company\dots\mebae-c".into(),title:"別件".into(),..Default::default()};
        let done=Task{title:"検証".into(),status:3,..task.clone()};
        let text=project_context(&[task.clone(),other,done],&task,"状況");
        assert!(text.contains("対象プロジェクト: chromarch"));
        assert!(text.contains("合計2件、未着手0・次の作業依頼待ち0・作業中1・確認待ち0・完了1"));
        assert!(!text.contains("別件"));
    }
    #[test]
    fn company_scope_uses_only_the_working_directory_ancestor() {
        assert_eq!(company_read_root(Path::new(r"\\?\D:\.company\eda")), Some(r"D:\.company".into()));
        assert_eq!(company_read_root(Path::new(r"\\?\UNC\server\share\.company\eda\docs")), Some(r"\\server\share\.company".into()));
        assert_eq!(company_read_root(Path::new(r"D:\other\eda")), None);
    }

    #[test]
    fn pending_run_requires_the_original_live_process_and_no_result() {
        let dir = unique_dir(&std::env::temp_dir()).unwrap();
        let pid = std::process::id();
        let created = process_creation(pid).unwrap();
        let marker = dir.join("worker.id");
        fs::write(&marker, format!("{pid} {created}")).unwrap();
        assert!(running(&dir));
        fs::write(dir.join("last-tool.txt"), "テスト成功").unwrap();
        fs::write(dir.join("activity.txt"), "event").unwrap();
        fs::File::options().write(true).open(dir.join("activity.txt")).unwrap()
            .set_modified(SystemTime::now() - std::time::Duration::from_secs(7200)).unwrap();
        let pending = dir.join("approval-00000000000000000000000000000000.pending");
        let state = || request_status(&dir, running(&dir), Approval::pending(&dir).as_ref());
        assert_eq!(state().label, "実行中");
        assert!(state().detail.contains("テスト成功"));
        fs::write(&pending, "承認要求").unwrap();
        assert_eq!(state().label, "承認待ち");
        fs::write(&marker, format!("{pid} {}", created + 1)).unwrap();
        assert!(!running(&dir));
        assert_eq!(state().label, "中断");
        assert!(state().detail.contains("テスト成功"));
        fs::write(&marker, format!("{pid} {created}")).unwrap();
        fs::write(dir.join("reply.md"), "done").unwrap();
        assert!(!running(&dir));
        assert_eq!(state().label, "回答受信");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn interrupted_history_keeps_the_request_input_and_partial_output_together() {
        let folder = unique_dir(&std::env::temp_dir()).unwrap();
        let dir = unique_dir(&folder).unwrap();
        for (name, value) in [
            ("meta.txt", "Codex"), ("request.md", "集計を確認"), ("prompt.txt", "送信した入力"),
            ("partial.txt", "11件を集計"), ("worker.id", "0 1"),
        ] {
            fs::write(dir.join(name), value).unwrap();
        }
        let history = transcript(&folder, false);
        for expected in ["中断", "集計を確認", "11件を集計", "prompt.txt", "partial.txt"] {
            assert!(history.contains(expected));
        }
        assert!(transcript(&folder, true).is_empty());
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn markdown_keeps_image_after_original_moves() {
        let dir = unique_dir(&std::env::temp_dir()).unwrap();
        let source = dir.join("添付 画像.png");
        let bytes = b"\x89PNG\r\n\x1a\nfixture";
        fs::write(&source, bytes).unwrap();
        let run = unique_dir(&dir).unwrap();
        save_request(&run, "## 確認", &[source.clone()]).unwrap();
        fs::remove_file(source).unwrap();
        assert_eq!(fs::read(run.join("images/0001.png")).unwrap(), bytes);
        assert_eq!(
            fs::read_to_string(message_path(&run, "request")).unwrap(),
            "## 確認\n\n![添付画像 1](images/0001.png)"
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
