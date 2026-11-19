use super::*;
#[link(name = "user32")]
extern "system" {
    fn GetWindowLongPtrW(h: H, i: i32) -> isize;
    fn SetWindowLongPtrW(h: H, i: i32, v: isize) -> isize;
}
struct Edit<'a> {
    app: &'a mut App,
    index: Option<usize>,
    draft: Task,
}
impl Edit<'_> {
    fn submit(&mut self, h: H) -> Result<()> {
        let due = get(h, 110);
        if !due.is_empty() && parse_date(&due).is_none() {
            return Err("期限は YYYY-MM-DD で入力してください".into());
        }
        let title = get(h, 107);
        self.draft.title = if title.trim().is_empty() {
            REQUESTS[0].into()
        } else {
            title.trim().into()
        };
        let agent = choice(h, 108);
        if agent != self.draft.agent {
            self.draft.model.clear();
        }
        if !self.draft.session.is_empty() && agent != self.draft.agent {
            return Err("このタスクは元のセッションの担当AIで再開します".into());
        }
        self.draft.agent = agent;
        self.draft.status = choice(h, 109);
        self.draft.due = due;
        self.draft.check = get(h, 111);
        self.draft.dept = get(h, 112).trim().into();
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
    let project = a.root()?.to_string();
    let index = if new {
        None
    } else {
        Some(a.task.ok_or("編集するタスクを選んでください")?)
    };
    let draft = index.map(|i| a.tasks[i].clone()).unwrap_or_else(|| Task {
        project,
        dept: a.department_filter.clone(),
        agent: a.agent_filter.saturating_sub(1),
        ..Default::default()
    });
    let mut e = Edit {
        app: a,
        index,
        draft,
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
    if result == 2 {
        ask::open(owner, a)?;
    }
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
extern "system" fn dialog_proc(h: H, m: u32, w: usize, l: isize) -> isize {
    if m == 0x2b && w == 108 {
        brand::draw(l);
        return 1;
    }
    if m == 0x110 {
        unsafe {
            SetWindowLongPtrW(h, -21, l);
        }
        let e = unsafe { &*(l as *const Edit) };
        let t = &e.draft;
        options(h, 107, &REQUESTS);
        options(h, 108, &AGENTS);
        options(h, 109, &STATES);
        for (id, value) in [
            (107, &t.title),
            (110, &t.due),
            (111, &t.check),
            (112, &t.dept),
        ] {
            set(h, id, value)
        }
        msg(h, 108, 0x14e, t.agent, 0);
        msg(h, 109, 0x14e, t.status, 0);
        unsafe {
            SetWindowTextW(
                h,
                wide(if e.index.is_none() {
                    "新規タスク — KAGARI"
                } else {
                    "タスク詳細 — KAGARI"
                })
                .as_ptr(),
            );
            SetFocus(GetDlgItem(h, 107));
        }
        #[cfg(test)]
        {
            let action = TEST_ACTION.with(|cell| cell.borrow_mut().take());
            if let Some(action) = action {
                action(h);
            }
        }
        return 0;
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
        157 | 158 => {
            set(h, 110, &due_shortcut((id == 158) as i64));
            1
        }
        1 | 114 | 115 => {
            let ptr = unsafe { GetWindowLongPtrW(h, -21) };
            if ptr != 0 {
                let e = unsafe { &mut *(ptr as *mut Edit) };
                match e.submit(h) {
                    Ok(()) => unsafe {
                        EndDialog(h, if id == 115 { 2 } else { 1 });
                    },
                    Err(err) => notice(h, &err),
                }
            }
            1
        }
        _ => 0,
    }
}
