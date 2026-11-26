use super::*;
pub(super) const CONTINUE_OR_COMPLETE: &str = "このタスクの依頼内容とこれまでの結果を確認してください。残っている作業があれば継続し、すべて完了していればタスクのステータスを「完了」にしてください。";
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
    saved: bool,
    details: bool,
    full_reply: bool,
    summary: String,
    overview: action_summary::Overview,
    answered: Vec<bool>,
    full: String,
    form:Option<decisions::Form>,
    questions:Vec<decisions::Question>,
    detail_links:Vec<(i32,i32,String)>,
    layout: native::Layout,
    decision_font: Option<visual::DecisionFont>,
    rendered_links: Vec<(i32,i32,String)>,
}
impl Edit<'_> {
    fn show_page(&mut self, h: H) {
        let display = |id, visible| unsafe { ShowWindow(GetDlgItem(h,id), if visible {5} else {0}); };
        let answering=self.form.is_some() && !self.details;
        for id in [420,414,435,421] { display(id,!self.details); }
        self.layout.detail_view=self.details;
        display(403,!answering);
        display(446,!self.details && !answering);
        display(449,!self.details && !answering);
        display(433,self.layout.log_expanded);
        display(404,true); display(438,!self.details);
        for id in [681,683,684,685,686] {display(id,answering);}
        set(h,421,if answering {"決めること"}else{"追加入力・依頼"});
        set(h,439,if answering {"回答を反映".into()}else{format!("決定事項に回答（{}）",self.questions.len())}.as_str());
        for id in (422..=429).chain([431,432,445]).chain([107,108,109,110,111,157,158,402,405,406,408,409,411,412,415]) {
            display(id,self.details);
        }
        for id in [450,451,452,453] {display(id,self.details);}
        display(437,!answering);
        unsafe { ShowWindow(GetDlgItem(h,444),if !answering && native::has_preview(h) {5} else {0}); }
        self.layout.resize_task(h,false);
        self.show_decisions(h);
        self.ask.approval_controls(h,!self.details);
        set(h,416,if self.details {"サマリに戻る"} else {"設定・履歴"});
        visual::set_action_icon(h,416,if self.details {39} else {40});
        unsafe { SetFocus(GetDlgItem(h,if self.details && self.layout.settings_open {107} else if answering {681} else {403})); }
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
    fn answer_heading(&self,h:H) {
        let prior=(self.ask.failed || self.ask.running) && self.reply_time!=0;
        let title=if prior && self.full_reply {"前回のAI回答（全文）"} else if prior {"前回の回答のサマリ"} else if self.ask.running {"今回の回答を待っています"} else if self.full_reply {"最新のAI回答（全文）"} else {"やり取りのサマリ"};
        let title=if !self.full_reply && self.reply_time!=0 && self.overview.links.is_empty(){if prior {"前回のAI回答（全文）"}else{"最新のAI回答（全文）"}}else{title};
        let count = self.answered.iter().filter(|a| !**a).count();
        set(h,420,&format!("{title}　未回答 {count} 件{}",reply_received_at(self.reply_time)));
    }
    fn bind_session(&mut self,h:H){
        msg(h,108,0x14e,self.draft.agent,0);
        self.ask.bind(h,&self.draft);
        let labels=self.session_labels();
        options(h,415,&labels.iter().map(String::as_str).collect::<Vec<_>>());
        unsafe { EnableWindow(GetDlgItem(h,415),(labels.len()>1) as i32); }
        self.load_reply(h);
    }
    fn select_agent(&mut self,h:H,agent:usize){
        if agent==self.draft.agent{return;}
        let model=get(h,402);
        self.draft.model=if model==ask::DEFAULT {String::new()} else {model.trim().into()};
        let effort=ask::efforts(self.draft.agent)[choice(h,408)];
        self.draft.effort=if effort==ask::DEFAULT {String::new()} else {effort.into()};
        self.draft.select_agent(agent);
        self.bind_session(h);
    }
    fn display_answer(&mut self,h:H) {
        self.answer_heading(h);
        let fallback=self.reply_time!=0 && self.overview.links.is_empty();
        unsafe {EnableWindow(GetDlgItem(h,435),(!fallback) as i32);}
        set(h,435,if fallback {"全文を表示中"} else if self.full_reply {"サマリに戻す"} else {"回答全文を表示"});
        visual::set_action_icon(h,435,if self.full_reply {39} else {38});
        if !self.full_reply && self.reply_time != 0 && !self.overview.links.is_empty() {
            set(h,414,&display_reply(&self.overview.text));
            reply_links::style_ranges(h,414,&self.overview.links);
            self.rendered_links.clear();
            return;
        }
        let shown=if self.full_reply || (self.reply_time!=0 && self.overview.links.is_empty()) {&self.full} else {&self.summary};
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
        self.answered=self.answer_states(h);
        self.overview=action_summary::overview(text,&self.questions,&self.answered);
        self.detail_links=markdown_preview::Preview::document(text,native::document_width(h,409)).show_answer(h,409);
    }
    fn answer_states(&self,h:H) -> Vec<bool> {
        self.form.as_ref().map_or_else(
            || decisions::Form::answered(&self.questions,&get(h,403)),
            |form| form.current_answers(&get(h,684)),
        )
    }
    fn refresh_answers(&mut self,h:H) {
        let states=self.answer_states(h);
        if states != self.answered {
            self.answered=states;
            self.overview=action_summary::overview(&self.full,&self.questions,&self.answered);
            if self.full_reply { self.answer_heading(h); } else { self.display_answer(h); }
        }
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
            let (agent, session) = (self.draft.agent, ask::reply_session(&self.draft));
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
        if let Some(form) = &mut self.form { form.apply(h); }
        let action = get(h,403);
        let unchanged = action == stored.next_action && self.ask.images == stored.draft_images;
        let mut next = self.app.clone();
        next.tasks[i].submitted_action_run.clear();
        if unchanged {
            next.tasks[i].next_action.clear();
            next.tasks[i].draft_images.clear();
        } else {
            next.tasks[i].next_action = action;
            next.tasks[i].draft_images.clone_from(&self.ask.images);
        }
        if next.save().is_err() { return; }
        self.draft.next_action.clone_from(&next.tasks[i].next_action);
        self.draft.draft_images.clone_from(&next.tasks[i].draft_images);
        *self.app = next;
        self.draft.submitted_action_run.clear();
        self.ask.track_submission(&self.draft);
        if unchanged {
            self.form = None;
            set(h,403,"");
            self.ask.clear_images();
            self.ask.refresh_attachments(h);
            self.show_page(h);
            self.layout.resize_task(h,false);
        }
    }
    fn apply_form(&mut self,h:H){if let Some(mut form)=self.form.take(){form.apply(h);}}
    fn read_request(&mut self, h: H) {
        if let Some(form)=&mut self.form { form.apply(h); }
        let action = get(h,403);
        self.draft.submitted_action_run = self.ask.submitted_run(&action).unwrap_or_default().to_owned();
        self.draft.next_action = action;
        self.draft.draft_images.clone_from(&self.ask.images);
    }
    fn save_request_on_close(&mut self, h: H) -> Result<()> {
        self.read_request(h);
        if let Some(i) = self.index {
            let stored = &self.app.tasks[i];
            if stored.next_action == self.draft.next_action && stored.draft_images == self.draft.draft_images { return Ok(()); }
            let mut next = self.app.clone();
            next.tasks[i].next_action.clone_from(&self.draft.next_action);
            next.tasks[i].draft_images.clone_from(&self.draft.draft_images);
            next.tasks[i].submitted_action_run.clear();
            next.save()?;
            *self.app = next;
            self.saved = true;
        } else if !self.draft.next_action.is_empty() || !self.draft.draft_images.is_empty() {
            self.submit(h)?;
        }
        Ok(())
    }
    fn read_fields(&mut self, h: H) -> Result<()> {
        self.read_request(h);
        let due = get(h, 110);
        if !due.is_empty() && parse_date(&due).is_none() {
            return Err("期限は YYYY-MM-DD で入力してください".into());
        }
        let title = get(h, 107);
        self.draft.title = if title.trim().is_empty() {
            self.draft.next_action.lines().map(str::trim).find(|s| !s.is_empty())
                .unwrap_or(REQUESTS[0]).chars().take(80).collect()
        } else {
            title.trim().into()
        };
        let agent = choice(h, 108);
        self.select_agent(h,agent);
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
        next.task = Some(i);
        next.save()?;
        self.index = Some(i);
        self.saved = true;
        self.ask.track_submission(&self.draft);
        *self.app = next;

        Ok(())
    }
}
pub fn open(owner: H, a: &mut App, new: bool) -> Result<()> {
    open_at(owner, a, new, false)
}
pub(super) fn open_at(owner: H, a: &mut App, new: bool, _focus_request: bool) -> Result<()> {
    open_with_parent(owner,a,new,_focus_request,None)
}
pub(super) fn open_with_parent(owner:H,a:&mut App,new:bool,_focus_request:bool,parent:Option<usize>)->Result<()> {
    open_seeded(owner,a,new,_focus_request,parent,None)
}
pub(super) fn open_seeded(owner:H,a:&mut App,new:bool,_focus_request:bool,parent:Option<usize>,seed:Option<(&str,&str)>)->Result<()> {
    let project=match parent {Some(i)=>a.tasks.get(i).ok_or("親タスクが見つかりません")?.project.clone(),None=>a.root()?.to_string()};
    let index = if new {
        None
    } else {
        Some(a.task.ok_or("編集するタスクを選んでください")?)
    };
    let mut draft = index.map(|i| a.tasks[i].clone()).unwrap_or_else(|| Task {
        project,
        parent:parent.map(|i|a.tasks[i].id),
        agent: parent.map_or(a.agent_filter.saturating_sub(1),|i|a.tasks[i].agent),
        ..Default::default()
    });
    if let Some((title, note)) = seed {
        draft.title = title.to_owned();
        draft.next_action = note.to_owned();
    }
    if let Some(session) = ask::active_session(&draft) { draft.activate(session); }
    let mut e = Edit {
        app: a,
        index,
        draft,
        pending: None,
        reply_time: 0,
        ask: ask::Ask::default(),
        #[cfg(test)]
        focus_request: _focus_request,
        saved: false,
        details: false,
        full_reply: false,
        summary: String::new(),
        overview: Default::default(),
        answered: Vec::new(),
        full: String::new(),
        questions:Vec::new(),
        form:None,
        detail_links:Vec::new(),
        layout: Default::default(),
        decision_font: None,
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
    e.ask.release_unused_images(e.app);
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
    local_datetime(millis).map(|t|format!("　　回答日時 {t}")).unwrap_or_default()
}
pub(super) fn local_datetime(millis:u128)->Option<String> {
    #[link(name="kernel32")]
    extern "system"{
        fn FileTimeToSystemTime(time:*const u64,out:*mut [u16;8])->i32;
        fn SystemTimeToTzSpecificLocalTime(zone:usize,utc:*const [u16;8],out:*mut [u16;8])->i32;
    }
    if millis==0{return None;}
    let filetime=millis.checked_add(11644473600000)?.checked_mul(10000)?.try_into().ok()?;
    let mut utc=[0;8];let mut local=[0;8];
    if unsafe{FileTimeToSystemTime(&filetime,&mut utc)!=0 && SystemTimeToTzSpecificLocalTime(0,&utc,&mut local)!=0}{
        Some(format!("{:04}/{:02}/{:02} {:02}:{:02}:{:02}",local[0],local[1],local[3],local[4],local[5],local[6]))
    }else{None}
}
fn display_reply(text: &str) -> String {text.replace("\r\n","\n").replace('\n',"\r\n")}

#[cfg(test)]
#[test]
fn overview_opens_the_exact_answer_row() {
    thread_local! { static VERIFIED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
    native::controls();
    unsafe { LoadLibraryW(wide("Msftedit.dll").as_ptr()); }
    let mut app=App::default();
    app.projects.push(std::env::temp_dir().to_string_lossy().into_owned());
    TEST_ACTION.with(|slot| *slot.borrow_mut()=Some(|h| {
        let reply="## 完了事項\r\n- 確認😀\r\n\r\n| ID | 決めること | 選択肢 |\r\n|---|---|---|\r\n| D1 | 確認😀 | |\r\n\r\n## 次の依頼\r\n- 確認😀";
        let e=unsafe { &mut *(GetWindowLongPtrW(h,-21) as *mut Edit) };
        e.reply_time=1;
        e.set_full(h,reply);
        e.display_answer(h);
        let pending=get(h,420).contains("未回答 1 件");
        set(h,403,"- D1 確認😀：確認済み");
        e.refresh_answers(h);
        let answered=get(h,414).contains("【回答済み】D1 確認😀") && get(h,420).contains("未回答 0 件");
        let mut point=[0i32;2];
        let at=e.overview.links[1].0+1;
        msg(h,414,0x426,point.as_mut_ptr() as usize,at as isize);
        let position=(point[0]+1) as isize | (((point[1]+2) as isize)<<16);
        msg(h,414,0x201,1,position);
        msg(h,414,0x202,0,position);
        let mut selected=vec![0u16;reply.encode_utf16().count()+1];
        let n=msg(h,414,0x43e,0,selected.as_mut_ptr() as isize) as usize;
        VERIFIED.with(|v| v.set(pending && answered && String::from_utf16_lossy(&selected[..n]).split_whitespace().collect::<Vec<_>>()==["D1","確認😀"]));
        unsafe { EndDialog(h,0); }
    }));
    open(0,&mut app,true).unwrap();
    assert!(VERIFIED.with(|v| v.get()));
}
extern "system" fn dialog_proc(h: H, m: u32, w: usize, l: isize) -> isize {
    if matches!(m, 5 | 0x24) {
        let ptr = unsafe { GetWindowLongPtrW(h,-21) } as *mut Edit;
        if let Some(e) = unsafe {ptr.as_mut()} {
            if m == 5 {
                e.layout.resize_task(h,false);
                if let Some(form)=e.form.as_mut(){form.rewrap(h);}
            } else { e.layout.minimum(l); }
        }
        return 0;
    }
    if m == 0x4e && l!=0 && matches!(w,414|409) {
        let Some(position) = reply_links::clicked_position(l) else { return 0; };
        let ptr=unsafe{GetWindowLongPtrW(h,-21)} as *mut Edit;
        if let Some(e)=unsafe{ptr.as_mut()} {
            if w==414 && !e.full_reply && e.reply_time != 0 {
                if let Some(&(_,_,(start,end)))=e.overview.links.iter().find(|&&(a,b,_)| a<=position && position<b) {
                    e.full_reply=true;
                    e.answer_heading(h);
                    set(h,435,"サマリに戻す");
                    visual::set_action_icon(h,435,39);
                    e.display_answer(h);
                    unsafe { SetFocus(GetDlgItem(h,414)); }
                    markdown_preview::Preview::select_source_line(h,414,&e.full,start,end);
                    return 1;
                }
            }
            let task=e.draft.clone(); return reply_links::click(h,l,&task,if w==414 {&e.rendered_links} else {&e.detail_links});
        }
    }
    const UPDATE_SEND:u32=0x8000+112;
    if m==0x111 && matches!(w&65535,403|684) && w>>16==0x300{
        unsafe{PostMessageW(h,UPDATE_SEND,0,0);}
        return 0;
    }
    if m==UPDATE_SEND{
        let ptr=unsafe{GetWindowLongPtrW(h,-21)} as *mut Edit;
        if let Some(e)=unsafe{ptr.as_mut()}{
            if let Some(form)=e.form.as_mut(){form.apply(h);}
            e.ask.update_send(h);
            e.refresh_answers(h);
            if e.details {e.layout.resize_task(h,false);}
        }
        return 1;
    }
    if m==0x2b && w==404 {visual::draw_send(h,l);return 1;}
    if m==0x2b && w==683 {visual::draw_choice(h,l);return 1;}
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
        e.decision_font=visual::decision_font(h);
        visual::set_action_icon(h,114,34);
        for (id,icon) in [(448,43),(450,44),(437,45),(411,46),(412,47),(452,48)] {visual::set_action_icon(h,id,icon);}
        visual::set_action_icon(h,449,42);
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
        e.answer_heading(h);
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
                e.bind_session(h);
            }
        }
        return 1;
    }
    if m == 0x111 && w & 65535 == 108 && w >> 16 == 1 {
        let ptr=unsafe{GetWindowLongPtrW(h,-21)} as *mut Edit;
        if let Some(e)=unsafe{ptr.as_mut()}{e.select_agent(h,choice(h,108));}
        return 1;
    }
    if matches!(m,ask::IMAGE_PASTE | ask::IMAGE_REMOVE) || (m == 0x111 && w >> 16 == 0 && matches!(w & 65535, 411 | 412)) {
        let ptr = unsafe {GetWindowLongPtrW(h,-21)} as *mut Edit;
        if let Some(e) = unsafe {ptr.as_mut()} {
            let action = if m == 0x111 {w as u32} else {m};
            if let Err(err) = e.ask.attachment_action(h, action,w) { notice(h, &err); }
            unsafe {ShowWindow(GetDlgItem(h,444),if e.form.is_none() && native::has_preview(h){5}else{0});}
            e.layout.resize_task(h,false);
        }
        return 1;
    }
    if m == 0x113 && w == 3 {
        unsafe { KillTimer(h,3); }
        set(h,114,"下書き保存");
        return 1;
    }
    if m == 0x113 && w == 2 {
        let ptr = unsafe {GetWindowLongPtrW(h,-21)} as *mut Edit;
        if let Some(e) = unsafe {ptr.as_mut()} {
            e.clear_completed_action(h);
            if e.ask.show(h) { e.load_reply(h); }
            e.ask.approval_controls(h,!e.details);
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
                    if e.full == display_reply(&reply.text) {
                        e.answer_heading(h);
                        return 1;
                    }
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
        let edit = unsafe { &mut *(GetWindowLongPtrW(h,-21) as *mut Edit) };
        match edit.save_request_on_close(h) {
            Ok(()) => unsafe { EndDialog(h, edit.saved as isize); },
            Err(err) => notice(h,&err),
        }
        return 1;
    }
    if m != 0x111 || w >> 16 != 0 {
        return 0;
    }
    let id = (w & 65535) as i32;
    match id {
        450 | 451 | 452 => {
            let e=unsafe{&mut *(GetWindowLongPtrW(h,-21) as *mut Edit)};
            if !e.details{return 1;}
            if id==450 {e.layout.settings_open=!e.layout.settings_open;}
            else {e.layout.history_tab=id==452;}
            e.layout.resize_task(h,false);
            1
        }
        453 => {
            let e=unsafe{&mut *(GetWindowLongPtrW(h,-21) as *mut Edit)};
            e.ask.more_history(h);
            1
        }
        448 => {
            let e=unsafe{&mut *(GetWindowLongPtrW(h,-21) as *mut Edit)};
            e.layout.log_expanded=!e.layout.log_expanded;
            set(h,448,if e.layout.log_expanded {"ログを閉じる"} else {"実行ログを表示"});
            unsafe {ShowWindow(GetDlgItem(h,433),if e.layout.log_expanded{5}else{0});}
            e.layout.resize_task(h,false);
            1
        }
        449 => {
            set(h,421,"修正内容を入力");
            unsafe {SetFocus(GetDlgItem(h,403));}
            1
        }
        446 => {
            let draft = get(h,403);
            if !draft.lines().any(|line| line.trim() == CONTINUE_OR_COMPLETE) {
                let separator = if draft.is_empty() || draft.ends_with('\n') { "" } else { "\r\n" };
                let end = draft.encode_utf16().count();
                msg(h,403,0xb1,end,end as isize);
                msg(h,403,0xc2,1,wide(&format!("{separator}{CONTINUE_OR_COMPLETE}")).as_ptr() as _);
            }
            unsafe { SetFocus(GetDlgItem(h,403)); }
            1
        }
        442 | 443 => {
            let e=unsafe{&mut *(GetWindowLongPtrW(h,-21) as *mut Edit)};
            if !e.details { if let Err(err)=e.ask.decide(h,id==442){notice(h,&err);} }
            1
        }
        439 => {
            let e=unsafe{&mut *(GetWindowLongPtrW(h,-21) as *mut Edit)};
            if e.form.is_some(){e.apply_form(h);}
            else if !e.questions.is_empty(){e.form=Some(decisions::Form::open(h,e.questions.clone()));}
            e.show_page(h);
            1
        }
        438 => {
            let e=unsafe{&*(GetWindowLongPtrW(h,-21) as *const Edit)};
            let field=if e.form.is_some(){684}else{403};
            if let Err(error)=voice_input::start(h,field){notice(h,&error);}
            1
        }
        437 => {
            let e=unsafe {&*(GetWindowLongPtrW(h,-21) as *const Edit)};
            let draft=if e.full_reply {e.full.clone()} else {e.summary.clone()};
            let body=if matches!(draft.as_str(),"回答はまだありません"|"回答を読み込み中…"){""}else{&draft};
            if let Err(e)=slack::open(h,body,&e.full){notice(h,&e);}
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
        404 | 445 => {
            let ptr = unsafe {GetWindowLongPtrW(h,-21)} as *mut Edit;
            if let Some(e) = unsafe {ptr.as_mut()} {
                let retry = id == 445;
                if retry && !e.details && !e.ask.failed { return 1; }
                let prior = e.draft.submitted_action_run.clone();
                let result = e.read_fields(h).and_then(|_| e.ask.execute(h, e.app, &mut e.index, &mut e.draft,retry));
                if e.draft.submitted_action_run != prior && !e.draft.submitted_action_run.is_empty() {
                    e.saved = true;
                }
                if retry && e.draft.submitted_action_run != prior {
                    e.details = false;
                    e.show_page(h);
                }
                match result {
                    Err(err) => notice(h,&err),
                    Ok(()) => { e.layout.resize_task(h,false); set(h,107,&e.draft.title); unsafe { SetWindowTextW(h,wide(&format!("{} — KAGARI",e.draft.title)).as_ptr()); } }
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
                        SetWindowTextW(h,wide(&format!("{} — KAGARI",e.draft.title)).as_ptr());
                        if get(h,107)!=e.draft.title { set(h,107,&e.draft.title); }
                        e.form = None;
                        e.details = false;
                        if e.full_reply {
                            e.full_reply = false;
                            e.display_answer(h);
                        }
                        e.show_page(h);
                        set(h,114,"保存しました");
                        SetTimer(h,3,1800,0);

                    },
                    Err(err) => notice(h, &err),
                }
            }
            1
        }
        _ => 0,
    }
}
