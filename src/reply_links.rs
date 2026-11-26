#[link(name="user32")]
extern "system" {
    fn SetWindowLongPtrW(h:H,index:i32,value:isize)->isize;
    fn GetWindowLongPtrW(h:H,index:i32)->isize;
}
use super::*;

fn web_url(text: &str) -> Option<&str> {
    let (scheme,rest)=text.split_once("://")?;
    let host=rest.split(['/', '?', '#']).next()?;
    ((scheme.eq_ignore_ascii_case("https") || scheme.eq_ignore_ascii_case("http"))
        && !host.is_empty() && !text.contains(['\\','"', '<','>'])
        && !text.chars().any(|c| c.is_control() || c.is_whitespace())).then_some(text)
}
fn path_without_line(path:&str)->&str {
    match path.rsplit_once(':') {
        Some((file,line)) if !line.is_empty() && line.bytes().all(|b|b.is_ascii_digit())=>file,
        _=>path,
    }
}
#[link(name="shell32")]
extern "system" {fn ShellExecuteW(h:H,op:Ptr,file:Ptr,args:Ptr,dir:Ptr,show:i32)->H;}
fn markdown_target(text: &str) -> Option<&str> {
    let path = text.split('#').next()?.trim().trim_matches(['<', '>']);
    let path = path.strip_prefix(r"\\?\").filter(|rest| rest.as_bytes().get(1)==Some(&b':')).unwrap_or(path);
    let file=path_without_line(path);
    let p = Path::new(file);
    if path.contains("://") || path.starts_with("//") || path.starts_with(r"\\") {
        return None;
    }
    if let Some((drive, rest)) = file.split_once(':') {
        if drive.len() != 1 || !drive.as_bytes()[0].is_ascii_alphabetic() || rest.contains(':') {
            return None;
        }
    }
    p.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"))
        .then_some(path)
}

// Return the displayed span and file destination. Keep the original answer intact.
pub(super) fn token(s: &str) -> Option<(usize, &str)> {
    if let Some(rest) = s.strip_prefix('`') {
        let end = rest.find('`')?;
        return Some((end + 2, markdown_target(&rest[..end])?));
    }
    if s.starts_with('[') {
        let start = s.find("](")? + 2;
        let mut depth = 1;
        for (i, ch) in s[start..].char_indices() {
            if ch == '(' {
                depth += 1;
            }
            if ch == ')' {
                depth -= 1;
                if depth == 0 {
                    let dest=s[start..start+i].trim().trim_start_matches('<').trim_end_matches('>');
                    return Some((start+i+1,web_url(dest).or_else(||markdown_target(dest))?));
                }
            }
        }
        return None;
    }
    if s.get(..8).is_some_and(|p| p.eq_ignore_ascii_case("https://"))
        || s.get(..7).is_some_and(|p| p.eq_ignore_ascii_case("http://")) {
        let mut depth = 0;
        let end = s.find(|c: char| match c {
            '(' => { depth += 1; false }
            ')' if depth > 0 => { depth -= 1; false }
            _ => c.is_whitespace() || c.is_control() || "\"‘’“”`<>「」『』。、，；：（）【】)".contains(c),
        }).unwrap_or(s.len());
        let url = s[..end].trim_end_matches([',', ';', '.', '!', '?']);
        return Some((url.len(), web_url(url)?));
    }
    let end = s
        .find(|c: char| c.is_whitespace() || "`[]()<>「」、。：".contains(c))
        .unwrap_or(s.len());
    let word = s[..end].trim_end_matches([',', ';', '.']);
    Some((word.len(), markdown_target(word)?))
}

pub(super) fn links(text: &str, mut found: impl FnMut(i32, i32, &str)) {
    let mut offset = 0;
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    for line in normalized.lines() {
        let mut rest = line;
        let mut boundary = true;
        while !rest.is_empty() {
            let ch = rest.chars().next().unwrap();
            if boundary || matches!(ch, '[' | '`') {
                if let Some((len, target)) = token(rest) {
                    let end = offset + rest[..len].encode_utf16().count() as i32;
                    found(offset, end, target);
                    rest = &rest[len..];
                    offset = end;
                    boundary = true;
                    continue;
                }
            }
            boundary = ch.is_whitespace() || "「『（(<：:、。".contains(ch);
            rest = &rest[ch.len_utf8()..];
            offset += ch.len_utf16() as i32;
        }
        offset += 1; // Rich Edit counts a paragraph break as one character.
    }
}

#[repr(C)]
struct CharFormat {
    size: u32,
    mask: u32,
    effects: u32,
    height: i32,
    offset: i32,
    color: u32,
    charset: u8,
    pitch: u8,
    face: [u16; 32],
}
pub(super) fn text_height(h:H,id:i32,start:i32)->i32 {
    let mut format:CharFormat=unsafe{std::mem::zeroed()};
    format.size=std::mem::size_of::<CharFormat>() as u32;
    msg(h,id,0xb1,start as usize,(start+1) as isize);
    msg(h,id,0x43a,1,&mut format as *mut _ as isize);
    format.height
}
pub fn style_ranges<T>(h:H,id:i32,ranges:&[(i32,i32,T)]) {
    let mut format = CharFormat {
        size: std::mem::size_of::<CharFormat>() as u32,
        mask: 32,
        effects: 0,
        height: 0,
        offset: 0,
        color: 0,
        charset: 0,
        pitch: 0,
        face: [0; 32],
    };
    msg(h, id, 0x445, 0, 0x04000000 | msg(h, id, 0x43b, 0, 0)); // ENM_LINK
    msg(h, id, 0xb1, 0, -1);
    msg(h, id, 0x444, 1, &format as *const _ as isize);
    format.effects = 32; // CFE_LINK
    for &(start,end,_) in ranges {
        msg(h, id, 0xb1, start as usize, end as isize);
        msg(h, id, 0x444, 1, &format as *const _ as isize);
    }
    msg(h, id, 0xb1, 0, 0);
}
#[derive(Debug)]
pub(super) enum LinkError {
    Missing(PathBuf),
    Outside,
    Read(String),
}

fn reference_scope(project: &Path) -> &Path {
    project.ancestors().find(|p| p.file_name().is_some_and(|n| n.eq_ignore_ascii_case(".company")))
        .unwrap_or(project)
}

// Resolve the existing ancestor as well, so a missing file under a junction
// is classified against the junction's destination.
fn location(path: &Path) -> std::io::Result<PathBuf> {
    match path.canonicalize() {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let parent = path.parent().ok_or(e)?;
            Ok(location(parent)?.join(path.file_name().ok_or_else(||
                std::io::Error::other("リンク先の名前を取得できません。"))?))
        }
        result => result,
    }
}

