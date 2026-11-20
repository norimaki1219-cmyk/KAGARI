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
 701=>{let i=index as usize;if i>=a.projects.len(){return Err("プロジェクトが見つかりません".into())}a.project=i;a.task=a.indices().first().copied();refresh(h,&a);render(h,&a);},
 702=>{let i=index as usize;if a.indices().contains(&i){a.task=Some(i);render(h,&a);}},
 703=>{set(h,182,&text);run(h,&mut a,182,0x300)?;},
 704=>{doc_path(a.root()?,&text)?;set(h,118,&text);preview(h,&a);attach_state(h,&a);},
 705=>{set(h,123,&text);run(h,&mut a,122,0)?;},
 706=>{a.unassigned=index==1;a.department_filter=text;a.metric=0;a.task=a.indices().first().copied();refresh(h,&a);render(h,&a);},
 707=>{a.agent_filter=index.clamp(0,2) as usize;a.task=a.indices().first().copied();refresh(h,&a);render(h,&a);},
 709=>{a.query.clear();set(h,182,"");run(h,&mut a,128,0)?;},
 708=>{register(h,&mut a,&text)?;},
 103|113|115|116|119|121|127|150..=153|156|159|160|175..=177|181|190|192|194..=200=>run(h,&mut a,id,0)?,
 _=>return Err("操作を確認してください".into())
 }
 let mut out=format!("<workspace project=\"{}\" task=\"{}\" panes=\"{}\" doc=\"{}\" metric=\"{}\" agent=\"{}\" filter=\"{}\" unassigned=\"{}\" query=\"{}\">",a.project,a.task.map(|i|i as i64).unwrap_or(-1),a.panes,a.doc,a.metric,a.agent_filter,xml(&a.department_filter),a.unassigned,xml(&a.query));
 for p in &a.projects{let _=write!(out,"<project>{}</project>",xml(p));}
 let mut depts=std::collections::BTreeMap::new();
 for t in &a.tasks{if Some(&t.project)==a.projects.get(a.project)&&(a.agent_filter==0||a.agent_filter==t.agent+1){let counts=depts.entry(&t.dept).or_insert([0;4]);counts[t.status]+=1;}}
 for (dept,c) in depts {let _=write!(out,"<dept name=\"{}\" a=\"{}\" b=\"{}\" c=\"{}\" d=\"{}\"/>",xml(dept),c[0],c[1],c[2],c[3]);}
 let scope=a.scope();for k in 0..4{let _=write!(out,"<metric>{}</metric>",scope.iter().filter(|i|matches_metric(&a.tasks[**i],k,today())).count());}
 for i in a.indices(){let t=&a.tasks[i];let _=write!(out,"<task id=\"{}\" title=\"{}\" agent=\"{}\" state=\"{}\" pin=\"{}\" dept=\"{}\" due=\"{}\" reason=\"{}\"/>",i,xml(&t.title),t.agent,t.status,t.pinned,xml(&t.dept),xml(&t.due),priority(t,today()).1);}
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
