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
function recordActivity($stage) {
 [IO.File]::WriteAllText((Join-Path $dir 'activity.txt'),$stage,$utf)
}
"#, include_str!("subagent_status.ps1"), r#"
try {
 $agent=$env:KAGARI_AGENT
 $cli=$env:KAGARI_CLI
 $flags=@()
 if($env:KAGARI_MODEL){$flags+=@('--model',$env:KAGARI_MODEL)}
 $prompt=[IO.File]::ReadAllText((Join-Path $dir 'prompt.txt'))
 $assets=Join-Path $dir 'images'
 $images=@(if(Test-Path -LiteralPath $assets){Get-ChildItem -LiteralPath $assets -File | Sort-Object Name})
 $diag=Join-Path $dir 'diagnostic.tmp'
 $ErrorActionPreference='Continue'
 if($agent -eq 'codex'){
 $last=Join-Path $dir 'last.tmp'
 if($env:KAGARI_EFFORT){$flags+=@('-c',('model_reasoning_effort='+$env:KAGARI_EFFORT))}
 $argv=@('exec')
 if($env:KAGARI_EDIT -eq '1'){$argv+=@('--sandbox','workspace-write')}
 if($env:KAGARI_SESSION){$argv+='resume'}
 foreach($img in $images){$flags+=@('--image',$img.FullName)}
 $argv+=$flags
 $argv+=@('--skip-git-repo-check','-o',$last)
 if($env:KAGARI_SESSION){$argv+=$env:KAGARI_SESSION}
 $argv+=@('--json','-')
 $prompt | & $cli @argv 2>$diag | ForEach-Object {
  $event=$_ | ConvertFrom-Json -ErrorAction SilentlyContinue
  try {Update-ChildStatus $event}catch{}
  if($event.type -eq 'item.started'){recordActivity 'item'}
  elseif($event.type -eq 'item.completed'){recordActivity 'item_done'}
  elseif($event.type){recordActivity 'event'}
  if($event.type -eq 'error'){$reply=$event.message}
 }
 $code=$LASTEXITCODE
 if($code -eq 0){$reply=[IO.File]::ReadAllText($last)}
 }else{
 if($env:KAGARI_EFFORT){$flags+=@('--effort',$env:KAGARI_EFFORT)}
 if($env:KAGARI_SESSION){$flags+=@('--resume',$env:KAGARI_SESSION)}
 if($env:KAGARI_COMPANY_ROOT){
 $flags+=@('--add-dir',$env:KAGARI_COMPANY_ROOT)
 $flags+=@('--append-system-prompt',('会社全体の進捗を確認する際は、追加の作業ディレクトリ '+$env:KAGARI_COMPANY_ROOT+' 配下を Read、Glob、Grep で参照できます。書き込み対象のプロジェクトは '+$env:KAGARI_PROJECT+'。それ以外のプロジェクトは参照用です。'))
 }
 if($env:KAGARI_PROJECT_EDIT_RULE){$flags+=@('--allowedTools',$env:KAGARI_PROJECT_EDIT_RULE)}
 if($images.Count){
 $mime=@{'.png'='image/png';'.jpg'='image/jpeg';'.gif'='image/gif';'.webp'='image/webp'}
 $parts=@(@{type='text';text=$prompt})
 foreach($img in $images){
 $parts+=@{type='image';source=@{type='base64';media_type=$mime[$img.Extension];data=[Convert]::ToBase64String([IO.File]::ReadAllBytes($img.FullName))}}
 }
 $packet=@{type='user';message=@{role='user';content=$parts}} | ConvertTo-Json -Compress -Depth 8
 $inputData=$packet
 $flags+=@('--input-format','stream-json')
 }else{
 $inputData=$prompt
 }
 if((& $cli --help 2>$null | Select-String -SimpleMatch '--forward-subagent-text' -Quiet)){$flags+='--forward-subagent-text'}
 $result=$null
 $inputData | & $cli -p @flags --output-format stream-json --verbose 2>$diag | ForEach-Object {
 $line=$_
 $event=$line | ConvertFrom-Json -ErrorAction SilentlyContinue
 try {Update-ChildStatus $event}catch{}
 if($event.type -eq 'result'){$result=$event}
 elseif($event.type -eq 'assistant' -and $event.message.content.type -contains 'tool_use'){recordActivity 'tool'}
 elseif($event.type -eq 'user' -and $event.message.content.type -contains 'tool_result'){recordActivity 'tool_done'}
 elseif($event.type){recordActivity 'event'}
 elseif($line){$reply=$line}
 }
 $code=$LASTEXITCODE
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
 if([string]::IsNullOrWhiteSpace($reply)){throw 'AIの回答を受け取れませんでした。'}
 $file='reply.md'
}catch{$reply=$_.Exception.Message}
$tmp=Join-Path $dir 'result.tmp'
[IO.File]::WriteAllText($tmp,$reply,$utf)
[IO.File]::Move($tmp,(Join-Path $dir $file))
foreach($leaf in @('diagnostic.tmp','last.tmp','prompt.txt')){[IO.File]::Delete((Join-Path $dir $leaf))}
if($env:KAGARI_NOTIFY -eq '1'){
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
  $xml.DocumentElement.SetAttribute('launch',('kagari-task://open/'+$env:KAGARI_HISTORY_KEY))
  $lines=$xml.GetElementsByTagName('text')
  $title=if($file -eq 'reply.md'){'AIの回答が届きました'}else{'AIへの依頼がエラーで終了しました'}
  $null=$lines.Item(0).AppendChild($xml.CreateTextNode($title))
  $null=$lines.Item(1).AppendChild($xml.CreateTextNode($env:KAGARI_TASK_TITLE))
  $toast=[Windows.UI.Notifications.ToastNotification]::new($xml)
  [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier($appId).Show($toast)
 }catch{
  [IO.File]::WriteAllText((Join-Path $dir 'notification-error.txt'),$_.Exception.Message,$utf)
 }
}
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
fn recent(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut paths: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("meta.txt").is_file())
        .collect();
    paths.sort_unstable_by(|a, b| b.cmp(a));
    paths.truncate(5);
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
#[link(name="kernel32")]
extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> H;
    fn GetProcessTimes(process: H, created: *mut u64, ended: *mut u64, kernel: *mut u64, user: *mut u64) -> i32;
    fn CloseHandle(handle: H) -> i32;
}
fn process_creation(pid: u32) -> Option<u64> {
    unsafe {
        let handle = OpenProcess(0x1000, 0, pid);
        if handle == 0 { return None; }
        let (mut created, mut ended, mut kernel, mut user) = (0, 0, 0, 0);
        let ok = GetProcessTimes(handle, &mut created, &mut ended, &mut kernel, &mut user);
        CloseHandle(handle);
        (ok != 0 && ended == 0).then_some(created)
    }
}
fn running(dir: &Path) -> bool {
    if message_path(dir, "reply").is_file() || dir.join("error.txt").is_file() { return false; }
    let Ok(marker) = fs::read_to_string(dir.join("worker.id")) else { return false; };
    let Some((pid, created)) = marker.split_once(' ') else { return false; };
    pid.parse().ok().and_then(process_creation).is_some_and(|time| Some(time) == created.parse().ok())
}
fn request_status(dir: &Path) -> String {
    let start = fs::metadata(dir.join("meta.txt")).and_then(|m| m.modified()).ok();
    let duration = |end: SystemTime| start.and_then(|t| end.duration_since(t).ok())
        .map(|t| format!("　経過 {}分{:02}秒", t.as_secs()/60, t.as_secs()%60)).unwrap_or_default();
    for (file, label) in [(message_path(dir,"reply"), "回答受信"), (dir.join("error.txt"), "実行エラー（設定・履歴で詳細を確認）")] {
        if let Ok(meta) = fs::metadata(file) {
            return format!("{label}{}",meta.modified().ok().map(duration).unwrap_or_default());
        }
    }
    if !running(dir) {
        return if dir.join("worker.id").is_file() { "中断（実行プロセス終了・回答未受信）" } else { "状態不明（実行記録なし・回答未受信）" }.into();
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
    format!("実行中{}　最終状況：{stage}{age}", duration(now))
}
fn transcript(folder: &Path, completed_only: bool) -> String {
    let mut blocks = Vec::new();
    for p in recent(folder) {
        let reply = snippet(message_path(&p, "reply"), 650);
        if reply.is_empty() && completed_only {
            continue;
        }
        let meta = snippet(p.join("meta.txt"), 140);
        let request = snippet(message_path(&p, "request"), 360);
        let result = if reply.is_empty() {
            let err = snippet(p.join("error.txt"), 500);
            if err.is_empty() {
                if running(&p) { "実行中（この画面を閉じても継続します）".into() }
                else if p.join("worker.id").is_file() { "実行が終了しましたが、回答を受信できませんでした。再実行できます。".into() }
                else { "回答未受信（実行状態不明）".into() }
            } else {
                format!("実行エラー　{err}")
            }
        } else {
            format!("AI　{reply}")
        };
        blocks.push(format!("{meta}\nあなた　{request}\n{result}"));
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

fn prompt(t: &Task, request: &str, previous: &str) -> String {
    format!("回答はMarkdown形式で記述してください。利用者の判断が必要な場合は、ID・決めること・選択肢の列を持つMarkdown表に項目をまとめ、各候補を同じセル内で<br>で分けてください。自由入力だけの項目は選択肢を空欄にしてください。\n\nタスク: {}\n仕様書: {}\n作業ログ: {}\n結果: {}\n\nこれまでのやり取り（参考）:\n{}\n\n今回の依頼:\n{}",t.title,t.docs[0],t.docs[1],t.docs[2],previous,request)
}
#[derive(Default)]
pub(super) struct Ask {
    folder: Option<PathBuf>,
    last: String,
    images: Vec<PathBuf>,
    paste_dir: Option<PathBuf>,
    running: bool,
}
impl Ask {
    pub(super) fn clear_images(&mut self) {
        self.images.clear();
        if let Some(dir) = self.paste_dir.take() {
            let _ = fs::remove_dir_all(dir);
        }
    }
    fn paste_image(&mut self) -> Result<()> {
        let sequence = unsafe { GetClipboardSequenceNumber() };
        if self.paste_dir.is_none() {
            self.paste_dir = Some(unique_dir(&std::env::temp_dir().join("KagariClipboard"))?);
        }
        let dir = self.paste_dir.as_ref().unwrap();
        let path = dir.join(format!("貼り付け-{:03}.png", self.images.len() + 1));
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
        self.images.push(path);
        Ok(())
    }
    fn attachments(&self, h: H) {
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
    pub(super) fn update_send(&self,h:H){
        let ready=!self.running && (!get(h,403).trim().is_empty() || !self.images.is_empty());
        unsafe{EnableWindow(GetDlgItem(h,404),ready as i32);}
    }
    fn busy(&self) -> bool {
        self.folder.as_ref().is_some_and(|p| recent(p).iter().any(|dir| running(dir)))
    }
    pub(super) fn show(&mut self, h: H) -> bool {
        let runs = self.folder.as_ref().map(|p| recent(p)).unwrap_or_default();
        let active = runs.iter().rev().find(|p| running(p));
        let busy = active.is_some();
        let status = active.or(runs.last()).map(|p| request_status(p)).unwrap_or_default();
        let status=if status.is_empty() {"依頼待ち".to_owned()}else{status};
        if get(h,433)!=status{set(h,433,&status);}
        self.running=busy;
        let children = active.and_then(|p| fs::read_to_string(p.join("subagents.txt")).ok()).unwrap_or_default();
        if get(h,434) != children { set(h,434,&children); }
        unsafe { ShowWindow(GetDlgItem(h,434), if children.is_empty() {0} else {5}); }
        self.update_send(h);
        set(h, 404, if busy {"実行中…"} else if status.starts_with("実行エラー") || status.starts_with("中断") {"再送する"} else {"依頼を送信"});
        let value = self
            .folder
            .as_ref()
            .map(|p| transcript(p, false))
            .unwrap_or_default();
        let view = if value.is_empty() {
            "このタスクの履歴はまだありません".into()
        } else {
            value.replace("\r\n", "\n").replace('\n', "\r\n")
        };
        if self.last != view {
            set(h, 405, &view);
            msg(h, 405, 0xb1, usize::MAX, -1);
            msg(h, 405, 0xb7, 0, 0);
            self.last = view;
            return true;
        }
        false
    }
    pub(super) fn execute(&mut self, h: H, app: &mut App, index: &mut Option<usize>, draft: &mut Task) -> Result<()> {
        if self.busy() { return Ok(()); }
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
        let request = get(h, 403);
        if request.trim().is_empty() && self.images.is_empty() {
            return Err("依頼内容を入力してください".into());
        }
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
        let body = prompt(t, &request, &prev);
        let dir = unique_dir(&folder)?;
        t.submitted_action_run = format!("{}/{}", t.history_key, dir.file_name().unwrap().to_string_lossy());
        let mut now = [0u16; 8];
        unsafe { GetLocalTime(&mut now) };
        let meta = format!(
            "{:04}/{:02}/{:02} {:02}:{:02}　{} / {}",
            now[0], now[1], now[3], now[4], now[5], AGENTS[a], model
        );
        let write =
            |name: &str, value: &str| fs::write(dir.join(name), value).map_err(|e| e.to_string());
        let mut prepare = || -> Result<()> {
            save_request(&dir, &request, &self.images)?;
            write("prompt.txt", &body)?;
            write("meta.txt", &meta)?;
            next.save()
        };
        if let Err(e) = prepare() {
            let _ = fs::remove_dir_all(&dir);
            return Err(e);
        }
        draft.history_key = next.tasks[i].history_key.clone();
        draft.submitted_action_run.clone_from(&next.tasks[i].submitted_action_run);
        *app = next;
        *index = Some(i);
        self.folder = Some(folder);
        let shell =
            PathBuf::from(std::env::var_os("SystemRoot").ok_or("Windows の場所が不明です")?)
                .join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
        let project = Path::new(&app.tasks[i].project);
        let original_cwd = Path::new(app.tasks[i].working_dir());
        let cwd = if a == 0 && !original_cwd.starts_with(project) { project } else { original_cwd };
        let edit_rule = project_edit_rule(project, cwd);
        let result = Command::new(shell)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .current_dir(cwd)
            .creation_flags(0x08000000)
            .args(["-NoProfile", "-NonInteractive", "-Command", WORKER])
            .env("KAGARI_AGENT", if a == 0 { "codex" } else { "claude" })
            .env("KAGARI_MODEL", &app.tasks[i].model)
            .env("KAGARI_EFFORT", &app.tasks[i].effort)
            .env("KAGARI_COMPANY_ROOT", company_read_root(Path::new(app.tasks[i].working_dir())).unwrap_or_default())
            .env("KAGARI_SESSION", &app.tasks[i].session)
            .env("KAGARI_PROJECT", project)
            .env("KAGARI_EDIT", if edit_rule.is_some() { "1" } else { "0" })
            .env("KAGARI_PROJECT_EDIT_RULE", edit_rule.unwrap_or_default())
            .env("KAGARI_CLI", cli)
            .env("KAGARI_NOTIFY", if cfg!(test) { "0" } else { "1" })
            .env("KAGARI_EXE", std::env::current_exe().unwrap_or_default())
            .env("KAGARI_HISTORY_KEY", &app.tasks[i].history_key)
            .env("KAGARI_TASK_TITLE", &app.tasks[i].title)
            .env("KAGARI_ICON", std::env::current_exe().unwrap_or_default().with_file_name("kagari.ico"))
            .env("KAGARI_RUN", &dir)
            .spawn();
        match result {
            Ok(child) => {
                let pid = child.id();
                if let Some(created) = process_creation(pid) {
                    write("worker.id", &format!("{pid} {created}"))?;
                }
            }
            Err(e) => {
                write("error.txt", &e.to_string())?;
                return Err(e.to_string());
            }
        }
        self.clear_images();
        self.attachments(h);
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
    pub(super) fn bind(&mut self, h: H, task: &Task) {
        self.folder = if task.history_key.is_empty() { None } else { base().ok().map(|p|p.join(&task.history_key)) };
        models(h, task.agent);
        if let Some(index) = efforts(task.agent).iter().position(|&level| level == task.effort) {
            msg(h, 408, 0x14e, index, 0);
        }
        if !task.model.is_empty() { set(h, 402, &task.model); }
        unsafe { EnableWindow(GetDlgItem(h, 108), task.session.is_empty() as i32); }
        self.show(h);
    }
    pub(super) fn init(&mut self, h: H, task: &Task) {
        self.bind(h, task);
        set(h, 403, &task.next_action);
        unsafe { SetWindowSubclass(GetDlgItem(h, 403), request_edit, 1, h as usize); }
        msg(h, 403, 0xc5, 65535, 0);
        self.attachments(h);
        self.update_send(h);
    }
    pub(super) fn attachment_action(&mut self, h: H, action: u32) -> Result<()> {
        match action {
            IMAGE_PASTE => self.paste_image()?,
            411 => if let Some(path) = picker::image(h)? {
                if !self.images.contains(&path) { self.images.push(path); }
            },
            412 => self.clear_images(),
            _ => return Ok(()),
        }
        self.attachments(h);
        self.update_send(h);
        Ok(())
    }
}

#[cfg(test)]
mod image_tests {
    use super::*;

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
        fs::write(&marker, format!("{pid} {}", created + 1)).unwrap();
        assert!(!running(&dir));
        fs::write(&marker, format!("{pid} {created}")).unwrap();
        fs::write(dir.join("reply.md"), "done").unwrap();
        assert!(!running(&dir));
        fs::remove_dir_all(dir).unwrap();
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