pub(super) fn resolve(task: &Task, target: &str) -> std::result::Result<PathBuf, LinkError> {
    let project = Path::new(&task.project).canonicalize().map_err(|e| LinkError::Read(e.to_string()))?;
    let scope = reference_scope(&project);
    let target = Path::new(target);
    let mut candidates = vec![project.join(target)];
    if !target.is_absolute() {
        candidates.push(Path::new(task.working_dir()).join(target));
        candidates.push(scope.join(target));
    }
    let mut inside = false;
    let mut error = None;
    for candidate in candidates {
        let path = match location(&migrated_path(&candidate)) {
            Ok(p) => p,
            Err(e) => { error = Some(e.to_string()); continue; }
        };
        let external_report = project_alias(&project) == Some("chromarch")
            && project.ancestors().last().is_some_and(|drive| chromarch_report(drive, &path));
        if path.starts_with(scope) || external_report {
            inside = true;
            if path.is_file() { return Ok(path); }
        }
    }
    if let Some(e) = error { Err(LinkError::Read(e)) }
    else if inside { Err(LinkError::Missing(scope.to_owned())) }
    else { Err(LinkError::Outside) }
}

fn moved_files(scope: &Path, name: &std::ffi::OsStr, cancel: &std::sync::atomic::AtomicBool) -> (Vec<PathBuf>, bool) {
    use std::os::windows::fs::MetadataExt;
    let mut pending = vec![scope.to_owned()];
    let mut files = Vec::new();
    let mut incomplete = false;
    while let Some(dir) = pending.pop() {
        let Ok(entries) = fs::read_dir(dir) else { incomplete = true; continue };
        for entry in entries {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) { return (Vec::new(), true); }
            let Ok(entry) = entry else { incomplete = true; continue };
            let Ok(meta) = entry.metadata() else { incomplete = true; continue };
            if meta.file_attributes() & 0x400 != 0 { continue; } // Do not follow reparse points.
            if meta.is_dir() { pending.push(entry.path()); }
            else if meta.is_file() && entry.file_name().eq_ignore_ascii_case(name) {
                files.push(entry.path());
            }
        }
    }
    files.sort();
    (files, incomplete)
}

