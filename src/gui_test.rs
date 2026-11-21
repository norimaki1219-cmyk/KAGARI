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
            set(m,403,"下書き");
            msg(m,403,0xb1,0,3);
            voice_input::TEST_SHORTCUT.with(|flag|flag.set(true));
            click(m,438);
            msg(m,403,0xc2,1,wide("に追記").as_ptr() as _);
            set(m, 107, "入力テスト");
            click(m, 114);
        })?;
        if a.tasks.last().map(|t|t.next_action.as_str())!=Some("下書きに追記"){return Err("音声入力の追記位置".into());}
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
        a.tasks[i].effort = "high".into();
        a.tasks[i].model = "sonnet".into();
        let dir = state_path()?
            .with_file_name("history")
            .join("123-01")
            .join("001");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let long_reply = format!("# 前回の回答\n[作業結果](result.md)\n| 投稿 | 中身 |\n|---|---|\n| 本文 | **太字の確認** |\n\n| ID | 決めること | 選択肢 |\n|---|---|---|\n| W1 | 方法 | a：手動<br>b：自動 |\n| W2 | 宛先 | |\n\n{}\nロードマップ末尾", "月別のタスク。".repeat(1200));
        for (name, value) in [
            ("meta.txt", "Claude Code / sonnet"),
            ("request.txt", "テスト依頼"),
            ("reply.txt", long_reply.as_str()),
        ] {
            std::fs::write(dir.join(name), value).map_err(|e| e.to_string())?;
        }
        a.save()?;
        let mut reopened = App::load()?;
        reopened.task = Some(i);
        thread_local! {static FOUND:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};}
        edit(h, &mut reopened, false, |m| {
            let reply_size=|| native::client_size(unsafe {GetDlgItem(m,414)});
            let normal=reply_size();
            unsafe{ShowWindow(m,3);}
            let maximized=reply_size();
            unsafe{ShowWindow(m,9);}
            let resized=maximized.0>normal.0 && maximized.1>normal.1 && reply_size()==normal;
            click(m,435);
            reply_links::TEST_PREVIEW.with(|cb| *cb.borrow_mut()=Some(|h|{
                let body=get(h,120);
                let source=std::fs::read_to_string(PathBuf::from(std::env::var("AI_WORKSPACE_TEST_DIR").unwrap()).join("result.md")).unwrap();
                let headings:Vec<_>=source.lines().filter_map(|line|line.trim_start_matches('#').strip_prefix(' ').filter(|_|line.starts_with('#'))).collect();
                let mut ok=body.contains("確認済み") && !body.contains("|---|");
                if let Some(last)=headings.last() {
                    let index=headings.len()-1;
                    ok &= msg(h,672,0x18b,0,0)==headings.len() as isize;
                    msg(h,672,0x186,index,0);
                    unsafe {SendMessageW(h,0x111,(1<<16)|672,0);}
                    let mut position=0i32;
                    msg(h,120,0xb0,&mut position as *mut _ as usize,0);
                    let len=last.encode_utf16().count();
                    msg(h,120,0xb1,position as usize,position as isize+len as isize);
                    let mut buf=vec![0u16;len+1];
                    msg(h,120,0x43e,0,buf.as_mut_ptr() as isize);
                    ok &= String::from_utf16_lossy(&buf[..len])==*last;
                }
                FOUND.with(|v|v.set(ok));
                click(h,2);
            }));
            FOUND.with(|v|v.set(false));
            let mut point=[0i32;2];
            msg(m,414,0x426,point.as_mut_ptr() as usize,8);
            let position=(point[0]+2) as isize | (((point[1]+2) as isize)<<16);
            msg(m,414,0x201,1,position);
            msg(m,414,0x202,0,position);
            FOUND.with(|v| v.set(resized && v.get() && get(m,414).starts_with("前回の回答") && !get(m,414).contains("**") && !get(m,414).contains("|---|") && !get(m,409).contains("| 投稿 |") && get(m,414).ends_with("ロードマップ末尾") && get(m,414).chars().count() > 7000 && get(m,435) == "サマリに戻す" && [435,438,114,416,439].iter().all(|id| msg(m,*id,0xf6,1,0)!=0)));
            click(m,435);
            set(m,403,"下書き");
            click(m,439);
            msg(m,683,0x186,1,0);
            unsafe{SendMessageW(m,0x111,(1<<16)|683,0);}
            msg(m,681,0x186,1,0);
            unsafe{SendMessageW(m,0x111,(1<<16)|681,0);}
            set(m,684,"担当者");
            click(m,439);
            FOUND.with(|v|v.set(v.get() && get(m,403)=="下書き\r\n- W1 方法：b：自動\r\n- W2 宛先：担当者"));
            set(m, 403, "手順を確認\r\n検証する");
            click(m, 114);
        })?;
        if !FOUND.with(|v| v.get()) || App::load()?.tasks[i].next_action != "手順を確認\r\n検証する" {
            return Err("最新回答または次のアクションの保存に失敗".into());
        }
        ask::TEST_ACTION.with(|c| {
            *c.borrow_mut() = Some(|m| {
                FOUND
                    .with(|v| v.set(get(m, 405).contains("前回の回答") && get(m, 402) == "sonnet" && get(m, 408) == "high"));
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
        if a.tasks.iter().any(|t| &t.project != a.root().unwrap()) || a.indices().len() != 4 {
            return Err("選択したプロジェクトへの自動登録に失敗".into());
        }
        let task = &mut reopened.tasks[i];
        task.session = "original-claude".into();
        task.session_title = "調査".into();
        let next = TaskSession { id: "next-codex".into(), title: "実装".into(), ..Default::default() };
        task.link_session(next.clone());
        task.link_session(next);
        reopened.save()?;
        let mut restored = App::load()?;
        restored.task = Some(i);
        if restored.tasks[i].session != "next-codex" || restored.tasks[i].other_sessions.len() != 1 {
            return Err("引き継ぎまたは重複防止に失敗".into());
        }
        edit(h, &mut restored, false, |m| {
            msg(m, 415, 0x14e, 1, 0);
            unsafe { SendMessageW(m, 0x111, (1 << 16) | 415, 0); }
            click(m, 114);
        })?;
        let restored = App::load()?;
        let task = &restored.tasks[i];
        if task.session != "original-claude" || task.agent != 1 || task.model != "sonnet"
            || task.effort != "high" || task.history_key != "123-01"
            || task.other_sessions[0].id != "next-codex" || task.next_action != reopened.tasks[i].next_action {
            return Err("セッション切り替え後の保存・復元に失敗".into());
        }
        // Exercise the real worker and dialog without contacting an AI provider.
        let fixture = state_path()?.with_file_name("background-fixture");
        fs::create_dir_all(&fixture).map_err(|e| e.to_string())?;
        let cli = fixture.join("codex.cmd");
        fs::write(&cli, concat!("@echo off\r\n",
            r#"echo {"type":"item.completed","item":{"type":"collab_tool_call","tool":"spawn_agent","receiver_thread_ids":["fixture-child"],"agents_states":{"fixture-child":{"status":"running"}}}}"#, "\r\n",
            "for /L %%i in (1,1,15) do (\r\n",
            " if exist \"%KAGARI_RUN%\\closed.ok\" goto done\r\n",
            " ping -n 2 127.0.0.1 >nul\r\n)\r\nexit /b 1\r\n",
            ":done\r\n echo background-complete > \"%KAGARI_RUN%\\last.tmp\"\r\n"))
            .map_err(|e| e.to_string())?;
        let original_path = std::env::var_os("PATH").unwrap_or_default();
        let paths = std::iter::once(fixture).chain(std::env::split_paths(&original_path));
        std::env::set_var("PATH", std::env::join_paths(paths).map_err(|e| e.to_string())?);
        let mut app = restored;
        app.task = Some(i);
        app.tasks[i].activate(0);
        let submitted = (|| {
            if client::find(0)? != cli { return Err("テストCLIを選択できません".into()); }
            ask::TEST_ACTION.with(|c| *c.borrow_mut() = Some(|m| {
                set(m, 403, "統合画面の保存テスト");
                click(m, 416);
                click(m, 416);
                click(m, 404);
                click(m, 2);
            }));
            ask::open(h, &mut app)
        })();
        std::env::set_var("PATH", original_path);
        submitted?;
        if App::load()?.tasks[i].next_action != "統合画面の保存テスト" { return Err("依頼送信時にタスク詳細を保存できません".into()); }
        let folder = state_path()?.with_file_name("history").join(&app.tasks[i].history_key);
        let dir = fs::read_dir(folder).map_err(|e| e.to_string())?
            .next().ok_or("依頼がありません")?.map_err(|e| e.to_string())?.path();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
        while !dir.join("subagents.txt").is_file() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        ask::TEST_ACTION.with(|c| *c.borrow_mut() = Some(|m| {
            FOUND.with(|v| v.set(get(m, 403) == "統合画面の保存テスト" && get(m, 404) == "実行中…" && get(m,433).contains("実行中") && get(m,434).contains("モデル不明　1/1")));
            click(m, 2);
        }));
        ask::open(h, &mut app)?;
        if !FOUND.with(|v| v.get()) { return Err("閉じた後の実行状態を復元できません".into()); }
        fs::write(dir.join("closed.ok"), "").map_err(|e| e.to_string())?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while !dir.join("reply.md").is_file() && !dir.join("error.txt").is_file() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let reply = dir.join("reply.md");
        let error = dir.join("error.txt");
        fs::rename(&reply,&error).map_err(|e|e.to_string())?;
        ask::TEST_ACTION.with(|c| *c.borrow_mut() = Some(|m| {
            FOUND.with(|v| v.set(get(m,403) == "統合画面の保存テスト" && get(m,433).contains("実行エラー")));
            click(m,2);
        }));
        ask::open(h,&mut app)?;
        if !FOUND.with(|v|v.get()) { return Err("失敗時の入力復元に失敗".into()); }
        let mut newer_draft = app.clone();
        ask::TEST_ACTION.with(|c| *c.borrow_mut() = Some(|m| {
            set(m,403,"新しい下書き");
            click(m,114);
        }));
        ask::open(h,&mut newer_draft)?;
        fs::rename(&error,&reply).map_err(|e|e.to_string())?;
        ask::TEST_ACTION.with(|c| *c.borrow_mut() = Some(|m| {
            FOUND.with(|v| v.set(get(m,403) == "新しい下書き"));
            click(m,2);
        }));
        ask::open(h,&mut newer_draft)?;
        if !FOUND.with(|v|v.get()) { return Err("新しい下書きの保持に失敗".into()); }
        ask::TEST_ACTION.with(|c| *c.borrow_mut() = Some(|m| {
            FOUND.with(|v| v.set(get(m, 403).is_empty() && get(m, 405).contains("background-complete") && get(m, 404) == "依頼を送信" && get(m,433).contains("回答受信")));
            click(m, 2);
        }));
        ask::open(h, &mut app)?;
        if !FOUND.with(|v| v.get()) { return Err("閉じた状態での回答保存・再表示に失敗".into()); }
        let history = fs::read(&reply).map_err(|e|e.to_string())?;
        let count = app.tasks.len();
        app.delete_task(i)?;
        if App::load()?.tasks.len() != count - 1 || fs::read(&reply).ok() != Some(history) {
            return Err("タスク削除後の履歴保持に失敗".into());
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
