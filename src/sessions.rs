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
pub(super) fn latest_reply(agent: usize, session: &str) -> Option<Reply> {
    let shell = PathBuf::from(std::env::var_os("SystemRoot")?).join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
    let output = Command::new(shell)
        .args(["-NoProfile", "-NonInteractive", "-Command", include_str!("session_reply.ps1")])
        .env("KAGARI_SESSION", session).env("KAGARI_AGENT", agent.to_string())
        .creation_flags(0x08000000).output().ok()?;
    if !output.status.success() {return None;}
    let raw = String::from_utf8(output.stdout).ok()?;
    let (stamp, reply) = raw.split_once('\n')?;
    let (text,previous) = reply.split_once('\0')?;
    Some(Reply{time:stamp.trim().parse().ok()?,text:text.into(),previous:previous.into()})
}
fn scan(project: &str) -> Result<Vec<Entry>> {
    let root = Path::new(project);
    let mut former = Vec::new();
    if let Some(alias) = project_alias(root) {
        let company = root.ancestors().find(|p|p.file_name().is_some_and(|n| n.eq_ignore_ascii_case(".company"))).unwrap();
        let relative = root.strip_prefix(company).unwrap().to_str().unwrap();
        for path in former_roots(alias,relative) {
            former.push(company.join(path).to_string_lossy().into_owned());
        }
    }
    let shell = PathBuf::from(std::env::var_os("SystemRoot").ok_or("Windowsの場所が不明です")?)
        .join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
    let out = Command::new(shell)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            include_str!("session_list.ps1"),
        ])
        .env("KAGARI_HISTORY_ALIAS",project_alias(Path::new(project)).unwrap_or(""))
        .env("KAGARI_HISTORY_FORMER",former.join("\n"))
        .env("KAGARI_HISTORY_PROJECT", project)
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
    title: String,
    target: Option<String>,
}
extern "system" fn dialog(h: H, m: u32, w: usize, l: isize) -> isize {
    unsafe {
        if m == 0x110 {
        native::round_fields(h);
            SetWindowLongPtrW(h, -21, l);
            let picker = &*(l as *const Picker);
            SetWindowTextW(h, wide(&picker.title).as_ptr());
            EnableWindow(GetDlgItem(h,606),picker.target.is_some() as i32);
            if let Some(title)=&picker.target {set(h,607,&format!("引き継ぎ先：{title}"));}
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
                            "このプロジェクトのセッション履歴はありません"
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
                if id == 604 || id == 605 || (id==606 && p.target.is_some()) || id == 602 && w >> 16 == 2 {
                    KillTimer(h, 1);
                    EndDialog(h, 1 + i * 3 + match id {604=>0,606=>2,_=>1});
                    return 1;
                }
            }
        }
    }
    0
}
pub fn open(h: H, a: &mut App) -> Result<()> {
    let project = a.root()?.to_owned();
    let name = Path::new(&project).file_name().unwrap_or_default().to_string_lossy();
    let name = project_alias(Path::new(&project)).unwrap_or(&name);
    let title = format!("{name} のセッション履歴 — KAGARI");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(scan(&project));
    });
    let mut p = Picker {
        title,
        target: a.task.filter(|i|a.indices().contains(i)).map(|i|a.tasks[i].title.clone()),
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
    let action=(n-1)%3;
    let e = &p.entries[((n - 1) / 3) as usize];
    let task = Task {
        agent: e.agent,
        session: e.id.clone(),
        project: if action==2 {a.tasks[a.task.ok_or("タスクを選んでください")?].project.clone()} else {a.root()?.to_owned()},
        title: e.title.clone(),
        ..Default::default()
    };
    select(h, a, &e.project, task, action==1, action==2)
}
fn select(h: H, a: &mut App, root: &str, mut task: Task, ask: bool, handoff: bool) -> Result<()> {
    let path = Path::new(root)
        .canonicalize()
        .map_err(|_| format!("作業フォルダーを開けません　{root}"))?;
    if !path.is_dir() {
        return Err("作業フォルダーが見つかりません".into());
    }
    let cwd = path.to_str().ok_or("パス変換エラー")?;
    let project = if task.project.is_empty() {project_root(cwd).to_owned()} else {task.project.clone()};
    if project != cwd {task.cwd=cwd.to_owned();}
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
    let owner = next
        .tasks
        .iter()
        .position(|t| {
            t.project == task.project
                && if task.session.is_empty() {
                    t.session.is_empty() && t.title == task.title
                } else {
                    t.contains_session(task.agent,&task.session)
                }
        });
    let i=if handoff {
        let target=a.task.filter(|i|a.indices().contains(i)).ok_or("引き継ぎ先のタスクを選択してください")?;
        if owner.is_some_and(|i|i!=target){return Err("このセッションは別のタスクに登録済みです".into());}
        target
    } else { owner.unwrap_or(next.tasks.len()) };
    if i==next.tasks.len() {
        task.session_title=task.title.clone();
        next.tasks.push(task);
    } else if !task.session.is_empty() {
        next.tasks[i].link_session(TaskSession{id:task.session,title:task.title,agent:task.agent,cwd:task.cwd,..Default::default()});
    }
    next.task = Some(i);
    next.query.clear();
    next.agent_filter = 0;
    next.metric = 0;
    next.queue_filter = QueueFilter::All;
    next.save()?;
    *a = next;
    set(h, 182, "");
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
        false,
    )
}
