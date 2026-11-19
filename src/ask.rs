use super::*;
use std::time::{SystemTime, UNIX_EPOCH};
const DEFAULT: &str = "CLIの既定";
#[cfg(test)]
thread_local! {pub(super) static TEST_ACTION:RefCell<Option<fn(H)>>=const{RefCell::new(None)};}
const WORKER: &str = r#"
$ErrorActionPreference='Stop'
$dir=$env:KAGARI_RUN
$utf=[Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding=$utf
$OutputEncoding=$utf
$file='error.txt'
try {
 $agent=$env:KAGARI_AGENT
 $cli=$env:KAGARI_CLI
 $flags=@()
 if($env:KAGARI_MODEL){$flags+=@('--model',$env:KAGARI_MODEL)}
 $prompt=[IO.File]::ReadAllText((Join-Path $dir 'prompt.txt'))
 $diag=Join-Path $dir 'diagnostic.tmp'
 $ErrorActionPreference='Continue'
 if($agent -eq 'codex'){
 $last=Join-Path $dir 'last.tmp'
 $argv=@('exec')
 if($env:KAGARI_EDIT -eq '1'){$argv+=@('--sandbox','workspace-write')}
 if($env:KAGARI_SESSION){$argv+='resume'}
 $argv+=$flags
 $argv+=@('--skip-git-repo-check','-o',$last)
 if($env:KAGARI_SESSION){$argv+=$env:KAGARI_SESSION}
 $argv+='-'
 $prompt | & $cli @argv 2>$diag | Out-Null
 $code=$LASTEXITCODE
 if($code -eq 0){$reply=[IO.File]::ReadAllText($last)}
 }else{
 if($env:KAGARI_SESSION){$flags+=@('--resume',$env:KAGARI_SESSION)}
 if($env:KAGARI_EDIT -eq '1'){$flags+=@('--allowedTools','Edit,Write')}
 $reply=($prompt | & $cli -p @flags --output-format text 2>$diag | Out-String)
 $code=$LASTEXITCODE
 }
 $ErrorActionPreference='Stop'
 if($code -ne 0){$detail=Get-Content -LiteralPath $diag -Tail 4 | Out-String;throw ('CLI exit '+$code+' '+$detail)}
 if([string]::IsNullOrWhiteSpace($reply)){throw 'AIの回答を受け取れませんでした。'}
 $file='reply.txt'
}catch{$reply=$_.Exception.Message}
$tmp=Join-Path $dir 'result.tmp'
[IO.File]::WriteAllText($tmp,$reply.Trim(),$utf)
[IO.File]::Move($tmp,(Join-Path $dir $file))
foreach($leaf in @('diagnostic.tmp','last.tmp','prompt.txt')){[IO.File]::Delete((Join-Path $dir $leaf))}
"#;
#[link(name = "user32")]
extern "system" {
    fn GetWindowLongPtrW(h: H, i: i32) -> isize;
    fn SetWindowLongPtrW(h: H, i: i32, v: isize) -> isize;
    fn SetTimer(h: H, i: usize, ms: u32, cb: usize) -> usize;
    fn KillTimer(h: H, i: usize) -> i32;
}
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
fn transcript(folder: &Path, completed_only: bool) -> String {
    let mut blocks = Vec::new();
    for p in recent(folder) {
        let reply = snippet(p.join("reply.txt"), 650);
        if reply.is_empty() && completed_only {
            continue;
        }
        let meta = snippet(p.join("meta.txt"), 140);
        let request = snippet(p.join("request.txt"), 360);
        let result = if reply.is_empty() {
            let err = snippet(p.join("error.txt"), 500);
            if err.is_empty() {
                "応答待ち（中断時は未受信）".into()
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
fn prompt(t: &Task, request: &str, previous: &str) -> String {
    format!("タスク: {}\n仕様書: {}\n作業ログ: {}\n結果: {}\n\nこれまでのやり取り（参考）:\n{}\n\n今回の依頼:\n{}",t.title,t.docs[0],t.docs[1],t.docs[2],previous,request)
}
struct Ask<'a> {
    app: &'a mut App,
    i: usize,
    folder: Option<PathBuf>,
    last: String,
}
impl Ask<'_> {
    fn show(&mut self, h: H) {
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
        }
    }
    fn execute(&mut self, h: H) -> Result<()> {
        let a = choice(h, 401);
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
        let request = get(h, 403);
        let request = request.trim();
        if request.is_empty() {
            return Err("依頼内容を入力してください".into());
        }
        let cli = client::find(a)?;
        let root = base()?;
        let folder = match &self.folder {
            Some(p) => p.clone(),
            None => unique_dir(&root)?,
        };
        let mut next = self.app.clone();
        let t = &mut next.tasks[self.i];
        t.history_key = folder.file_name().unwrap().to_string_lossy().into();
        t.agent = a;
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
        let body = prompt(t, request, &prev);
        let dir = unique_dir(&folder)?;
        let mut now = [0u16; 8];
        unsafe { GetLocalTime(&mut now) };
        let meta = format!(
            "{:04}/{:02}/{:02} {:02}:{:02}　{} / {}",
            now[0], now[1], now[3], now[4], now[5], AGENTS[a], model
        );
        let write =
            |name: &str, value: &str| fs::write(dir.join(name), value).map_err(|e| e.to_string());
        write("request.txt", request)?;
        write("prompt.txt", &body)?;
        next.save()?;
        *self.app = next;
        self.folder = Some(folder);
        write("meta.txt", &meta)?;
        let shell =
            PathBuf::from(std::env::var_os("SystemRoot").ok_or("Windows の場所が不明です")?)
                .join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
        let result = Command::new(shell)
            .current_dir(&self.app.tasks[self.i].project)
            .creation_flags(0x08000000)
            .args(["-NoProfile", "-NonInteractive", "-Command", WORKER])
            .env("KAGARI_AGENT", if a == 0 { "codex" } else { "claude" })
            .env("KAGARI_MODEL", &self.app.tasks[self.i].model)
            .env("KAGARI_SESSION", &self.app.tasks[self.i].session)
            .env(
                "KAGARI_EDIT",
                if sessions::own_source(Path::new(&self.app.tasks[self.i].project)) {
                    "1"
                } else {
                    "0"
                },
            )
            .env("KAGARI_CLI", cli)
            .env("KAGARI_RUN", &dir)
            .spawn();
        if let Err(e) = result {
            write("error.txt", &e.to_string())?;
            return Err(e.to_string());
        }
        set(h, 403, "");
        self.show(h);
        Ok(())
    }
}
fn models(h: H, agent: usize) {
    if agent == 0 {
        options(h, 402, &[DEFAULT, "gpt-6-sol", "gpt-6-luna"])
    } else {
        options(h, 402, &[DEFAULT, "sonnet", "opus", "haiku"])
    }
}
pub fn open(owner: H, app: &mut App) -> Result<()> {
    let i = app.task.ok_or("タスクを選んでください")?;
    let key = &app.tasks[i].history_key;
    let folder = if key.is_empty() {
        None
    } else {
        Some(base()?.join(key))
    };
    let mut state = Ask {
        app,
        i,
        folder,
        last: String::new(),
    };
    let n = unsafe {
        DialogBoxParamW(
            GetModuleHandleW(std::ptr::null()),
            400 as Ptr,
            owner,
            proc,
            &mut state as *mut _ as _,
        )
    };
    if n < 0 {
        return Err("依頼画面を開けません".into());
    }
    Ok(())
}
extern "system" fn proc(h: H, m: u32, w: usize, l: isize) -> isize {
    if m == 0x2b && w == 401 {
        brand::draw(l);
        return 1;
    }
    if m == 0x110 {
        unsafe {
            SetWindowLongPtrW(h, -21, l);
        }
        let s = unsafe { &mut *(l as *mut Ask) };
        let t = &s.app.tasks[s.i];
        set(h, 407, &t.title);
        options(h, 401, &AGENTS);
        msg(h, 401, 0x14e, t.agent, 0);
        models(h, t.agent);
        if !t.model.is_empty() {
            set(h, 402, &t.model)
        }
        if !t.session.is_empty() {
            unsafe {
                EnableWindow(GetDlgItem(h, 401), 0);
            }
            set(h, 407, &format!("{}　｜　元のセッションを再開", t.title));
            set(h, 403, "前回の続きから進めてください。");
        } else {
            set(h, 403, &t.title);
        }
        msg(h, 403, 0xc5, 4096, 0);
        s.show(h);
        unsafe {
            SetTimer(h, 1, 2000, 0);
        }
        #[cfg(test)]
        {
            let cb = TEST_ACTION.with(|c| c.borrow_mut().take());
            if let Some(cb) = cb {
                cb(h);
            }
        }
        return 1;
    }
    if m == 0x10 || m == 0x111 && w & 65535 == 2 {
        unsafe {
            KillTimer(h, 1);
            EndDialog(h, 0);
        }
        return 1;
    }
    if m == 0x113 || m == 0x111 && w & 65535 == 404 && w >> 16 == 0 {
        let ptr = unsafe { GetWindowLongPtrW(h, -21) };
        if ptr != 0 {
            let s = unsafe { &mut *(ptr as *mut Ask) };
            if m == 0x113 {
                s.show(h)
            } else if let Err(e) = s.execute(h) {
                notice(h, &e)
            }
        }
        return 1;
    }
    if m == 0x111 && w & 65535 == 401 && w >> 16 == 1 {
        models(h, choice(h, 401));
        msg(h, 406, 0xf1, 0, 0);
        return 1;
    }
    0
}
