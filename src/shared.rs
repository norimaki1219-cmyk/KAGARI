use super::*;
use std::{
    io::Write,
    process::Stdio,
    sync::mpsc::{self, Receiver, TryRecvError},
};
thread_local! {
 static JOB:RefCell<Option<Receiver<Result<String>>>>=const{RefCell::new(None)};
 static LAST:RefCell<String>=const{RefCell::new(String::new())};
 static KEY:RefCell<String>=const{RefCell::new(String::new())};
}
#[link(name = "user32")]
extern "system" {
    fn SetTimer(h: H, id: usize, ms: u32, cb: usize) -> usize;
    fn KillTimer(h: H, id: usize) -> i32;
}
const FETCH: &str = r#"
$ErrorActionPreference='Stop'
[Console]::InputEncoding=[Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$key=[Console]::ReadLine()
$headers=@{Authorization=('Bearer '+$key)}
try {
 $base='https://progress.eda-edamap.workers.dev/api/'
 $goals=Invoke-RestMethod ($base+'goals') -Headers $headers -TimeoutSec 15 -MaximumRedirection 0
 $projects=Invoke-RestMethod ($base+'projects') -Headers $headers -TimeoutSec 15 -MaximumRedirection 0
 if($null -eq $goals.goals -or $null -eq $projects.projects){exit 2}
 '共有目標　'+$goals.goals.Count+' 件　｜　取得 '+(Get-Date -Format 'yyyy-MM-dd HH:mm')
 foreach($g in $goals.goals){'';'■ '+$g.project+'　/　'+$g.title;('    {0}%　{1}　期限 {2}' -f $g.progress.percent,$g.progress.label,$g.due_date)}
 ''; '進行状況　'+$projects.projects.Count+' 件'
 foreach($p in $projects.projects){'';'■ '+$p.name+'　/　'+$p.agent+'　'+$p.status;$p.current_action;if($p.issue){'課題　'+$p.issue};if($p.next_step){'次　'+$p.next_step}}
}catch{if($_.Exception.Response.StatusCode.value__ -in 401,403){exit 3};exit 2}
"#;
fn fetch(key: String) -> Result<String> {
    let shell = PathBuf::from(std::env::var_os("SystemRoot").ok_or("Windows の場所が不明です")?)
        .join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
    let mut child = Command::new(shell)
        .creation_flags(0x08000000)
        .args(["-NoProfile", "-NonInteractive", "-Command", FETCH])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    {
        let mut input = child.stdin.take().unwrap();
        writeln!(input, "{key}").map_err(|e| e.to_string())?;
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(if out.status.code() == Some(3) {
            "合言葉を確認してください"
        } else {
            "取得できません。接続先と通信状態を確認してください"
        }
        .into());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
fn sync(h: H) {
    if JOB.with(|j| j.borrow().is_some()) {
        return;
    }
    let key = get(h, 201);
    if key.trim().is_empty() {
        set(h, 204, "ダッシュボードの閲覧用合言葉を入力してください");
        return;
    }
    KEY.with(|s| *s.borrow_mut() = key.clone());
    let (tx, rx) = mpsc::channel();
    JOB.with(|s| *s.borrow_mut() = Some(rx));
    std::thread::spawn(move || {
        let _ = tx.send(fetch(key));
    });
    set(h, 204, "共有目標と進捗を取得中…");
    unsafe {
        SetTimer(h, 1, 250, 0);
    }
}
extern "system" fn view(h: H, m: u32, w: usize, _: isize) -> isize {
    match m {
        0x110 => {
        native::round_fields(h);
            KEY.with(|k| set(h, 201, &k.borrow()));
            LAST.with(|v| set(h, 203, &v.borrow()));
            if JOB.with(|j| j.borrow().is_some()) {
                unsafe {
                    SetTimer(h, 1, 250, 0);
                }
            } else if !get(h, 201).is_empty() {
                sync(h)
            }
            1
        }
        0x113 => {
            let result = JOB.with(|j| {
                j.borrow().as_ref().and_then(|v| match v.try_recv() {
                    Ok(value) => Some(value),
                    Err(TryRecvError::Disconnected) => Some(Err("取得処理を終了しました".into())),
                    Err(_) => None,
                })
            });
            if let Some(result) = result {
                JOB.with(|j| *j.borrow_mut() = None);
                unsafe {
                    KillTimer(h, 1);
                }
                match result {
                    Ok(value) => {
                        set(h, 203, &value);
                        LAST.with(|v| *v.borrow_mut() = value);
                        set(h, 204, "最新の取得結果を表示しています");
                    }
                    Err(e) => set(h, 204, &e),
                }
            }
            1
        }
        0x111 if w & 65535 == 202 => {
            sync(h);
            1
        }
        0x10 | 0x111 if m == 0x10 || w & 65535 == 2 => {
            unsafe {
                KillTimer(h, 1);
                EndDialog(h, 0);
            }
            1
        }
        _ => 0,
    }
}
pub fn open(h: H) {
    unsafe {
        DialogBoxParamW(GetModuleHandleW(std::ptr::null()), 200 as Ptr, h, view, 0);
    }
}