struct Candidates {
    scope: PathBuf,
    target: String,
    results: std::sync::mpsc::Receiver<(Vec<PathBuf>, bool)>,
    files: Vec<PathBuf>,
}
#[link(name="user32")]
extern "system" {
    fn SetTimer(h: H, id: usize, ms: u32, cb: usize) -> usize;
    fn KillTimer(h: H, id: usize) -> i32;
}
extern "system" fn candidate_dialog(h: H, m: u32, w: usize, l: isize) -> isize {
    if m == 0x110 {
        native::round_fields(h);
        let state = unsafe { &*(l as *const Candidates) };
        set(h, 674, &state.target);
        unsafe {
            SetWindowLongPtrW(h,-21,l);
            EnableWindow(GetDlgItem(h,1),0);
            SetTimer(h,1,80,0);
        }
        return 1;
    }
    if m == 0x113 && w == 1 {
        let state = unsafe { &mut *(GetWindowLongPtrW(h,-21) as *mut Candidates) };
        let (files, incomplete) = match state.results.try_recv() {
            Ok(result) => result,
            Err(std::sync::mpsc::TryRecvError::Empty) => return 1,
            Err(_) => (Vec::new(), true),
        };
        unsafe { KillTimer(h,1); }
        state.files = files;
        let mut message = if state.files.is_empty() {
            "ファイルが見つかりません。同名の移動候補は見つかりませんでした。".to_owned()
        } else {
            format!("元の場所にファイルがありません。移動候補 {} 件から資料を選んでください。", state.files.len())
        };
        if incomplete { message.push_str(" 一部のフォルダーを検索できませんでした。"); }
        set(h,675,&message);
        for file in &state.files {
            let label = file.strip_prefix(&state.scope).unwrap_or(file).display().to_string();
            msg(h,676,0x180,0,wide(&label).as_ptr() as _);
        }
        #[cfg(test)]
        if let Some(action) = TEST_CANDIDATES.with(|cb| cb.borrow_mut().take()) { action(h); }
        return 1;
    }
    if m == 0x111 && w & 65535 == 676 && w >> 16 == 1 {
        let state = unsafe { &*(GetWindowLongPtrW(h,-21) as *const Candidates) };
        let chosen = state.files.get(msg(h,676,0x188,0,0) as usize);
        if let Some(file) = chosen { set(h,677,&file.display().to_string()); }
        unsafe { EnableWindow(GetDlgItem(h,1),chosen.is_some() as i32); }
        return 1;
    }
    if m == 0x111 && (w & 65535 == 1 || w == (2 << 16 | 676)) {
        let index = msg(h,676,0x188,0,0);
        if index >= 0 { unsafe { EndDialog(h,index+1); } }
        return 1;
    }
    if m == 0x10 || m == 0x111 && w & 65535 == 2 {
        unsafe { EndDialog(h,0); }
        return 1;
    }
    0
}

