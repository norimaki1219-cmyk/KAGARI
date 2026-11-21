use super::*;
#[link(name = "user32")]
extern "system" {
    fn PostMessageW(h:H,m:u32,w:usize,l:isize)->i32;
    fn GetWindowLongPtrW(h: H, i: i32) -> isize;
    fn SetTimer(h: H, id: usize, ms: u32, cb: usize) -> usize;
    fn KillTimer(h: H, id: usize) -> i32;
    fn SetWindowLongPtrW(h: H, i: i32, v: isize) -> isize;
}
struct Edit<'a> {
    app: &'a mut App,
    index: Option<usize>,
    draft: Task,
    pending: Option<std::sync::mpsc::Receiver<Option<Reply>>>,
    reply_time: u128,
    ask: ask::Ask,
    #[cfg(test)]
    focus_request: bool,
    details: bool,
    full_reply: bool,
    summary: String,
    full: String,
    form:Option<decisions::Form>,
    questions:Vec<decisions::Question>,
    detail_links:Vec<(i32,i32,String)>,
    layout: native::Layout,
    rendered_links: Vec<(i32,i32,String)>,
}
impl Edit<'_> {
    fn show_page(&self, h: H) {
        let display = |id, visible| unsafe { ShowWindow(GetDlgItem(h,id), if visible {5} else {0}); };
        let answering=self.form.is_some() && !self.details;
        for id in [420,414,435,421] { display(id,!self.details); }
        for id in [403,404,438] { display(id,!self.details && !answering); }
        for id in [681,683,684,685,686] {display(id,answering);}
        set(h,421,if answering {"決めること"}else{"次のアクション（依頼）"});
        set(h,439,if answering {"回答を反映".into()}else{format!("決定事項に回答（{}）",self.questions.len())}.as_str());
        for id in (422..=429).chain([431,432]).chain([107,108,109,110,111,157,158,402,405,406,408,409,411,412,415]) {
            display(id,self.details);
        }
        display(437,self.details);
        self.show_decisions(h);
        set(h,416,if self.details {"サマリに戻る"} else {"設定・履歴"});
        visual::set_action_icon(h,416,if self.details {39} else {40});
        unsafe { SetFocus(GetDlgItem(h,if self.details {107} else if answering {681} else {403})); }
    }
    fn session_labels(&self)->Vec<String> {
        let t=&self.draft;
        let label=|agent:usize,title:&str,id:&str| {
            if id.is_empty(){format!("{}　ローカルの依頼",AGENTS[agent])}
            else{format!("{}　{}　({})",AGENTS[agent],title,id)}
        };
        let mut labels=vec![label(t.agent,&t.session_title,&t.session)];
        labels.extend(t.other_sessions.iter().map(|s|label(s.agent,&s.title,&s.id)));
        labels
    }
    fn display_answer(&mut self,h:H) {
        let title=if self.full_reply {"最新のAI回答（全文）"} else {"やり取りのサマリ"};
        set(h,420,&format!("{title}{}",reply_received_at(self.reply_time)));
        set(h,435,if self.full_reply {"サマリに戻す"} else {"回答全文を表示"});
        visual::set_action_icon(h,435,if self.full_reply {39} else {38});
        let shown=if self.full_reply {&self.full} else {&self.summary};
        self.rendered_links = markdown_preview::Preview::document(shown,native::document_width(h,414)).show_answer(h,414);
    }
    fn show_decisions(&self,h:H){
        visual::set_action_icon(h,439,if self.form.is_some(){37}else{36});
        unsafe {ShowWindow(GetDlgItem(h,439),if !self.details && !self.questions.is_empty(){5}else{0});}
    }
    fn set_full(&mut self,h:H,text:&str) {
        let questions=decisions::parse(text);
        if self.questions!=questions {
            let was_open=self.form.is_some();
            self.apply_form(h);
            self.questions=questions;
            if was_open {self.show_page(h);} else {
                set(h,439,&format!("決定事項に回答（{}）",self.questions.len()));
                self.show_decisions(h);
            }
        }
        self.full=display_reply(text);
        self.detail_links=markdown_preview::Preview::document(text,native::document_width(h,409)).show_answer(h,409);
    }
    fn load_reply(&mut self,h:H){
        self.pending=None;
        unsafe{KillTimer(h,1);}
        let t=&self.draft;
        let local = ask::latest_reply(t);
        self.reply_time = local.as_ref().map_or(0, |r| r.time);
        let placeholder = if t.session.is_empty() {"回答はまだありません"} else {"回答を読み込み中…"};
        self.set_full(h,local.as_ref().map_or(placeholder, |r| &r.text));
        self.summary = display_reply(&local.as_ref().map(action_summary::conversation).unwrap_or_else(||placeholder.into()));
        self.display_answer(h);
        if !self.draft.session.is_empty() {
            let (agent, session) = (self.draft.agent, self.draft.session.clone());
            let (tx, rx) = std::sync::mpsc::channel();
            self.pending = Some(rx);
            std::thread::spawn(move || {let _ = tx.send(sessions::latest_reply(agent, &session));});
            unsafe { SetTimer(h, 1, 150, 0); }
        }
    }
    fn clear_completed_action(&mut self, h: H) {
        let Some(i) = self.index else { return; };
        let stored = &self.app.tasks[i];
        if !ask::action_completed(stored) { return; }
        let sent = stored.next_action.clone();
        let mut next = self.app.clone();
        next.tasks[i].submitted_action_run.clear();
        next.tasks[i].next_action.clear();
        if next.save().is_err() { return; }
        *self.app = next;
        self.draft.submitted_action_run.clear();
        if self.draft.next_action == sent { self.draft.next_action.clear(); }
        if get(h,403) == sent { set(h,403,""); }
    }
    fn apply_form(&mut self,h:H){if let Some(form)=self.form.take(){form.apply(h);}}
    fn read_fields(&mut self, h: H) -> Result<()> {
        if self.form.is_some(){self.apply_form(h);self.show_page(h);}
        let due = get(h, 110);
        if !due.is_empty() && parse_date(&due).is_none() {
            return Err("期限は YYYY-MM-DD で入力してください".into());
        }
        let action = get(h,403);
        if self.draft.next_action != action { self.draft.submitted_action_run.clear(); }
        self.draft.next_action = action;
        let title = get(h, 107);
        self.draft.title = if title.trim().is_empty() {
            self.draft.next_action.lines().map(str::trim).find(|s| !s.is_empty())
                .unwrap_or(REQUESTS[0]).chars().take(80).collect()
        } else {
            title.trim().into()
        };
        let agent = choice(h, 108);
        if agent != self.draft.agent {
            self.draft.model.clear();
            self.draft.effort.clear();
        }
        if !self.draft.session.is_empty() && agent != self.draft.agent {
            return Err("このタスクは元のセッションの担当AIで再開します".into());
        }
        self.draft.agent = agent;
        self.draft.status = choice(h, 109);
        self.draft.due = due;
        self.draft.check = get(h, 111);
        let model = get(h, 402);
        let model = model.trim();
        self.draft.model = if model == ask::DEFAULT { String::new() } else { model.into() };
        let effort = ask::efforts(agent)[choice(h, 408)];
        self.draft.effort = if effort == ask::DEFAULT { String::new() } else { effort.into() };
        Ok(())
    }
    fn submit(&mut self, h: H) -> Result<()> {
        self.read_fields(h)?;
        let mut next = self.app.clone();
        let i = self.index.unwrap_or(next.tasks.len());
        if self.index.is_some() {
            next.tasks[i] = self.draft.clone()
        } else {
            next.tasks.push(self.draft.clone())
        }
        next.save()?;
        self.index = Some(i);
        next.task = Some(i);
        *self.app = next;

        Ok(())
    }
}
pub fn open(owner: H, a: &mut App, new: bool) -> Result<()> {
    open_at(owner, a, new, false)
}
pub(super) fn open_at(owner: H, a: &mut App, new: bool, _focus_request: bool) -> Result<()> {
    let project = a.root()?.to_string();
    let index = if new {
        None
    } else {
        Some(a.task.ok_or("編集するタスクを選んでください")?)
    };
    let draft = index.map(|i| a.tasks[i].clone()).unwrap_or_else(|| Task {
        project,
        agent: a.agent_filter.saturating_sub(1),
        ..Default::default()
    });
    let mut e = Edit {
        app: a,
        index,
        draft,
        pending: None,
        reply_time: 0,
        ask: ask::Ask::default(),
        #[cfg(test)]
        focus_request: _focus_request,
        details: false,
        full_reply: false,
        summary: String::new(),
        full: String::new(),
        questions:Vec::new(),
        form:None,
        detail_links:Vec::new(),
        layout: Default::default(),
        rendered_links: Vec::new(),
    };
    let result = unsafe {
        DialogBoxParamW(
            GetModuleHandleW(std::ptr::null()),
            300 as Ptr,
            owner,
            dialog_proc,
            &mut e as *mut _ as isize,
        )
    };
    if result < 0 {
        return Err("タスク詳細を開けません".into());
    }
    e.ask.clear_images();
    if result > 0 && !a.task.is_some_and(|i| a.indices().contains(&i)) {
        a.task = a.indices().first().copied();
    }
    refresh(owner, a);
    render(owner, a);
    if result > 0 {
        set(owner, 141, "保存しました");
    }
    Ok(())
}
#[cfg(test)]
pub(super) type TestAction = fn(H);
#[cfg(test)]
thread_local! {pub(super) static TEST_ACTION:RefCell<Option<TestAction>>=const{RefCell::new(None)};}
fn reply_received_at(millis:u128)->String {
    #[link(name="kernel32")]
    extern "system"{
        fn FileTimeToSystemTime(time:*const u64,out:*mut [u16;8])->i32;
        fn SystemTimeToTzSpecificLocalTime(zone:usize,utc:*const [u16;8],out:*mut [u16;8])->i32;
    }
    if millis==0{return String::new();}
    let Some(filetime)=millis.checked_add(11644473600000).and_then(|n|n.checked_mul(10000)).and_then(|n|u64::try_from(n).ok()) else{return String::new();};
    let mut utc=[0;8];let mut local=[0;8];
    if unsafe{FileTimeToSystemTime(&filetime,&mut utc)!=0 && SystemTimeToTzSpecificLocalTime(0,&utc,&mut local)!=0}{
        format!("　　回答日時 {:02}/{:02} {:02}:{:02}",local[1],local[3],local[4],local[5])
    }else{String::new()}
}
fn display_reply(text: &str) -> String {text.replace("\r\n","\n").replace('\n',"\r\n")}
extern "system" fn dialog_proc(h: H, m: u32, w: usize, l: isize) -> isize {
    if matches!(m, 5 | 0x24) {
        let ptr = unsafe { GetWindowLongPtrW(h,-21) } as *const Edit;
        if let Some(e) = unsafe {ptr.as_ref()} {
            if m == 5 { e.layout.resize_task(h,false); } else { e.layout.minimum(l); }
        }
        return 0;
    }
    if m == 0x4e && l!=0 && matches!(w,414|409) {
        let ptr=unsafe{GetWindowLongPtrW(h,-21)} as *const Edit;
        if let Some(e)=unsafe{ptr.as_ref()} { let task=e.draft.clone(); return reply_links::click(h,l,&task,if w==414 {&e.rendered_links} else {&e.detail_links}); }
    }
    const UPDATE_SEND:u32=0x8000+112;
    if m==0x111 && w&65535==403 && w>>16==0x300{
        unsafe{PostMessageW(h,UPDATE_SEND,0,0);}
        return 0;
    }
    if m==UPDATE_SEND{
        let ptr=unsafe{GetWindowLongPtrW(h,-21)} as *const Edit;
        if let Some(e)=unsafe{ptr.as_ref()}{e.ask.update_send(h);}
        return 1;
    }
    if m==0x2b && w==404 {visual::draw_send(h,l);return 1;}
    if m == 0x138 { return visual::task_caption(h,w,l); }
    if m == 0x2b && w == 108 {
        brand::draw(l);
        return 1;
    }
    if m == 0x110 {
        unsafe {
            SetWindowLongPtrW(h, -21, l);
        }
        let e = unsafe { &mut *(l as *mut Edit) };
        visual::set_action_icon(h,114,34);
        visual::set_action_icon(h,438,35);
        e.layout = native::Layout::capture(h);
        let labels = e.session_labels();
        options(h,415,&labels.iter().map(String::as_str).collect::<Vec<_>>());
        unsafe {EnableWindow(GetDlgItem(h,415),(labels.len()>1) as i32);}
        e.load_reply(h);
        let t=&e.draft;
        options(h, 107, &REQUESTS);
        options(h, 108, &AGENTS);
        options(h, 109, &STATES);
        for (id, value) in [
            (107, &t.title),
            (110, &t.due),
            (111, &t.check),
        ] {
            set(h, id, value)
        }
        msg(h, 108, 0x14e, t.agent, 0);
        msg(h, 109, 0x14e, t.status, 0);
        e.ask.init(h, t);
        unsafe {
            SetTimer(h, 2, 2000, 0);
            SetWindowTextW(
                h,
                wide(&format!("{} — KAGARI", if t.title.is_empty() {"新規タスク"} else {&t.title})).as_ptr(),
            );
        }
        e.clear_completed_action(h);
        e.show_page(h);
        #[cfg(test)]
        {
            let action = if e.focus_request { ask::TEST_ACTION.with(|cell| cell.borrow_mut().take()) } else { TEST_ACTION.with(|cell| cell.borrow_mut().take()) };
            if let Some(action) = action {
                action(h);
            }
        }
        return 0;
    }
    if m==0x111 && matches!(w&65535,681|683) && w>>16==1 {
        let e=unsafe{&mut *(GetWindowLongPtrW(h,-21) as *mut Edit)};
        if let Some(form)=e.form.as_mut(){form.select(h,(w&65535) as i32);}
        return 1;
    }
    if m==0x111 && w & 65535==415 && w >> 16==1 {
        let ptr=unsafe{GetWindowLongPtrW(h,-21)} as *mut Edit;
        if let Some(e)=unsafe{ptr.as_mut()} {
            let index=choice(h,415);
            if index>0 && index<=e.draft.other_sessions.len(){
                e.draft.activate(index-1);
                e.ask.bind(h,&e.draft);
                let labels=e.session_labels();
                options(h,415,&labels.iter().map(String::as_str).collect::<Vec<_>>());
                msg(h,108,0x14e,e.draft.agent,0);
                e.load_reply(h);
            }
        }
        return 1;
    }
    if m == 0x111 && w & 65535 == 108 && w >> 16 == 1 {
        ask::models(h, choice(h, 108));
        return 1;
    }
    if m == ask::IMAGE_PASTE || (m == 0x111 && w >> 16 == 0 && matches!(w & 65535, 411 | 412)) {
        let ptr = unsafe {GetWindowLongPtrW(h,-21)} as *mut Edit;
        if let Some(e) = unsafe {ptr.as_mut()} {
            let action = if m == ask::IMAGE_PASTE {m} else {w as u32};
            if let Err(err) = e.ask.attachment_action(h, action) { notice(h, &err); }
        }
        return 1;
    }
    if m == 0x113 && w == 2 {
        let ptr = unsafe {GetWindowLongPtrW(h,-21)} as *mut Edit;
        if let Some(e) = unsafe {ptr.as_mut()} {
            e.clear_completed_action(h);
            if e.ask.show(h) { e.load_reply(h); }
        }
        return 1;
    }
    if m == 0x113 && w == 1 {
        let ptr = unsafe {GetWindowLongPtrW(h, -21)} as *mut Edit;
        if let Some(e) = unsafe {ptr.as_mut()} {
            if let Some(rx) = &e.pending {
                let result = match rx.try_recv() {
                    Err(std::sync::mpsc::TryRecvError::Empty) => return 1,
                    Ok(reply) => reply,
                    Err(_) => None,
                };
                unsafe {KillTimer(h, 1);}
                e.pending = None;
                if let Some(reply) = result.filter(|r| r.time >= e.reply_time) {
                    e.reply_time=reply.time;
                    e.summary = display_reply(&action_summary::conversation(&reply));
                    e.set_full(h,&reply.text);
                    e.display_answer(h);
                } else if e.reply_time == 0 {
                    e.set_full(h,"回答はまだありません");
                    e.summary = "回答はまだありません".into();
                    e.display_answer(h);
                }
            }
        }
        return 1;
    }
    if m == 0x10 || m == 0x111 && w & 65535 == 2 {
        unsafe {
            EndDialog(h, 0);
        }
        return 1;
    }
    if m != 0x111 || w >> 16 != 0 {
        return 0;
    }
    let id = (w & 65535) as i32;
    match id {
        439 => {
            let e=unsafe{&mut *(GetWindowLongPtrW(h,-21) as *mut Edit)};
            if e.form.is_some(){e.apply_form(h);}
            else if !e.questions.is_empty(){e.form=Some(decisions::Form::open(h,e.questions.clone()));}
            e.show_page(h);
            1
        }
        438 => {
            if let Err(error)=voice_input::start(h){notice(h,&error);}
            1
        }
        437 => {
            let e=unsafe {&*(GetWindowLongPtrW(h,-21) as *const Edit)};
            let draft=if e.full_reply {e.full.clone()} else {e.summary.clone()};
            let body=if matches!(draft.as_str(),"回答はまだありません"|"回答を読み込み中…"){""}else{&draft};
            if let Err(e)=slack::open(h,body){notice(h,&e);}
            1
        }
        435 => {
            let ptr = unsafe {GetWindowLongPtrW(h,-21)} as *mut Edit;
            if let Some(e) = unsafe {ptr.as_mut()} {
                e.full_reply = !e.full_reply;
                e.display_answer(h);
            }
            1
        }
        416 => {
            let ptr = unsafe { GetWindowLongPtrW(h,-21) } as *mut Edit;
            if let Some(e) = unsafe { ptr.as_mut() } {
                e.apply_form(h);
                e.details = !e.details;
                e.show_page(h);
            }
            1
        }
        157 | 158 => {
            set(h, 110, &due_shortcut((id == 158) as i64));
            1
        }
        404 => {
            let ptr = unsafe {GetWindowLongPtrW(h,-21)} as *mut Edit;
            if let Some(e) = unsafe {ptr.as_mut()} {
                let result = e.read_fields(h).and_then(|_| e.ask.execute(h, e.app, &mut e.index, &mut e.draft));
                match result {
                    Err(err) => notice(h,&err),
                    Ok(()) => { set(h,107,&e.draft.title); unsafe { SetWindowTextW(h,wide(&format!("{} — KAGARI",e.draft.title)).as_ptr()); } }
                }
            }
            1
        }
        1 | 114 => {
            let ptr = unsafe { GetWindowLongPtrW(h, -21) };
            if ptr != 0 {
                let e = unsafe { &mut *(ptr as *mut Edit) };
                match e.submit(h) {
                    Ok(()) => unsafe {
                        EndDialog(h, 1);
                    },
                    Err(err) => notice(h, &err),
                }
            }
            1
        }
        _ => 0,
    }
}
