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
// /usage refreshes the signed-in CLI's local utilization cache without an AI turn.
#[no_mangle]
pub extern "system" fn kagari_refresh_claude_usage()->i32 {
    use std::{process::Stdio,time::{Duration,Instant}};
    let Ok(cli)=client::find(1) else{return 1};
    let Ok(mut child)=Command::new(cli)
        .args(["-p","/usage","--output-format","json","--no-session-persistence","--tools","","--max-turns","0"])
        .current_dir(std::env::temp_dir()).creation_flags(0x08000000)
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() else{return 2};
    let start=Instant::now();
    loop {
        match child.try_wait(){
            Ok(Some(status))=>return if status.success(){0}else{2},
            Ok(None) if start.elapsed()<Duration::from_secs(25)=>std::thread::sleep(Duration::from_millis(100)),
            _=>{let _=child.kill();let _=child.wait();return 3;}
        }
    }
}
#[no_mangle]
pub extern "system" fn kagari_open(parent: H) -> H {
    native::controls();
    unsafe {
        LoadLibraryW(wide("Msftedit.dll").as_ptr());
    }
    if let Err(error) = App::load() {
        notice(parent, &error);
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
  0=>{queue_import::collect(&mut a)?;},
 719=>{
    if queue_import::collect(&mut a)? { return Ok("<statuses changed=\"1\"/>".into()); }
    let mut out=String::from("<statuses>");
    for i in a.indices(){
        let (label,detail)=ask::queue_status(&a.tasks[i]);
        let detail = xml(&detail).replace("\r\n", "\n").replace('\n', "&#10;");
        let _=write!(out,r#"<status id="{i}" label="{}" detail="{detail}"/>"#,xml(&label));
    }
    if index == 1 {
        let (counts, _) = task_counts::task_counts(&a.tasks);
        for (i, project) in a.projects.iter().enumerate() {
            let [unstarted, _, _, _, followup] = counts.get(project.as_str()).copied().unwrap_or_default();
            let _ = write!(out, r#"<progress id="{i}" a="{unstarted}" e="{followup}"/>"#);
        }
    }
    out.push_str("</statuses>");return Ok(out);
 },
 760|761|762=>{
    let mut out=String::from("<approvals>");
    let (wanted,shown)=text.split_once('\n').unwrap_or(("", ""));
    for task in &a.tasks {
        for (key,prompt) in ask::task_approvals(task) {
            if id==760 {
                let summary=xml(&prompt.summary).replace('\r',"&#13;");
                let _=write!(out,r#"<approval key="{}" task="{}">{summary}</approval>"#,xml(&key),xml(&task.title));
            } else if key==wanted && prompt.summary==shown {
                prompt.respond(id==761)?;
                return Ok("<ok/>".into());
            }
        }
    }
    if id!=760 {return Err("この承認要求は変更または終了しました。".into());}
    out.push_str("</approvals>");return Ok(out);
 },
 765=>{
    let (key,value)=text.split_once('\n').ok_or("完了条件の対象を確認できません")?;
    let key=TaskId::parse(key)?;
    let value=value.trim();
    if value.chars().count()>240 || value.chars().any(char::is_control){return Err("完了条件は240文字以内の1行で入力してください".into())}
    let mut next=a.clone();
    next.tasks.iter_mut().find(|t|t.id==key).ok_or("タスクが見つかりません")?.completion_condition=value.into();
    next.save()?;*a=next;
 },
 764=>{
    let project=a.projects.iter().position(|p|Path::new(p).file_name().is_some_and(|n|n.eq_ignore_ascii_case("SHIRANUI")))
        .ok_or("SHIRANUIプロジェクトが登録されていません")?;
    let mut fields=text.splitn(3,'\n');
    let alert=fields.next().and_then(|v|v.parse::<u64>().ok()).filter(|n|*n>0).ok_or("検知IDを取得できません")?;
    let name=fields.next().unwrap_or("").trim();
    let evidence=fields.next().unwrap_or("");
    let id=TaskId((u64::from_be_bytes(*b"SHIRANUI") as u128)<<64 | alert as u128);
    let mut next=a.clone();
    let i=if let Some(i)=next.tasks.iter().position(|t|t.id==id){i}else{
        next.tasks.push(Task{id,project:next.projects[project].clone(),title:format!("検知 #{alert}　{name}"),
            next_action:format!("SHIRANUIの検知 #{alert} を調査してください。以下は検知記録です。\n\n{evidence}"),..Default::default()});
        next.tasks.len()-1
    };
    next.project=project;next.task=Some(i);next.metric=0;next.agent_filter=0;next.query.clear();next.queue_filter=QueueFilter::All;
    next.save()?;*a=next;set(h,182,"");msg(h,125,0x14e,0,0);refresh(h,&a);render(h,&a);
 },
 763=>{
    let project=a.projects.get(index as usize).ok_or("登録先を選んでください")?.clone();
    let mut fields=text.splitn(4,'\n');
    let title=fields.next().unwrap_or("").trim();
    let review=fields.next().unwrap_or("");
    let goal=fields.next().unwrap_or("");
    let action=fields.next().unwrap_or("");
    if title.is_empty() || parse_date(review).is_none() || goal.is_empty(){return Err("改善タスクの入力を確認してください".into());}
    let mut next=a.clone();
    next.tasks.push(Task{project,title:title.into(),check:review.into(),kpi_goal:goal.into(),next_action:action.into(),..Default::default()});
    next.project=index as usize;next.task=Some(next.tasks.len()-1);
    next.metric=0;next.agent_filter=0;next.query.clear();next.queue_filter=QueueFilter::All;
    next.save()?;*a=next;set(h,182,"");msg(h,125,0x14e,0,0);refresh(h,&a);render(h,&a);
 },
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
         a.metric=0; a.agent_filter=0; a.query.clear(); a.queue_filter=QueueFilter::All; set(h,182,""); msg(h,125,0x14e,0,0);
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
  723=>{
     a.queue_filter = match index {
         0 => QueueFilter::All,
         1 => QueueFilter::Working,
         2 => QueueFilter::AiRunning,
         3 => QueueFilter::Reviewing,
         _ => return Err("作業キューの条件が不正です".into()),
     };
     let visible = a.indices();
     a.task = a.task.filter(|i| visible.contains(i)).or_else(|| visible.first().copied());
     refresh(h, &a);
     render(h, &a);
  },
 745 => {
    let project=a.projects.get(index as usize).ok_or("プロジェクトが見つかりません")?;
    let mut out=String::from("<tasks>");
    for t in a.tasks.iter().filter(|t|t.project.eq_ignore_ascii_case(project)) {
        let _=write!(out,r#"<task key="{}" parent="{}" title="{}" state="{}">"#,t.id,t.parent.map(|p|p.to_string()).unwrap_or_default(),xml(&t.title),t.status);
        for (kind,path) in ["仕様書","作業ログ","結果"].iter().zip(&t.docs) {
            if !path.is_empty(){let _=write!(out,r#"<doc kind="{kind}">{}</doc>"#,xml(path));}
        }
        out.push_str("</task>");
    }
    out.push_str("</tasks>");return Ok(out);
 },
 715 | 746 => {
    let i=if id==746 {a.tasks.iter().position(|t|t.id.to_string()==text)} else {a.notification_row(&text)}
        .ok_or("通知元のタスクは削除されたか、見つかりません。")?;
    a.project=a.projects.iter().position(|p|p==&a.tasks[i].project).ok_or("プロジェクトが見つかりません")?;
     a.metric=0; a.query.clear(); a.agent_filter=0; a.queue_filter=QueueFilter::All; a.task=Some(i);
    set(h,182,""); msg(h,125,0x14e,0,0); refresh(h,&a);render(h,&a);
 },
 720 => { editor::open_with_parent(h,&mut a,true,false,Some(index as usize))?; },
 721 => {
    let parent=if text=="root"{None}else{Some(text.parse::<usize>().map_err(|_|"親タスクが不正です")?)};
    let mut next=a.clone();next.set_parent(index as usize,parent)?;next.save()?;*a=next;
 },
 722 => {
    let mut out=String::from("<parents>");
    for (p,t) in a.tasks.iter().enumerate().filter(|(p,_)|a.can_parent(index as usize,*p)){
        let _=write!(out,"<parent id=\"{p}\">{}</parent>",xml(&t.title));
    }
    out.push_str("</parents>");return Ok(out);
 },
 741 | 742 => {
    let mut next=a.clone();
    if id==741 {
        let undo=bulk_edit::apply(&mut next,&text)?;
        next.save()?;
        next.bulk_undo=Some((next.project,undo));
    } else {
        bulk_edit::undo(&mut next)?;
        next.save()?;
    }
    let visible=next.indices();
    next.task=next.task.filter(|i|visible.contains(i)).or_else(||visible.first().copied());
    *a=next;refresh(h,&a);render(h,&a);
 },
 747 => {
    let (title,note)=text.split_once('\n').ok_or("記事の見出しが必要です")?;
    editor::open_seeded(h,&mut a,true,false,None,Some((title,note)))?;
 },
 743 => {
    let ids=bulk_edit::selection(&a,&text)?;
    let mut out=String::from("<parents>");
    for (p,t) in a.tasks.iter().enumerate().filter(|(p,_)|ids.iter().all(|&i|a.can_parent(i,*p))){
        let _=write!(out,"<parent id=\"{p}\">{}</parent>",xml(&t.title));
    }
    out.push_str("</parents>");return Ok(out);
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
  let matches=a.indices();
  if a.queue_filter != QueueFilter::All && !matches!(id, 704 | 717) {
      let selected = a.task.filter(|i| matches.contains(i)).or_else(|| matches.first().copied());
      if selected != a.task {
          a.task = selected;
          render(h, &a);
      }
  }
  let mut out=format!("<workspace version=\"{}\" defaultProject=\"{}\" pinnedOnly=\"{}\" project=\"{}\" task=\"{}\" panes=\"{}\" doc=\"{}\" metric=\"{}\" agent=\"{}\" queueFilter=\"{}\" query=\"{}\">",env!("CARGO_PKG_VERSION"),a.default_project_index().map_or(-1,|i|i as i64),a.pinned_projects_only,a.project,a.task.map(|i|i as i64).unwrap_or(-1),a.panes,a.doc,a.metric,a.agent_filter,a.queue_filter as usize,xml(&a.query));
 let undo_count=a.bulk_undo.as_ref().filter(|(p,_)|*p==a.project).map_or(0,|(_,tasks)|tasks.len());
 let _=write!(out,"<bulkUndo>{undo_count}</bulkUndo>");
 let (project_counts, child_counts) = task_counts::task_counts(&a.tasks);
 for project in &a.projects {
     let counts = project_counts.get(project.as_str()).copied().unwrap_or_default();
     let [a0,b,c,d,e] = counts;
     let _ = write!(out, r#"<project alias="{}" pinned="{}" a="{a0}" b="{b}" c="{c}" d="{d}" e="{e}">{}</project>"#,
         project_alias(Path::new(project)).unwrap_or(""), a.pinned_projects.contains(&project.to_lowercase()), xml(project));
 }
 let scope=a.scope();for k in 0..4{let _=write!(out,"<metric>{}</metric>",scope.iter().filter(|i|matches_metric(&a.tasks[**i],k,today())).count());}
 for (id,r) in a.queue_receipts.iter().enumerate().rev().filter(|(_,r)|a.projects.get(a.project)==Some(&r.project)){
     let parent=a.tasks.iter().position(|t|!r.parent_key.is_empty() && t.project==r.project
         && (t.history_key==r.parent_key || t.other_sessions.iter().any(|s|s.history_key==r.parent_key)));
     let _=write!(out,r#"<receipt id="{id}" parent="{}" name="{}" key="{}" added="{}" existing="{}" failed="{}">"#,
         parent.map_or(-1,|i|i as i64),xml(parent.map_or("",|i|a.tasks[i].title.as_str())),xml(&r.parent_key),r.added.len(),r.existing,r.failed);
     for title in &r.added{let _=write!(out,"<added>{}</added>",xml(title));}
     out.push_str("</receipt>");
 }
 let rows:std::collections::HashMap<_,_>=a.tasks.iter().enumerate().map(|(row,t)|(t.id,row)).collect();
 let parent_row=|t:&Task| t.parent.and_then(|id|rows.get(&id).copied());
 let row_info=|t:&Task| {
     let counts=child_counts.get(&t.id).copied().unwrap_or_default();
     format!(r#" key="{}" condition="{}" goal="{}" state="{}" children="{}" childrenDone="{}" childrenWaiting="{}""#,
       t.id,xml(&t.completion_condition),xml(&t.kpi_goal),t.status,counts.iter().sum::<usize>(),counts[3],counts[2])
 };
 let mut included:std::collections::HashSet<_>=matches.iter().copied().collect();
 for &i in &matches {
     let mut ancestor=parent_row(&a.tasks[i]);
     while let Some(p)=ancestor {
         if !included.insert(p){break;}
         let parent=&a.tasks[p];
         let _=write!(out,r#"<ancestor id="{p}" parent="{}" title="{}"{}/>"#,parent_row(parent).map_or(-1,|id|id as i64),xml(&parent.title),row_info(parent));
         ancestor=parent_row(parent);
     }
 }
 for i in matches{
     let t=&a.tasks[i];
     if !t.kpi_goal.is_empty(){let _=write!(out,r#"<kpiGoal task="{i}">{}</kpiGoal>"#,xml(&t.kpi_goal));}
     let _=write!(out,r#"<task id="{}" parent="{}" project="{}" title="{}" agent="{}" pin="{}" due="{}" reason="{}"{}/>"#,i,parent_row(t).map_or(-1,|p|p as i64),xml(&t.project),xml(&t.title),t.agent,t.pinned,xml(&t.due),priority(t,today()).1,row_info(t));
 }
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

#[no_mangle]
pub extern "system" fn kagari_checklist_rtf(h: H, source: *const u8, size: usize, token: u32, length: *mut i32) -> *const u8 {
    let bytes = unsafe { std::slice::from_raw_parts(source, size) };
    let Ok(text) = std::str::from_utf8(bytes) else { return std::ptr::null() };
    let marker = format!("KagariCheck{token}Z");
    let preview = markdown_preview::Preview::document_with_checks(text, native::document_width(h,120), &marker);
    RTF.with(|cell| {
        let mut buf = cell.borrow_mut();
        *buf = preview.rtf.into_bytes();
        unsafe { *length = buf.len() as i32; }
        buf.as_ptr()
    })
}
