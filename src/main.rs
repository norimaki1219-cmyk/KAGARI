#![windows_subsystem = "windows"]
mod ask;
mod brand;
mod chart;
mod client;
mod editor;
mod native;
mod picker;
mod sessions;
mod shared;
mod tree;
mod visual;
use std::{
    cell::RefCell,
    fs,
    io::{Read, Seek, SeekFrom},
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
};
type H = isize;
type Ptr = *const u16;
type Dialog = extern "system" fn(H, u32, usize, isize) -> isize;
#[link(name = "user32")]
extern "system" {
    fn DialogBoxParamW(i: H, t: Ptr, p: H, f: Dialog, l: isize) -> isize;
    fn SetFocus(h: H) -> H;
    fn ShowWindow(h: H, show: i32) -> i32;
    fn EnableWindow(h: H, b: i32) -> i32;
    fn EndDialog(h: H, r: isize) -> i32;
    fn GetDlgItem(h: H, id: i32) -> H;
    fn SendMessageW(h: H, m: u32, w: usize, l: isize) -> isize;
    fn GetWindowTextLengthW(h: H) -> i32;
    fn GetWindowTextW(h: H, b: *mut u16, n: i32) -> i32;
    fn SetWindowTextW(h: H, s: Ptr) -> i32;
    fn MessageBoxW(h: H, s: Ptr, t: Ptr, f: u32) -> i32;
}
#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryW(s: Ptr) -> H;
    fn GetModuleHandleW(s: Ptr) -> H;
    fn GetLocalTime(t: *mut [u16; 8]);
    fn CreateMutexW(a: usize, o: i32, n: Ptr) -> H;
    fn GetLastError() -> u32;
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn msg(h: H, id: i32, m: u32, w: usize, l: isize) -> isize {
    unsafe { SendMessageW(GetDlgItem(h, id), m, w, l) }
}
fn set(h: H, id: i32, s: &str) {
    unsafe {
        SetWindowTextW(GetDlgItem(h, id), wide(s).as_ptr());
    }
}
fn get(h: H, id: i32) -> String {
    unsafe {
        let c = GetDlgItem(h, id);
        let mut buf = vec![0; GetWindowTextLengthW(c) as usize + 1];
        let n = GetWindowTextW(c, buf.as_mut_ptr(), buf.len() as i32);
        String::from_utf16_lossy(&buf[..n as usize])
    }
}
fn notice(h: H, s: &str) {
    unsafe {
        MessageBoxW(h, wide(s).as_ptr(), wide("KAGARI").as_ptr(), 48);
    }
}
const AGENTS: [&str; 2] = ["Codex", "Claude Code"];
const STATES: [&str; 4] = ["未着手", "作業中", "確認待ち", "完了"];
const DOCS: [&str; 3] = ["仕様書", "作業ログ", "結果"];
const REQUESTS: [&str; 5] = [
    "進捗を確認",
    "未完了の作業を洗い出す",
    "次に取り組む作業を提案して",
    "変更内容をレビューして",
    "不具合の原因を調べて",
];
#[derive(Clone, Default)]
struct Task {
    session: String,
    history_key: String,
    model: String,
    pinned: bool,
    project: String,
    title: String,
    agent: usize,
    status: usize,
    due: String,
    check: String,
    dept: String,
    docs: [String; 3],
}
#[derive(Default, Clone)]
struct App {
    panes: u8,
    metric: usize,
    search_panel: bool,
    query: String,
    department_filter: String,
    unassigned: bool,
    agent_filter: usize,
    projects: Vec<String>,
    tasks: Vec<Task>,
    project: usize,
    task: Option<usize>,
    doc: usize,
    log: String,
}
thread_local! {static APP:RefCell<App>=RefCell::default();}
type Result<T> = std::result::Result<T, String>;
fn state_path() -> Result<PathBuf> {
    Ok(
        PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA unavailable")?)
            .join("RustAIWorkspace")
            .join("workspace.tsv"),
    )
}
fn escape(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\t', "%09")
        .replace('\n', "%0A")
        .replace('\r', "%0D")
}
fn decode(s: &str) -> String {
    s.replace("%09", "\t")
        .replace("%0A", "\n")
        .replace("%0D", "\r")
        .replace("%25", "%")
}
impl App {
    fn save(&self) -> Result<()> {
        let mut rows = vec![format!("V\t{}", self.panes)];
        for p in &self.projects {
            rows.push(format!("P\t{}", escape(p)));
        }
        for t in &self.tasks {
            let fields = [
                t.project.clone(),
                t.title.clone(),
                t.agent.to_string(),
                t.status.to_string(),
                t.due.clone(),
                t.check.clone(),
                t.dept.clone(),
                t.docs[0].clone(),
                t.docs[1].clone(),
                t.docs[2].clone(),
                (t.pinned as u8).to_string(),
                t.history_key.clone(),
                t.model.clone(),
                t.session.clone(),
            ];
            rows.push(format!(
                "T\t{}",
                fields
                    .iter()
                    .map(|s| escape(s))
                    .collect::<Vec<_>>()
                    .join("\t")
            ));
        }
        rows.push(format!("L\t{}", escape(&self.log)));
        let dest = state_path()?;
        fs::create_dir_all(dest.parent().unwrap()).map_err(|e| e.to_string())?;
        let tmp = dest.with_extension("tmp");
        fs::write(&tmp, rows.join("\n"))
            .and_then(|_| fs::rename(tmp, dest))
            .map_err(|e| e.to_string())
    }
    fn load() -> Result<Self> {
        let mut a = Self::default();
        let data = match fs::read_to_string(state_path()?) {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(a),
            Err(e) => return Err(e.to_string()),
        };
        for line in data.lines() {
            let f: Vec<_> = line.split('\t').map(decode).collect();
            match f[0].as_str() {
                "V" if f.len() == 2 => a.panes = f[1].parse::<u8>().unwrap_or(0) & 31,
                "P" if f.len() == 2 => a.projects.push(f[1].clone()),
                "L" if f.len() == 2 => a.log = f[1].clone(),
                "T" if matches!(f.len(), 11 | 12 | 14 | 15) => a.tasks.push(Task {
                    session: f.get(14).cloned().unwrap_or_default(),
                    history_key: match f.get(12) {
                        Some(k)
                            if k.len() <= 64
                                && k.bytes().all(|b| b.is_ascii_digit() || b == b'-') =>
                        {
                            k.clone()
                        }
                        Some(_) => return Err("履歴IDが不正です".into()),
                        None => String::new(),
                    },
                    model: f.get(13).cloned().unwrap_or_default(),
                    pinned: matches!(f.get(11).map(String::as_str), Some("1")),
                    project: f[1].clone(),
                    title: f[2].clone(),
                    agent: f[3]
                        .parse()
                        .ok()
                        .filter(|v| *v < 2)
                        .ok_or("Invalid agent")?,
                    status: f[4]
                        .parse()
                        .ok()
                        .filter(|v| *v < 4)
                        .ok_or("Invalid status")?,
                    due: f[5].clone(),
                    check: f[6].clone(),
                    dept: f[7].clone(),
                    docs: [f[8].clone(), f[9].clone(), f[10].clone()],
                }),
                _ => return Err("保存形式を読み取れません".into()),
            }
        }
        Ok(a)
    }
    fn root(&self) -> Result<&str> {
        self.projects
            .get(self.project)
            .map(String::as_str)
            .ok_or("プロジェクトを登録してください".into())
    }
    fn scope(&self) -> Vec<usize> {
        let mut visible: Vec<_> = self
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| Some(&t.project) == self.projects.get(self.project))
            .filter(|(_, t)| {
                if self.unassigned {
                    t.dept.is_empty()
                } else {
                    self.department_filter.is_empty() || t.dept == self.department_filter
                }
            })
            .filter(|(_, t)| self.agent_filter == 0 || self.agent_filter == t.agent + 1)
            .map(|(i, _)| i)
            .collect();
        let day = today();
        visible.sort_by_key(|&i| {
            let t = &self.tasks[i];
            (
                !t.pinned,
                priority(t, day).0,
                parse_date(&t.due).unwrap_or(i64::MAX),
            )
        });
        visible
    }
    fn indices(&self) -> Vec<usize> {
        let d = today();
        self.scope()
            .into_iter()
            .filter(|&i| matches_metric(&self.tasks[i], self.metric, d))
            .filter(|&i| {
                self.query.is_empty() || self.tasks[i].title.to_lowercase().contains(&self.query)
            })
            .collect()
    }
}
fn options(h: H, id: i32, items: &[&str]) {
    msg(h, id, 0x14b, 0, 0);
    for s in items {
        msg(h, id, 0x143, 0, wide(s).as_ptr() as _);
    }
    msg(h, id, 0x14e, 0, 0);
}
fn choice(h: H, id: i32) -> usize {
    msg(h, id, 0x147, 0, 0).max(0) as usize
}
fn date(y: i32, m: i32, d: i32) -> Option<i64> {
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let days = [31, 28 + leap as i32, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if !(1..=9999).contains(&y) || !(1..=12).contains(&m) || d < 1 || d > days[m as usize - 1] {
        return None;
    }
    let prev = (y - 1) as i64;
    Some(
        365 * prev + prev / 4 - prev / 100
            + prev / 400
            + days[..m as usize - 1].iter().sum::<i32>() as i64
            + d as i64
            - 1,
    )
}
fn parse_date(s: &str) -> Option<i64> {
    let v: Vec<_> = s.split('-').collect();
    if v.len() != 3 || v[0].len() != 4 || v[1].len() != 2 || v[2].len() != 2 {
        return None;
    }
    date(v[0].parse().ok()?, v[1].parse().ok()?, v[2].parse().ok()?)
}
fn priority(t: &Task, day: i64) -> (u8, &'static str) {
    let due = parse_date(&t.due);
    if t.status == 3 {
        return (6, "完了");
    }
    if due.is_some_and(|d| d < day) {
        (0, "期限超過")
    } else if due == Some(day) {
        (1, "本日期限")
    } else if t.status == 2 {
        (2, "確認待ち")
    } else if due.is_some_and(|d| d <= day + 3) {
        (3, "期限間近")
    } else if t.status == 1 {
        (4, "作業中")
    } else {
        (5, "通常")
    }
}
fn today() -> i64 {
    let mut now = [0u16; 8];
    unsafe { GetLocalTime(&mut now) };
    date(now[0] as _, now[1] as _, now[3] as _).unwrap_or(0)
}
fn matches_metric(t: &Task, metric: usize, day: i64) -> bool {
    match metric {
        1 => parse_date(&t.due).is_some_and(|d| d / 7 == day / 7),
        2 => t.status == 2,
        3 => t.status != 3 && parse_date(&t.due).is_some_and(|d| d < day),
        _ => true,
    }
}
fn due_shortcut(offset: i64) -> String {
    let target = today() + offset;
    let mut now = [0u16; 8];
    unsafe { GetLocalTime(&mut now) };
    for y in now[0] as i32..=now[0] as i32 + 1 {
        for m in 1..=12 {
            if let Some(first) = date(y, m, 1) {
                let d = (target - first + 1) as i32;
                if date(y, m, d) == Some(target) {
                    return format!("{y:04}-{m:02}-{d:02}");
                }
            }
        }
    }
    String::new()
}
fn refresh(h: H, a: &App) {
    tree::roots(h, a);
    msg(h, 106, 0x1009, 0, 0);
    let day = today();
    let scope = a.scope();
    let mut counts = [0usize; 4];
    let mut done = 0;
    for i in scope {
        let t = &a.tasks[i];
        for (k, n) in counts.iter_mut().enumerate() {
            *n += matches_metric(t, k, day) as usize
        }
        if t.status == 3 && matches_metric(t, 1, day) {
            done += 1
        }
    }
    for (k, label) in ["▦ すべて", "▤ 今週期限", "◷ 確認待ち", "⚠ 期限超過"]
        .iter()
        .enumerate()
    {
        let id = 150 + k as i32;
        set(h, id, &format!("{label}  {} 件", counts[k]));
        msg(h, id, 0xf1, (a.metric == k) as usize, 0);
    }
    let percent = if counts[1] == 0 {
        0
    } else {
        100 * done / counts[1]
    };
    chart::update(h, a);
    set(
        h,
        105,
        &if counts[1] == 0 {
            "今週期限　—".into()
        } else {
            format!("今週　{percent}%　({done}/{})", counts[1])
        },
    );
    panel(h, a);
    for (row, i) in a.indices().into_iter().enumerate() {
        let t = &a.tasks[i];
        native::row(
            h,
            row,
            [
                &format!("{}{}", if t.pinned { "★ " } else { "" }, t.title),
                if priority(t, day).1 == STATES[t.status] {
                    "—"
                } else {
                    priority(t, day).1
                },
                visual::SYMBOLS[t.status],
                AGENTS[t.agent],
                &t.check,
            ],
            t.status,
            Some(i) == a.task,
        );
    }
}

fn panel(h: H, a: &App) {
    unsafe {
        for id in [104, 127, 143, 145] {
            ShowWindow(
                GetDlgItem(h, id),
                if a.search_panel || a.panes & 1 != 0 {
                    0
                } else {
                    5
                },
            );
        }
        ShowWindow(
            GetDlgItem(h, 191),
            if a.search_panel && a.panes & 1 == 0 {
                5
            } else {
                0
            },
        );
    }
    set(
        h,
        142,
        if a.search_panel {
            "検索結果"
        } else {
            "プロジェクト"
        },
    );
    if a.search_panel {
        msg(h, 191, 0x184, 0, 0);
        for (row, i) in a.indices().into_iter().enumerate() {
            msg(h, 191, 0x180, 0, wide(&a.tasks[i].title).as_ptr() as _);
            if a.task == Some(i) {
                msg(h, 191, 0x186, row, 0);
            }
        }
    }
}
fn render(h: H, a: &App) {
    set(
        h,
        141,
        if a.projects.is_empty() {
            "プロジェクトを追加してください"
        } else if a.task.is_some() {
            ""
        } else {
            "新規タスク"
        },
    );
    let t = a.task.map(|i| a.tasks[i].clone()).unwrap_or_default();
    set(
        h,
        159,
        if t.pinned {
            "★ 固定を解除"
        } else {
            "☆ ピン留め"
        },
    );
    unsafe {
        EnableWindow(GetDlgItem(h, 159), a.task.is_some() as i32);
    }
    let next = [
        "▶  作業を開始",
        "確認待ちにする",
        "✓  完了にする",
        "再開する",
    ][t.status];
    set(h, 156, next);
    unsafe {
        EnableWindow(GetDlgItem(h, 156), a.task.is_some() as i32);
    }
    set(h, 118, &t.docs[a.doc]);
    for n in 0..3 {
        msg(h, 175 + n, 0xf1, (a.doc == n as usize) as usize, 0);
    }
    attach_state(h, a);
    unsafe {
        EnableWindow(GetDlgItem(h, 115), a.task.is_some() as i32);
        EnableWindow(GetDlgItem(h, 160), a.task.is_some() as i32);
    }
    preview(h, a);
}
fn rtf_text(s: &str) -> String {
    let mut out = String::new();
    for c in s.encode_utf16() {
        match c {
            92 | 123 | 125 => {
                out.push('\\');
                out.push(c as u8 as char)
            }
            32..=126 => out.push(c as u8 as char),
            _ => out.push_str(&format!("\\u{}?", c as i16)),
        }
    }
    out
}
fn markdown(s: &str) -> String {
    let mut r = String::from(
        "{\\rtf1\\ansi\\uc1\\deff0{\\fonttbl{\\f0 Yu Gothic UI;}{\\f1 Consolas;}}\\f0\\fs20 ",
    );
    let mut code = false;
    for line in s.lines() {
        if line.starts_with("\u{60}\u{60}\u{60}") {
            code = !code;
            continue;
        }
        let level = line.bytes().take_while(|v| *v == b'#').count();
        let (style, body) = if code {
            ("\\f1 ", line.to_owned())
        } else if (1..=6).contains(&level) && line[level..].starts_with(' ') {
            (
                if level == 1 {
                    "\\b\\fs30 "
                } else {
                    "\\b\\fs24 "
                },
                line[level..].trim().to_owned(),
            )
        } else {
            (
                "",
                line.replacen("- [x] ", "☑ ", 1)
                    .replacen("- [ ] ", "□ ", 1)
                    .strip_prefix("- ")
                    .map(|s| format!("• {s}"))
                    .unwrap_or_else(|| {
                        line.replacen("- [x] ", "☑ ", 1).replacen("- [ ] ", "□ ", 1)
                    }),
            )
        };
        r.push_str(&format!("{{{}{}}}\\par ", style, rtf_text(&body)))
    }
    r.push('}');
    r
}
fn doc_path(root: &str, s: &str) -> Result<PathBuf> {
    let p = Path::new(s);
    if p.is_absolute()
        || p.components().any(|c| {
            !matches!(
                c,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
    {
        return Err("プロジェクト内の相対パスを指定してください".into());
    }
    if !p
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"))
    {
        return Err("Markdownファイルを指定してください".into());
    }
    Ok(Path::new(root).join(p))
}
fn preview(h: H, a: &App) {
    let s = get(h, 118);
    if s.trim().is_empty() {
        set(
            h,
            120,
            if a.projects.is_empty() {
                "「＋ プロジェクト」から作業フォルダーを選んでください。"
            } else {
                "左のツリーからMarkdownを選んでください。"
            },
        );
        return;
    }
    display_document(h, doc_path(a.root().unwrap_or(""), &s));
}
fn display_document(h: H, path: Result<PathBuf>) {
    let result: Result<String> = (|| {
        let path = path?;
        let f = fs::File::open(path).map_err(|e| e.to_string())?;
        let mut body = String::new();
        f.take(512 * 1024 + 1)
            .read_to_string(&mut body)
            .map_err(|e| e.to_string())?;
        if body.len() > 512 * 1024 {
            return Err("文書の表示上限は512 KiBです".into());
        }
        Ok(markdown(&body))
    })();
    match result {
        Ok(r) => {
            let mut bytes = r.into_bytes();
            bytes.push(0);
            let params = [0u32, 1252];
            msg(h, 120, 0x461, params.as_ptr() as usize, bytes.as_ptr() as _);
        }
        Err(e) => set(h, 120, &e),
    }
}
fn run(h: H, a: &mut App, id: i32, event: usize) -> Result<()> {
    match (id, event) {
        (185 | 186, 0) => {
            if a.panes & 1 != 0 {
                run(h, a, 194, 0)?;
            }
            a.search_panel = id == 186;
            panel(h, a);
            unsafe {
                SetFocus(GetDlgItem(h, if a.search_panel { 182 } else { 104 }));
            }
        }
        (187, 0) => return run(h, a, 121, 0),
        (188, 0) => return run(h, a, 115, 0),
        (193, 0) => {
            if let Some(action) = native::layout_menu(h, a.panes) {
                return run(h, a, action, 0);
            }
        }
        (194..=200, 0) => {
            let mut next = a.clone();
            next.panes = match id {
                194..=197 => a.panes ^ (1 << (id - 194)),
                198 => a.panes & !16,
                199 => a.panes | 16,
                _ => 0,
            };
            next.save()?;
            *a = next;
            native::apply_layout(h, a.panes);
            panel(h, a);
            attach_state(h, a);
        }
        (190, 0) => return sessions::open(h, a),
        (192, 0) => return sessions::improve(h, a),
        (189, 0) => return run(h, a, 116, 0),
        (191, 1) => {
            if let Some(&i) = a.indices().get(msg(h, 191, 0x188, 0, 0) as usize) {
                a.task = Some(i);
                refresh(h, a);
                render(h, a);
            }
        }
        (182, 0x300) => {
            a.query = get(h, 182).trim().to_lowercase();
            let visible = a.indices();
            if !a.task.is_some_and(|i| visible.contains(&i)) {
                a.task = visible.first().copied();
                render(h, a);
            }
            refresh(h, a);
        }
        (183, 0) => {
            set(h, 182, "");
            a.query.clear();
            a.task = a.indices().first().copied();
            refresh(h, a);
            render(h, a);
        }
        (181, 0) => {
            save_doc(h, a)?;
            attach_state(h, a);
        }
        (150..=153, 0) => {
            a.metric = (id - 150) as usize;
            a.task = a.indices().first().copied();
            refresh(h, a);
            render(h, a);
        }
        (159, 0) => {
            let i = a.task.ok_or("タスクを選んでください")?;
            let mut updated = a.clone();
            updated.tasks[i].pinned = !updated.tasks[i].pinned;
            updated.save()?;
            *a = updated;
            refresh(h, a);
            set(
                h,
                159,
                if a.tasks[i].pinned {
                    "★ 固定を解除"
                } else {
                    "☆ ピン留め"
                },
            );
        }
        (156, 0) => {
            let t = a.task.ok_or("タスクを選んでください")?;
            let mut next = a.clone();
            next.tasks[t].status = [1, 2, 3, 1][next.tasks[t].status];
            next.save()?;
            *a = next;
            if !a.indices().contains(&t) {
                a.task = a.indices().first().copied();
            }
            refresh(h, a);
            render(h, a);
        }
        (128, 0) => {
            a.metric = 0;
            set(h, 124, "");
            msg(h, 125, 0x14e, 0, 0);
            a.unassigned = false;
            a.department_filter.clear();
            a.agent_filter = 0;
            a.task = a.indices().first().copied();
            refresh(h, a);
            render(h, a);
        }
        (155, 1) => {
            let Some(name) = chart::selection(h) else {
                return Ok(());
            };

            a.unassigned = name.is_empty();
            a.department_filter = name;
            a.metric = 0;
            set(h, 124, &a.department_filter);
            a.task = a.indices().first().copied();
            refresh(h, a);
            render(h, a);
        }
        (126, 0) => {
            a.unassigned = false;
            a.department_filter = get(h, 124).trim().to_owned();
            a.agent_filter = choice(h, 125);
            a.task = a.indices().first().copied();
            refresh(h, a);
            render(h, a);
        }
        (127, 0) => {
            let Some(folder) = picker::folder(h) else {
                return Ok(());
            };
            return register(h, a, &folder);
        }
        (145, 0) => {
            let folder = tree::selected_folder(h)?;
            return register(h, a, &folder);
        }
        (143, 0) => {
            tree::reset(h);
            tree::roots(h, a);
        }
        (106, 1) => {
            let selected = a
                .indices()
                .get(msg(h, 106, 0x100c, usize::MAX, 2) as usize)
                .copied();
            if selected != a.task {
                a.task = selected;
                render(h, a);
            }
        }
        (113 | 160, 0) => {
            editor::open(h, a, id == 113)?;
        }
        (175..=177, 0) => {
            a.doc = (id - 175) as usize;
            render(h, a);
        }
        (119, 0) => {
            preview(h, a);
        }
        (115, 0) => {
            ask::open(h, a)?;
            refresh(h, a);
            render(h, a);
        }
        (116, 0) => {
            let root = a.root()?;
            let shell =
                PathBuf::from(std::env::var_os("SystemRoot").ok_or("SystemRoot unavailable")?)
                    .join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
            let mut cmd = Command::new(shell);
            cmd.current_dir(root)
                .creation_flags(0x10)
                .args(["-NoLogo", "-NoProfile"]);
            cmd.spawn().map_err(|e| e.to_string())?;
        }
        (103, 0) => shared::open(h),
        (121, 0) => {
            let root = a.root()?;
            let mut out = String::new();
            for args in [
                &["status", "--short"][..],
                &["diff", "--no-ext-diff", "--stat"],
                &["diff", "--no-ext-diff"],
                &["diff", "--cached", "--no-ext-diff"],
            ] {
                let mut child = Command::new("git")
                    .args(args)
                    .current_dir(root)
                    .creation_flags(0x08000000)
                    .stdout(std::process::Stdio::piped())
                    .spawn()
                    .map_err(|e| e.to_string())?;
                let mut b = Vec::new();
                child
                    .stdout
                    .take()
                    .unwrap()
                    .take(131073)
                    .read_to_end(&mut b)
                    .map_err(|e| e.to_string())?;
                let truncated = b.len() > 131072;
                if truncated {
                    let _ = child.kill();
                }
                let status = child.wait().map_err(|e| e.to_string())?;
                if !truncated && !status.success() {
                    return Err("Git差分を取得できません。リポジトリを確認してください".into());
                }
                out.push_str(&String::from_utf8_lossy(&b));
                if truncated {
                    out.push_str("\n…表示上限…\n")
                }
            }
            set(
                h,
                120,
                if out.is_empty() {
                    "変更はありません"
                } else {
                    &out
                },
            );
        }
        (122, 0) => {
            let path = get(h, 123);
            let mut f = fs::File::open(&path).map_err(|e| e.to_string())?;
            let end = f.metadata().map_err(|e| e.to_string())?.len();
            f.seek(SeekFrom::Start(end.saturating_sub(65536)))
                .map_err(|e| e.to_string())?;
            let mut b = Vec::new();
            f.take(65536)
                .read_to_end(&mut b)
                .map_err(|e| e.to_string())?;
            set(h, 120, &String::from_utf8_lossy(&b));
            a.log = path;
            a.save()?;
        }
        _ => {}
    }
    Ok(())
}
fn register(h: H, a: &mut App, folder: &str) -> Result<()> {
    let p = Path::new(folder)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !p.is_dir() {
        return Err("フォルダーを選択してください".into());
    }
    let s = p.to_str().ok_or("パス変換エラー")?.to_owned();
    if !a.projects.contains(&s) {
        a.projects.push(s.clone())
    }
    a.project = a.projects.iter().position(|p| p == &s).unwrap();
    a.task = a.indices().first().copied();
    a.save()?;
    refresh(h, a);
    render(h, a);
    Ok(())
}
fn attach_state(h: H, a: &App) {
    let path = get(h, 118);
    let show = a
        .task
        .is_some_and(|i| !path.is_empty() && path != a.tasks[i].docs[a.doc]);
    unsafe {
        ShowWindow(
            GetDlgItem(h, 181),
            if show && a.panes & 8 == 0 { 5 } else { 0 },
        );
    }
    set(h, 181, &format!("{}に登録", DOCS[a.doc]));
}
fn save_doc(h: H, a: &mut App) -> Result<()> {
    if let Some(i) = a.task {
        let path = get(h, 118);
        if !path.is_empty() {
            doc_path(a.root()?, &path)?;
        }
        let mut next = a.clone();
        next.tasks[i].docs[next.doc] = path;
        next.save()?;
        *a = next;
        set(h, 141, "文書を関連付けました");
    }
    Ok(())
}
extern "system" fn dialog(h: H, m: u32, w: usize, l: isize) -> isize {
    match m {
        0x2b if w == 125 => {
            brand::draw(l);
            1
        }
        0x2b if w == 155 => {
            chart::draw(l);
            1
        }
        0x2b if visual::action(w as i32) => {
            visual::draw(l);
            1
        }
        0x138 => visual::legend(w, l),
        0x136 => native::background(w),
        5 => {
            if w != 1 {
                native::resize(h);
            }
            0
        }
        0x24 => {
            native::minimum(l);
            0
        }
        0x4e => {
            let hdr = unsafe { &*(l as *const native::Header) };
            if hdr.code == -530 {
                visual::hint(l);
                return 0;
            }
            if hdr.id == 106 && hdr.code == -12 {
                return visual::list_draw(h, l);
            }
            if hdr.id == 104 {
                return tree::notification(h, hdr.code, l);
            }
            if hdr.id == 106 && matches!(hdr.code, -3 | -4) {
                APP.with(|cell| {
                    if let Ok(mut a) = cell.try_borrow_mut() {
                        if a.task.is_some() {
                            if let Err(e) = run(h, &mut a, 160, 0) {
                                notice(h, &e)
                            }
                        }
                    }
                });
                return 1;
            }
            if hdr.id == 106 && hdr.code == -101 {
                APP.with(|cell| {
                    if let Ok(mut a) = cell.try_borrow_mut() {
                        let _ = run(h, &mut a, 106, 1);
                    }
                });
            }
            0
        }
        0x110 => {
            match App::load() {
                Ok(mut a) => APP.with(|cell| {
                    let mut state = cell.borrow_mut();
                    a.task = a.indices().first().copied();
                    native::init(h);

                    options(h, 125, &["全担当", "Codex", "Claude Code"]);
                    refresh(h, &a);
                    render(h, &a);
                    set(h, 123, &a.log);
                    *state = a;
                    native::capture(h);
                    native::apply_layout(h, state.panes);
                    panel(h, &state);
                    attach_state(h, &state);
                }),
                Err(e) => {
                    notice(h, &e);
                    unsafe {
                        EndDialog(h, 1);
                    }
                }
            }
            1
        }
        0x10 => {
            unsafe {
                EndDialog(h, 0);
            }
            1
        }
        0x111 => {
            let id = (w & 65535) as i32;
            if visual::action(id) && w >> 16 == 6 {
                set(h, 141, &get(h, id));
                return 1;
            }
            if visual::action(id) && w >> 16 == 7 {
                set(h, 141, "");
                return 1;
            }
            if id == 2 {
                return dialog(h, 0x10, 0, 0);
            }
            APP.with(|cell| {
                if let Ok(mut a) = cell.try_borrow_mut() {
                    if let Err(e) = run(h, &mut a, id, w >> 16) {
                        notice(h, &e)
                    }
                }
            });
            1
        }
        _ => 0,
    }
}
fn main() {
    native::controls();
    unsafe {
        let lock = CreateMutexW(0, 0, wide("Local\\RustAIWorkspace_v1").as_ptr());
        if lock == 0 || GetLastError() == 183 {
            notice(0, "KAGARI は既に起動中か、開始できません。");
            return;
        }
        LoadLibraryW(wide("Msftedit.dll").as_ptr());
        if DialogBoxParamW(GetModuleHandleW(std::ptr::null()), 100 as Ptr, 0, dialog, 0) == -1 {
            notice(0, "画面を初期化できません");
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dates() {
        assert!(parse_date("2025-02-29").is_none());
        assert_eq!(
            parse_date("2024-03-01").unwrap() - parse_date("2024-02-29").unwrap(),
            1
        );
        assert_eq!(parse_date("2026-11-16").unwrap() % 7, 0)
    }
    #[test]
    fn encoding() {
        let s = "日本語\t%09\n";
        assert_eq!(decode(&escape(s)), s);
        assert!(markdown("# 結果\n- [x] OK").contains("\\b\\fs30"));
        assert!(doc_path("D:\\work", "../private.md").is_err());
    }
}
#[cfg(not(test))]
mod bridge;
#[cfg(test)]
mod gui_test;