fn select_candidate(h: H, scope: PathBuf, target: &str) -> Option<PathBuf> {
    let name = Path::new(target).file_name()?.to_owned();
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop = cancel.clone();
    let root = scope.clone();
    let (send,results) = std::sync::mpsc::channel();
    std::thread::spawn(move || { let _ = send.send(moved_files(&root,&name,&stop)); });
    let mut state = Candidates { scope, target: target.to_owned(), results, files: Vec::new() };
    let index = unsafe { DialogBoxParamW(GetModuleHandleW(std::ptr::null()), 651 as Ptr, h,
        candidate_dialog, &mut state as *mut _ as isize) };
    cancel.store(true,std::sync::atomic::Ordering::Relaxed);
    if index == -1 { notice(h,"移動候補の画面を開けませんでした。"); }
    let chosen = state.files.get((index-1) as usize)?;
    match chosen.canonicalize() {
        Ok(file) if !file.starts_with(&state.scope) => notice(h,"選んだ資料は参照範囲外です。"),
        Ok(file) if file.is_file() => return Some(file),
        _ => notice(h,"選んだ資料を開けません。ファイルの場所を確認してください。"),
    }
    None
}
#[repr(C, packed(4))]
struct LinkEvent {
    hwnd: H,
    id: usize,
    code: u32,
    #[cfg(target_pointer_width = "64")]
    header_padding: u32,
    message: u32,
    w: usize,
    l: isize,
    start: i32,
    end: i32,
}
pub(super) fn clicked_position(l: isize) -> Option<i32> {
    let e = unsafe { &*(l as *const LinkEvent) };
    if e.code != 0x70b || !matches!(e.id, 414 | 409) || e.message != 0x202 {
        return None;
    }
    Some(e.start)
}
pub fn click(h: H, l: isize, task: &Task, rendered:&[(i32,i32,String)]) -> isize {
    let Some(position) = clicked_position(l) else { return 0; };
    let Some((_,_,target)) = rendered.iter().find(|(start,end,_)| *start<=position && position<*end)
        else { return 0 };
    if let Some(url)=web_url(target) {
        let null=std::ptr::null();
        if unsafe{ShellExecuteW(h,wide("open").as_ptr(),wide(url).as_ptr(),null,null,1)}<=32 {
            notice(h,"既定のブラウザーでリンクを開けませんでした。");
        }
        return 1;
    }
    let id = unsafe { (*(l as *const LinkEvent)).id as i32 };
    let mut scroll = [0i32;2];
    msg(h,id,0x4dd,0,scroll.as_mut_ptr() as isize); // EM_GETSCROLLPOS
    let file=path_without_line(target);
    let line=target.strip_prefix(file).and_then(|s|s.strip_prefix(':')).and_then(|n|n.parse::<usize>().ok());
    let path = match resolve(task,file) {
        Ok(path) => Some(path),
        Err(LinkError::Missing(scope)) => select_candidate(h,scope,file),
        Err(LinkError::Outside) => { notice(h,&format!("参照範囲外のMarkdownリンクです。\n{target}")); None }
        Err(LinkError::Read(err)) => { notice(h,&format!("リンク先を確認できません。\n{target}\n{err}")); None }
    };
    if let Some(path) = path {
        unsafe {
            let mut state = (path, native::Layout::default(),line);
            DialogBoxParamW(
                GetModuleHandleW(std::ptr::null()),
                650 as Ptr,
                h,
                preview,
                &mut state as *mut _ as isize,
            );
        }
    }
    msg(h,id,0x4de,0,scroll.as_ptr() as isize); // EM_SETSCROLLPOS
    1
}
#[cfg(test)]
thread_local! {pub(super) static TEST_CANDIDATES:RefCell<Option<fn(H)>>=const{RefCell::new(None)};}
#[cfg(test)]
thread_local! {pub(super) static TEST_PREVIEW:std::cell::RefCell<Option<fn(H)>>=const{std::cell::RefCell::new(None)};}
extern "system" fn preview(h: H, m: u32, w: usize, l: isize) -> isize {
    if m == 0x110 {
        let (path, layout,line) = unsafe { &mut *(l as *mut (PathBuf,native::Layout,Option<usize>)) };
        *layout = native::Layout::capture(h);
        unsafe { SetWindowLongPtrW(h,-21,l); }
        unsafe {
            SetWindowTextW(
                h,
                wide(&format!(
                    "{} — KAGARI",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ))
                .as_ptr(),
            );
        }
        display_document_at_line(h, Ok(path.clone()),*line);
        #[cfg(test)]
        if let Some(action) = TEST_PREVIEW.with(|cb| cb.borrow_mut().take()) {
            action(h);
        }
        return 1;
    }
    if m==0x111 && w&65535==672 && w>>16==1 {
        let index=msg(h,672,0x188,0,0); // LB_GETCURSEL
        if index<0 {return 1;}
        let pos=msg(h,672,0x199,index as usize,0); // LB_GETITEMDATA
        if pos>=0 {
            msg(h,120,0xb1,pos as usize,pos);
            msg(h,120,0xb7,0,0);
            let line=msg(h,120,0x436,0,pos);
            let first=msg(h,120,0xce,0,0);
            msg(h,120,0xb6,0,line-first);
        }
        return 1;
    }
    if matches!(m,5|0x24) {
        let ptr = unsafe {GetWindowLongPtrW(h,-21)} as *const (PathBuf,native::Layout,Option<usize>);
        if let Some((_,layout,_)) = unsafe {ptr.as_ref()} {
            if m==5 {layout.resize_task(h,true);} else {layout.minimum(l);}
        }
    }
    if m == 0x10 || m == 0x111 && w & 65535 == 2 {
        unsafe {
            EndDialog(h, 0);
        }
        return 1;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn moved_link_picker_opens_selected_file_and_preserves_scroll() {
        let root = std::env::temp_dir().join(format!("kagari-b7-{}", TaskId::default().0));
        let project = root.join("project");
        for (dir, body) in [("first", "資料その一"), ("second", "資料その二")] {
            fs::create_dir_all(project.join(dir)).unwrap();
            fs::write(project.join(dir).join("report.md"), body).unwrap();
        }
        let task = Task { project: project.display().to_string(), ..Default::default() };
        let missing = "old/report.md";
        assert!(matches!(resolve(&task, missing), Err(LinkError::Missing(_))));
        fs::write(root.join("report.md"), "範囲外").unwrap();
        for target in [root.join("report.md"), root.join("absent/report.md")] {
            assert!(matches!(resolve(&task, target.to_str().unwrap()), Err(LinkError::Outside)));
        }
        assert_eq!(resolve(&task, "first/report.md").unwrap(), project.join("first/report.md").canonicalize().unwrap());
        thread_local! { static CHECK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
        extern "system" fn exercise(h:H, m:u32, _:usize, l:isize) -> isize {
            if m != 0x110 { return 0; }
            let task = unsafe { &*(l as *const Task) };
            set(h,414,&"依頼の長い回答\r\n".repeat(200));
            msg(h,414,0xb6,0,60);
            let offset = || {
                let mut point = [0i32; 2];
                msg(h,414,0x4dd,0,point.as_mut_ptr() as isize);
                point
            };
            let before = offset();
            TEST_CANDIDATES.with(|cb| *cb.borrow_mut() = Some(|dialog| {
                let two = msg(dialog,676,0x18b,0,0)==2 && get(dialog,675).contains("移動候補 2 件");
                CHECK.with(|ok| ok.set(two));
                msg(dialog,676,0x186,1,0);
                unsafe { SendMessageW(dialog,0x111,(1<<16)|676,0); }
                eprintln!("rows={two}, chosen={:?}",get(dialog,677));
                CHECK.with(|ok| ok.set(ok.get() && get(dialog,677).ends_with(r"second\report.md")));
                unsafe { SendMessageW(dialog,0x111,1,0); }
                if !two { unsafe { EndDialog(dialog,0); } }
            }));
            TEST_PREVIEW.with(|cb| *cb.borrow_mut() = Some(|dialog| {
                CHECK.with(|ok| ok.set(ok.get() && get(dialog,120).contains("資料その二")));
                unsafe { EndDialog(dialog,0); }
            }));
            let mut event: LinkEvent = unsafe { std::mem::zeroed() };
            event.id=414;
            event.code=0x70b;
            event.message=0x202;
            click(h,&event as *const _ as isize,task,&[(0,8,"old/report.md".into())]);
            let shown = TEST_PREVIEW.with(|cb| cb.borrow().is_none());
            eprintln!("shown={shown}, verified={}, before={before:?}, after={:?}",CHECK.with(|ok|ok.get()),offset());
            let passed = CHECK.with(|ok| ok.get()) && shown && before[1]>0 && offset()==before;
            TEST_CANDIDATES.with(|cb| *cb.borrow_mut() = Some(|dialog| {
                CHECK.with(|ok| ok.set(msg(dialog,676,0x18b,0,0)==0 && get(dialog,675).contains("候補は見つかりません")));
                unsafe { EndDialog(dialog,0); }
            }));
            click(h,&event as *const _ as isize,task,&[(0,8,"absent.md".into())]);
            unsafe { EndDialog(h,(passed && CHECK.with(|ok| ok.get()) && offset()==before) as isize); }
            1
        }
        native::controls();
        let outcome = unsafe {
            LoadLibraryW(wide("Msftedit.dll").as_ptr());
            DialogBoxParamW(GetModuleHandleW(std::ptr::null()),300 as Ptr,0,exercise,&task as *const _ as isize)
        };
        fs::remove_dir_all(root).unwrap();
        assert_eq!(outcome,1);
    }
    #[test]
    fn external_reports_must_be_direct_children_of_a_bundle_on_the_project_drive() {
        let drive=Path::new(r"D:\");
        let file="QUEUE-ALL-CITIES-Q302-Q324-20261118.md";
        let bundle=drive.join("CHROMARCH-Shibuya-LocalBundle-20261117");
        assert!(chromarch_report(drive,&bundle.join(file)));
        assert!(!chromarch_report(drive,&bundle.join("nested").join(file)));
        assert!(!chromarch_report(drive,&bundle.join("unrelated.md")));
        assert!(!chromarch_report(drive,&drive.join("other").join(file)));
        assert!(!chromarch_report(Path::new(r"C:\"),&bundle.join(file)));
    }
    #[test]
    fn label_is_outside_the_clickable_drive_path() {
        let path = r"D:\.company\KasaneFactoryOS\ロードマップ_5年_月別タスク_20261121.md";
        for label in ["全文：", "全文： ", "全文:", "全文: "] {
            let input = format!("{label}{path}");
            let mut matches = Vec::new();
            links(&input, |start, _, target| {
                matches.push((start, target.to_owned()))
            });
            assert_eq!(
                matches,
                vec![(label.encode_utf16().count() as i32, path.to_owned())]
            );
        }
    }
    #[test]
    fn detects_the_same_japanese_document_in_both_reply_formats() {
        let text = r"`D:\.company\KasaneFactoryOS\月別タスク.md`
リンク：[月別タスク.md](KasaneFactoryOS/月別タスク.md)";
        let mut targets = Vec::new();
        links(text, |_, _, path| targets.push(path.to_owned()));
        assert_eq!(
            targets,
            [
                r"D:\.company\KasaneFactoryOS\月別タスク.md",
                "KasaneFactoryOS/月別タスク.md"
            ]
        );
    }
}
