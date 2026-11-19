use super::*;
fn click(h: H, id: usize) {
    unsafe {
        SendMessageW(h, 0x111, id, 0);
    }
}
fn edit(h: H, a: &mut App, new: bool, action: fn(H)) -> Result<()> {
    editor::TEST_ACTION.with(|v| *v.borrow_mut() = Some(action));
    run(h, a, if new { 113 } else { 160 }, 0)
}
thread_local! { static REPORT:RefCell<Result<()>>=RefCell::new(Err("dialog not initialized".into())); }
extern "system" fn exercise(h: H, m: u32, _: usize, l: isize) -> isize {
    if m == 0x4e {
        let hdr = unsafe { &*(l as *const native::Header) };
        if hdr.id == 104 {
            return tree::notification(h, hdr.code, l);
        }
    }
    if m != 0x110 {
        return 0;
    }
    let outcome = (|| -> Result<()> {
        let mut a = App::default();
        options(h, 107, &REQUESTS);
        options(h, 108, &AGENTS);
        options(h, 109, &STATES);
        options(h, 117, &DOCS);
        let root = std::env::var("AI_WORKSPACE_TEST_DIR").unwrap();
        native::init(h);
        native::check_resize(h)?;
        register(h, &mut a, &root)?;
        APP.with(|c| *c.borrow_mut() = a.clone());
        tree::check(h)?;
        edit(h, &mut a, true, |m| {
            set(m, 107, "入力テスト");
            click(m, 114);
        })?;
        set(h, 118, "result.md");
        run(h, &mut a, 181, 0)?;
        run(h, &mut a, 119, 0)?;
        if !get(h, 120).contains("確認済み") {
            return Err("Markdown表示失敗".into());
        }
        edit(h, &mut a, false, |m| {
            msg(m, 109, 0x14e, 2, 0);
            click(m, 114);
        })?;
        if !get(h, 152).contains("1 件") {
            return Err("KPI更新失敗".into());
        }
        run(h, &mut a, 159, 0)?;
        run(h, &mut a, 152, 0)?;
        run(h, &mut a, 156, 0)?;
        if !a.indices().is_empty() || !App::load()?.tasks[0].pinned {
            return Err("状態またはピン保存失敗".into());
        }
        run(h, &mut a, 128, 0)?;
        edit(h, &mut a, true, |m| {
            set(m, 107, "次のタスク");
            click(m, 158);
            click(m, 114);
        })?;
        if a.tasks[1].due != due_shortcut(1) || a.indices().first() != Some(&0) {
            return Err("簡易登録失敗".into());
        }
        edit(h, &mut a, true, |m| click(m, 114))?;
        if a.tasks.last().unwrap().title != REQUESTS[0] {
            return Err("空欄登録失敗".into());
        }
        edit(h, &mut a, true, |m| {
            msg(m, 107, 0x14e, 2, 0);
            set(m, 112, "開発");
            click(m, 114);
        })?;
        edit(h, &mut a, false, |m| {
            set(m, 107, "破棄");
            click(m, 2);
        })?;
        if App::load()?.tasks.last().unwrap().title != REQUESTS[2] {
            return Err("キャンセルまたは候補登録失敗".into());
        }
        run(h, &mut a, 186, 0)?;
        set(h, 182, "次のタスク");
        run(h, &mut a, 182, 0x300)?;
        if a.indices().len() != 1 || msg(h, 191, 0x18b, 0, 0) != 1 {
            return Err("検索パネルの結果が一致しません".into());
        }
        run(h, &mut a, 183, 0)?;
        run(h, &mut a, 185, 0)?;
        for id in [194, 195, 196, 197, 199] {
            run(h, &mut a, id, 0)?;
        }
        if App::load()?.panes != 31 {
            return Err("レイアウト保存失敗".into());
        }
        run(h, &mut a, 200, 0)?;
        let i = a.task.ok_or("選択タスクなし")?;
        a.tasks[i].history_key = "123-01".into();
        a.tasks[i].agent = 1;
        a.tasks[i].model = "sonnet".into();
        let dir = state_path()?
            .with_file_name("history")
            .join("123-01")
            .join("001");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        for (name, value) in [
            ("meta.txt", "Claude Code / sonnet"),
            ("request.txt", "テスト依頼"),
            ("reply.txt", "前回の回答"),
        ] {
            std::fs::write(dir.join(name), value).map_err(|e| e.to_string())?;
        }
        a.save()?;
        let mut reopened = App::load()?;
        reopened.task = Some(i);
        thread_local! {static FOUND:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};}
        ask::TEST_ACTION.with(|c| {
            *c.borrow_mut() = Some(|m| {
                FOUND
                    .with(|v| v.set(get(m, 405).contains("前回の回答") && get(m, 402) == "sonnet"));
                click(m, 2);
            })
        });
        run(h, &mut reopened, 115, 0)?;
        if !FOUND.with(|v| v.get()) {
            return Err("履歴・モデルの復元失敗".into());
        }
        if unsafe { GetDlgItem(h, 107) } != 0 {
            return Err("メインに詳細欄が残っています".into());
        }
        if msg(h, 155, 0x18b, 0, 0) != 2 {
            return Err("部署別の集計行数が一致しません".into());
        }
        msg(h, 155, 0x186, 1, 0);
        run(h, &mut a, 155, 1)?;
        if a.indices().len() != 1 || a.department_filter != "開発" {
            return Err("部署選択に失敗".into());
        }
        msg(h, 155, 0x186, 0, 0);
        run(h, &mut a, 155, 1)?;
        if !a.unassigned || a.indices().len() != 3 {
            return Err("部署未設定の絞り込みに失敗".into());
        }
        Ok(())
    })();
    REPORT.with(|r| *r.borrow_mut() = outcome);
    unsafe {
        EndDialog(h, 0);
    }
    1
}
#[test]
fn native_workflow() {
    native::controls();
    if std::env::var_os("AI_WORKSPACE_TEST_DIR").is_none() {
        return;
    }
    unsafe {
        LoadLibraryW(wide("Msftedit.dll").as_ptr());
        DialogBoxParamW(
            GetModuleHandleW(std::ptr::null()),
            100 as Ptr,
            0,
            exercise,
            0,
        );
    }
    REPORT.with(|r| r.borrow().clone().unwrap());
}
