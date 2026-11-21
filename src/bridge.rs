use super::*;
use std::fmt::Write;
#[link(name = "user32")]
extern "system" {
    fn CreateDialogParamW(inst: H, t: Ptr, parent: H, proc: Dialog, param: H) -> H;
    fn DestroyWindow(h: H) -> i32;
}
thread_local! {static XML:RefCell<Vec<u16>>=const{RefCell::new(Vec::new())};}
fn response(s: String) -> Ptr {
    XML.with(|v| {
        *v.borrow_mut() = wide(&s);
        v.borrow().as_ptr()
    })
}
fn xml(s: &str) -> String {
    s.chars()
        .filter(|c| *c >= ' ' || matches!(c, '\n' | '\r' | '\t'))
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
#[no_mangle]
pub extern "system" fn kagari_open(parent: H) -> H {
    native::controls();
    unsafe {
        LoadLibraryW(wide("Msftedit.dll").as_ptr());
    }
    if App::load().is_err() {
        return 0;
    }
    unsafe {
        let h = CreateDialogParamW(
            GetModuleHandleW(std::ptr::null()),
            100 as Ptr,
            parent,
            dialog,
            0,
        );
        ShowWindow(h, 0);
        h
    }
}
#[no_mangle]
pub extern "system" fn kagari_close(h: H) {
    unsafe {
        DestroyWindow(h);
    }
}
#[no_mangle]
pub extern "system" fn kagari_action(h: H, id: i32, index: i32, input: Ptr) -> Ptr {
    let text = if input.is_null() {
        String::new()
    } else {
        unsafe {
            let mut n = 0;
            while n < 32768 && *input.add(n) != 0 {
                n += 1
            }
            String::from_utf16_lossy(std::slice::from_raw_parts(input, n))
        }
    };
    let result=APP.with(|cell|->Result<String>{
 let mut a=cell.try_borrow_mut().map_err(|_|"処理中です")?;
 match id{
 0=>{},
 716=>{
    let project=a.projects.get(index as usize).ok_or("プロジェクトが見つかりません")?.to_lowercase();
    let mut out=String::from("<history>");
    for point in a.progress_history.iter().filter(|p|p.project==project) {
        let [a,b,c,d]=point.counts;
        let _=write!(out,r#"<point time="{}" a="{a}" b="{b}" c="{c}" d="{d}"/>"#,point.millis);
    }
    out.push_str("</history>");return Ok(out);
 },
 701=>{let i=index as usize;if i>=a.projects.len(){return Err("プロジェクトが見つかりません".into())}a.project=i;
    if Path::new(&a.projects[i]).file_name().is_some_and(|name|name.eq_ignore_ascii_case(".company")) {
        a.metric=0; a.agent_filter=0; a.query.clear(); set(h,182,""); msg(h,125,0x14e,0,0);
    }
    a.task=a.indices().first().copied();refresh(h,&a);render(h,&a);},
 702=>{let i=index as usize;if a.indices().contains(&i){a.task=Some(i);render(h,&a);}},
 703=>{set(h,182,&text);run(h,&mut a,182,0x300)?;},
 704=>{doc_path(a.document_root()?,&text)?;set(h,118,&text);preview(h,&a);attach_state(h,&a);},
 717=>{
    let root=a.projects.get(index as usize).ok_or("プロジェクトが見つかりません")?;
    let path=doc_path(root,&text)?;
    set(h,118,"");display_document(h,Ok(path));
 },
 705=>{set(h,123,&text);run(h,&mut a,122,0)?;},
 707=>{a.agent_filter=index.clamp(0,2) as usize;a.task=a.indices().first().copied();refresh(h,&a);render(h,&a);},
 715 => {
    let i=a.tasks.iter().position(|t| !text.is_empty() && (t.history_key==text || t.other_sessions.iter().any(|s|s.history_key==text)))
        .ok_or("通知元のタスクは削除されたか、見つかりません。")?;
    a.project=a.projects.iter().position(|p|p==&a.tasks[i].project).ok_or("プロジェクトが見つかりません")?;
    a.metric=0; a.query.clear(); a.agent_filter=0; a.task=Some(i);
    set(h,182,""); msg(h,125,0x14e,0,0); refresh(h,&a);render(h,&a);
 },
 714 => { a.delete_task(index as usize)?; refresh(h,&a); render(h,&a); },
 718 => {
    let project=a.projects.get(index as usize).ok_or("プロジェクトが見つかりません")?.clone();
    let mut next=a.clone();
    next.default_project=if a.default_project_index()==Some(index as usize) {None} else {Some(project)};
    next.save()?;
    *a=next;
 },
 712 | 713 => {
    let mut next = a.clone();
    if id == 712 {
        let key = next.projects.get(index as usize).ok_or("プロジェクトが見つかりません")?.to_lowercase();
        if !next.pinned_projects.remove(&key) { next.pinned_projects.insert(key); }
    } else {
        next.pinned_projects_only = index == 1;
    }
    next.save()?;
    *a = next;
 },
 711=>{
    let i=index as usize;
    let status=text.parse::<usize>().ok().filter(|&v|v<STATES.len()).ok_or("状態が不正です")?;
    if !a.indices().contains(&i){return Err("表示中のタスクを選んでください".into())}
    let mut next=a.clone();
    next.tasks[i].status=status;
    let visible=next.indices();
    next.task=Some(i).filter(|id|visible.contains(id)).or_else(||visible.first().copied());
    next.save()?;*a=next;refresh(h,&a);render(h,&a);
 },
 710=>{let mut next=a.clone();next.panes &= 2;next.save()?;*a=next;native::apply_layout(h,a.panes);panel(h,&a);attach_state(h,&a);},
 709=>{a.query.clear();set(h,182,"");run(h,&mut a,128,0)?;},
 708=>{register(h,&mut a,&text)?;},
 103|113|115|116|119|121|127|150..=153|156|159|160|175..=177|181|190|192|194..=200=>run(h,&mut a,id,0)?,
 _=>return Err("操作を確認してください".into())
 }
 let mut out=format!("<workspace version=\"{}\" defaultProject=\"{}\" pinnedOnly=\"{}\" project=\"{}\" task=\"{}\" panes=\"{}\" doc=\"{}\" metric=\"{}\" agent=\"{}\" query=\"{}\">",env!("CARGO_PKG_VERSION"),a.default_project_index().map_or(-1,|i|i as i64),a.pinned_projects_only,a.project,a.task.map(|i|i as i64).unwrap_or(-1),a.panes,a.doc,a.metric,a.agent_filter,xml(&a.query));
 let mut project_counts = std::collections::HashMap::new();
 for task in &a.tasks {
     let counts = project_counts.entry(task.project.as_str()).or_insert([0usize; 4]);
     counts[task.status] += 1;
 }
 for project in &a.projects {
     let counts = project_counts.get(project.as_str()).copied().unwrap_or_default();
     let _ = write!(out, r#"<project pinned="{}" a="{}" b="{}" c="{}" d="{}">{}</project>"#,
         a.pinned_projects.contains(&project.to_lowercase()), counts[0], counts[1], counts[2], counts[3], xml(project));
 }
 let scope=a.scope();for k in 0..4{let _=write!(out,"<metric>{}</metric>",scope.iter().filter(|i|matches_metric(&a.tasks[**i],k,today())).count());}
 for i in a.indices(){let t=&a.tasks[i];let _=write!(out,"<task id=\"{}\" project=\"{}\" title=\"{}\" agent=\"{}\" state=\"{}\" pin=\"{}\" due=\"{}\" reason=\"{}\"/>",i,xml(&t.project),xml(&t.title),t.agent,t.status,t.pinned,xml(&t.due),priority(t,today()).1);}
 let path=get(h,118);let can_attach=a.task.is_some_and(|i|!path.is_empty()&&path!=a.tasks[i].docs[a.doc]);let _=write!(out,"<preview path=\"{}\" attach=\"{}\">{}</preview></workspace>",xml(&path),can_attach,xml(&get(h,120)));
 Ok(out)});
    response(result.unwrap_or_else(|e| format!("<error>{}</error>", xml(&e))))
}
#[repr(C, packed(4))]
struct Stream {
    cookie: usize,
    error: u32,
    callback: extern "system" fn(usize, *const u8, i32, *mut i32) -> u32,
}
extern "system" fn receive(cookie: usize, bytes: *const u8, n: i32, done: *mut i32) -> u32 {
    unsafe {
        let out = &mut *(cookie as *mut Vec<u8>);
        out.extend_from_slice(std::slice::from_raw_parts(bytes, n as usize));
        *done = n;
    }
    0
}
thread_local! {static RTF:RefCell<Vec<u8>>=const{RefCell::new(Vec::new())};}
#[no_mangle]
pub extern "system" fn kagari_rtf(h: H, length: *mut i32) -> *const u8 {
    RTF.with(|v| {
        let mut buf = v.borrow_mut();
        buf.clear();
        let mut stream = Stream {
            cookie: &mut *buf as *mut _ as usize,
            error: 0,
            callback: receive,
        };
        msg(h, 120, 0x44a, 2, &mut stream as *mut _ as _);
        unsafe {
            *length = buf.len() as i32;
        }
        buf.as_ptr()
    })
}
