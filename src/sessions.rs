use super::*;
use std::sync::mpsc::{self, TryRecvError};
#[link(name = "user32")]
extern "system" {
    fn SetWindowLongPtrW(h: H, i: i32, v: H) -> H;
    fn GetWindowLongPtrW(h: H, i: i32) -> H;
    fn SetTimer(h: H, i: usize, ms: u32, cb: usize) -> usize;
    fn KillTimer(h: H, i: usize) -> i32;
}
#[derive(Clone)]
struct Entry {
    agent: usize,
    id: String,
    project: String,
    title: String,
    date: String,
}
fn scan() -> Result<Vec<Entry>> {
    let shell = PathBuf::from(std::env::var_os("SystemRoot").ok_or("Windowsの場所が不明です")?)
        .join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
    let out = Command::new(shell)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            include_str!("session_list.ps1"),
        ])
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err("セッション履歴を読み取れませんでした".into());
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let v: Vec<_> = line.split('\t').collect();
            if v.len() != 5 {
                return None;
            }
            Some(Entry {
                agent: match v[0] {
                    "0" => 0,
                    "1" => 1,
                    _ => return None,
                },
                id: v[1].into(),
                project: v[2].into(),
                title: v[3].into(),
                date: v[4].into(),
            })
        })
        .collect())
}
struct Picker {
    entries: Vec<Entry>,
    rx: mpsc::Receiver<Result<Vec<Entry>>>,
}
extern "system" fn dialog(h: H, m: u32, w: usize, l: isize) -> isize {
    unsafe {
        if m == 0x110 {
            SetWindowLongPtrW(h, -21, l);
            SetTimer(h, 1, 180, 0);
            return 1;
        }
        let ptr = GetWindowLongPtrW(h, -21) as *mut Picker;
        if ptr.is_null() {
            return 0;
        }
        let p = &mut *ptr;
        let id = w & 65535;
        if m == 0x113 {
            let result = match p.rx.try_recv() {
                Ok(v) => v,
                Err(TryRecvError::Empty) => return 1,
                Err(_) => Err("読み込みが中断されました".into()),
            };
            KillTimer(h, 1);
            match result {
                Ok(rows) => {
                    p.entries = rows;
                    for row in &p.entries {
                        msg(
                            h,
                            602,
                            0x180,
                            0,
                            wide(&format!(
                                "{}　{}　{}",
                                row.date, AGENTS[row.agent], row.title
                            ))
                            .as_ptr() as _,
                        );
                    }
                    set(
                        h,
                        601,
                        if p.entries.is_empty() {
                            "ローカルCLIの履歴がありません"
                        } else {
                            "最近のセッション　｜　更新日時順"
                        },
                    );
                }
                Err(e) => set(h, 601, &e),
            }
            return 1;
        }
        if m == 0x10 || m == 0x111 && id == 2 {
            KillTimer(h, 1);
            EndDialog(h, 0);
            return 1;
        }
        if m == 0x111 {
            let i = msg(h, 602, 0x188, 0, 0);
            if i >= 0 && (i as usize) < p.entries.len() {
                if id == 602 {
                    set(h, 603, &p.entries[i as usize].project);
                }
                if id == 604 || id == 605 || id == 602 && w >> 16 == 2 {
                    KillTimer(h, 1);
                    EndDialog(h, 1 + i * 2 + if id == 604 { 0 } else { 1 });
                    return 1;
                }
            }
        }
    }
    0
}
pub fn open(h: H, a: &mut App) -> Result<()> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(scan());
    });
    let mut p = Picker {
        entries: vec![],
        rx,
    };
    let n = unsafe {
        DialogBoxParamW(
            GetModuleHandleW(std::ptr::null()),
            600 as Ptr,
            h,
            dialog,
            &mut p as *mut _ as _,
        )
    };
    if n < 0 {
        return Err("履歴画面を開けません".into());
    }
    if n == 0 {
        return Ok(());
    }
    let e = &p.entries[((n - 1) / 2) as usize];
    let task = Task {
        agent: e.agent,
        session: e.id.clone(),
        title: e.title.clone(),
        ..Default::default()
    };
    select(h, a, &e.project, task, n % 2 == 0)
}
fn select(h: H, a: &mut App, root: &str, mut task: Task, ask: bool) -> Result<()> {
    let path = Path::new(root)
        .canonicalize()
        .map_err(|_| format!("作業フォルダーを開けません　{root}"))?;
    if !path.is_dir() {
        return Err("作業フォルダーが見つかりません".into());
    }
    let project = path.to_str().ok_or("パス変換エラー")?.to_owned();
    task.project = project.clone();
    let mut next = a.clone();
    let proj = next
        .projects
        .iter()
        .position(|p| p == &project)
        .unwrap_or_else(|| {
            next.projects.push(project);
            next.projects.len() - 1
        });
    next.project = proj;
    let i = next
        .tasks
        .iter()
        .position(|t| {
            t.project == task.project
                && if task.session.is_empty() {
                    t.session.is_empty() && t.title == task.title
                } else {
                    t.agent == task.agent && t.session == task.session
                }
        })
        .unwrap_or_else(|| {
            next.tasks.push(task);
            next.tasks.len() - 1
        });
    next.task = Some(i);
    next.department_filter.clear();
    next.query.clear();
    next.unassigned = false;
    next.agent_filter = 0;
    next.metric = 0;
    next.save()?;
    *a = next;
    set(h, 182, "");
    set(h, 124, "");
    msg(h, 125, 0x14e, 0, 0);
    refresh(h, a);
    render(h, a);
    if ask {
        ask::open(h, a)?;
        refresh(h, a);
        render(h, a);
    }
    Ok(())
}
pub fn own_source(p: &Path) -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    exe.ancestors().skip(1).take(3).any(|root| {
        root.join("src/ask.rs").is_file()
            && root.join("ui.rc").is_file()
            && root.join("Cargo.toml").is_file()
            && root.canonicalize().ok().as_deref() == Some(p)
    })
}
pub fn improve(h: H, a: &mut App) -> Result<()> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let root = exe
        .ancestors()
        .skip(1)
        .take(3)
        .filter_map(|p| p.canonicalize().ok())
        .find(|p| own_source(p))
        .ok_or("ZIPをソース一式とともに展開してから起動してください")?;
    select(
        h,
        a,
        root.to_str().ok_or("パス変換エラー")?,
        Task {
            title: "KAGARI自身の改善".into(),
            ..Default::default()
        },
        true,
    )
}
