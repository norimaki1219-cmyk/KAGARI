use super::*;
fn click(h: H, id: usize) {
    unsafe {
        SendMessageW(h, 0x111, id, 0);
    }
}
fn reachable(dialog:H,id:i32)->bool {
    #[link(name="user32")]
    extern "system"{
        fn ClientToScreen(child:H,point:*mut [i32;2])->i32;
        fn ScreenToClient(parent:H,point:*mut [i32;2])->i32;
        fn ChildWindowFromPointEx(parent:H,point:i64,flags:u32)->H;
        fn IsWindowEnabled(h:H)->i32;
    }
    unsafe{
        let child=GetDlgItem(dialog,id);
        let (w,h)=native::client_size(child);
        let mut center=[w/2,h/2];
        ClientToScreen(child,&mut center);
        ScreenToClient(dialog,&mut center);
        let point=(center[0] as u32 as i64) | ((center[1] as i64)<<32);
        w>0 && h>0 && IsWindowEnabled(child)!=0 && ChildWindowFromPointEx(dialog,point,1)==child
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
            let height=||native::client_size(unsafe{GetDlgItem(m,414)}).1;
            let compact=height();
            click(m,448);
            let expanded=height();
            click(m,448);
            click(m,449);
            if compact<=expanded || height()!=compact || !get(m,403).is_empty() || get(m,421)!="修正内容を入力" {
                unsafe {EndDialog(m,0);} return;
            }
            set(m,403,"下書き");
            msg(m,403,0xb1,0,3);
            voice_input::TEST_SHORTCUT.with(|flag|flag.set(true));
            click(m,438);
            msg(m,403,0xc2,1,wide("に追記").as_ptr() as _);
            set(m, 107, "入力テスト");
            click(m, 114);
            click(m, 2);
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
            click(m, 2);
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
            click(m, 2);
        })?;
        if a.tasks[1].due != due_shortcut(1) || a.indices().first() != Some(&0) {
            return Err("簡易登録失敗".into());
        }
        edit(h, &mut a, true, |m| { click(m,114); click(m,2); })?;
        if a.tasks.last().unwrap().title != REQUESTS[0] {
            return Err("空欄登録失敗".into());
        }
        edit(h, &mut a, true, |m| {
            msg(m, 107, 0x14e, 2, 0);
            click(m, 114);
            click(m, 2);
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
            click(m,114);
            FOUND.with(|v| v.set(v.get() && get(m,421)=="次のアクション（依頼）" && get(m,435)=="回答全文を表示" && get(m,114)=="保存しました"
                && App::load().is_ok_and(|app| app.tasks.iter().any(|t| t.next_action.contains("W2 宛先：担当者")))));

            FOUND.with(|v|v.set(v.get() && get(m,403)=="下書き\r\n- W1 方法：b：自動\r\n- W2 宛先：担当者"));
            set(m, 403, "手順を確認\r\n検証する");
            click(m, 114);
            click(m, 2);
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
        reopened.save()?;
        edit(h,&mut reopened,false,|m|{
            msg(m,108,0x14e,0,0);
            unsafe { SendMessageW(m,0x111,(1<<16)|108,0); }
            click(m,114);
            click(m,2);
        })?;
        let switched=&reopened.tasks[i];
        if switched.agent!=0 || !switched.session.is_empty() || !switched.history_key.is_empty()
            || !switched.contains_session(1,"original-claude"){
            return Err("担当AI変更後のセッション分離に失敗".into());
        }
        edit(h,&mut reopened,false,|m|{
            msg(m,108,0x14e,1,0);
            unsafe { SendMessageW(m,0x111,(1<<16)|108,0); }
            click(m,114);
            click(m,2);
        })?;
        let task=&mut reopened.tasks[i];
        if task.session!="original-claude" || task.model!="sonnet"{return Err("元の担当AIの復元に失敗".into());}
        task.other_sessions.retain(|s| !s.id.is_empty() || !s.history_key.is_empty());
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
            click(m, 2);
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
        if !ask::queue_status(&app.tasks[i]).0.ends_with("実行中"){return Err("一覧のAI実行状態".into());}
        fs::write(dir.join("closed.ok"), "").map_err(|e| e.to_string())?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while !dir.join("reply.md").is_file() && !dir.join("error.txt").is_file() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let reply = dir.join("reply.md");
        let error = dir.join("error.txt");
        fs::rename(&reply,&error).map_err(|e|e.to_string())?;
        ask::TEST_ACTION.with(|c| *c.borrow_mut() = Some(|m| {
            FOUND.with(|v| v.set(get(m,403) == "統合画面の保存テスト" && get(m,433).contains("実行失敗")));
            click(m,2);
        }));
        ask::open(h,&mut app)?;
        if !FOUND.with(|v|v.get()) { return Err("失敗時の入力復元に失敗".into()); }
        let mut newer_draft = app.clone();
        ask::TEST_ACTION.with(|c| *c.borrow_mut() = Some(|m| {
            set(m,403,"新しい下書き");
            click(m,114);
            click(m,2);
        }));
        ask::open(h,&mut newer_draft)?;
        if !ask::queue_status(&app.tasks[i]).0.ends_with("実行失敗"){return Err("一覧のAIエラー状態".into());}
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
        if !ask::queue_status(&app.tasks[i]).0.ends_with("回答受信"){return Err("一覧のAI回答状態".into());}
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

// Each phase starts a fresh editor process with its own LOCALAPPDATA and a local CLI fixture.
#[test]
fn draft_round_trip() {
    const BODY: &str = "確認してください\r\n- A1 色：青";
    const PNG: &[u8] = include_bytes!("../assets/kagari-icons-22.png");
    let Ok(phase) = std::env::var("KAGARI_TEST_DRAFT_PHASE") else {
        let root = std::env::temp_dir().join(format!("kagari-draft-test-{}-{}", std::process::id(), TaskId::default().0));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("codex.cmd"), concat!("@echo off\r\n",
            "echo invoked >> \"%LOCALAPPDATA%\\invocations\"\r\n",
            "if exist \"%LOCALAPPDATA%\\retry\" goto retry\r\n",
            "for /L %%i in (1,1,20) do (\r\n",
            " if exist \"%LOCALAPPDATA%\\closed\" exit /b 1\r\n",
            " ping -n 2 127.0.0.1 >nul\r\n)\r\nexit /b 1\r\n:retry\r\n",
            "for /L %%i in (1,1,20) do (\r\n",
            " if exist \"%LOCALAPPDATA%\\finish\" goto done\r\n",
            " ping -n 2 127.0.0.1 >nul\r\n)\r\nexit /b 1\r\n",
            ":done\r\necho done > \"%KAGARI_RUN%\\last.tmp\"\r\n")).unwrap();
        let path = std::env::join_paths(std::iter::once(root.clone())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap()))).unwrap();
        for phase in ["compose", "send", "notification", "retry", "reopen"] {
            let status = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "gui_test::draft_round_trip", "--nocapture"])
                .env("KAGARI_TEST_DRAFT_PHASE", phase).env("LOCALAPPDATA", &root)
                .env("PATH", &path).status().unwrap();
            assert!(status.success(), "{phase}");
        }
        fs::remove_dir_all(root).unwrap();
        return;
    };
    let root = PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap());
    assert_eq!(client::find(0).unwrap(), root.join("codex.cmd"));
    let history = state_path().unwrap().with_file_name("history");
    native::controls();
    unsafe { LoadLibraryW(wide("Msftedit.dll").as_ptr()); }
    let mut app = App::load().unwrap();
    app.task = Some(0);
    let action: fn(H) = match phase.as_str() {
        "compose" => {
            let project = root.to_string_lossy().into_owned();
            app.projects = vec![project.clone()];
            app.tasks.push(Task { project, history_key: "123".into(), title: "復元".into(), ..Default::default() });
            let dir = history.join("123/001");
            fs::create_dir_all(&dir).unwrap();
            let long="詳細を確認してから進める".repeat(36);
            fs::write(dir.join("reply.md"), format!("| ID | 決めること | 選択肢 |\n|---|---|---|\n| A1 | 色 | 赤<br>青<br>{long} |")).unwrap();
            let source = root.join("添付.png");
            fs::write(&source, PNG).unwrap();
            picker::TEST_IMAGE.with(|slot| *slot.borrow_mut() = Some(source));
            |h| {
                set(h,403,"");
                click(h,446);
                assert_eq!(get(h,403),editor::CONTINUE_OR_COMPLETE);
                set(h,403,"既存の📋下書き");
                msg(h,403,0xb1,0,2);
                click(h,446);
                click(h,446);
                assert_eq!(get(h,403),format!("既存の📋下書き\r\n{}",editor::CONTINUE_OR_COMPLETE));
                set(h,403,"確認してください");
                click(h,411);
                let root=PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap());
                let extra=root.join("削除対象.png");
                fs::write(&extra,PNG).unwrap();
                for _ in 0..2 {
                    picker::TEST_IMAGE.with(|slot| *slot.borrow_mut()=Some(extra.clone()));
                    click(h,411);
                    assert!(get(h,413).starts_with("2枚"));
                    unsafe{SendMessageW(h,ask::IMAGE_REMOVE,1,0);}
                    assert!(get(h,413).starts_with("1枚") && get(h,413).contains("添付.png"));
                }
                click(h,439);
                for mode in [1,3] {
                    unsafe {ShowWindow(h,mode);}
                    let rows=msg(h,683,0x18b,0,0);
                    assert!(rows>3 || msg(h,683,0x1a1,2,0)>msg(h,683,0x1a1,0,0));
                    msg(h,683,0x186,(rows-1) as usize,0);
                    unsafe{SendMessageW(h,0x111,(1<<16)|683,0);}
                    assert_eq!(get(h,684),"詳細を確認してから進める".repeat(36));
                    assert!(reachable(h,684) && reachable(h,404));
                    set(h,684,"赤青");
                    msg(h,684,0xb1,1,1);
                    voice_input::TEST_SHORTCUT.with(|flag|flag.set(true));
                    click(h,438);
                    msg(h,684,0xc2,1,wide("と").as_ptr() as _);
                    assert_eq!(get(h,684),"赤と青");
                }
                msg(h,683,0x186,1,0);
                unsafe { SendMessageW(h,0x111,(1<<16)|683,0); SendMessageW(h,0x10,0,0); }
            }
        }
        "notification" => {
            app.task = app.notification_row("123");
            assert_eq!(app.task,Some(0));
            |h| {
                assert_eq!(get(h,404),"送信済み");
                click(h,404);
                click(h,2);
            }
        }
        "send" | "retry" => {
            assert_eq!(app.tasks[0].next_action, BODY);
            assert_eq!(fs::read(&app.tasks[0].draft_images[0]).unwrap(), PNG);
            if phase == "retry" {
                fs::write(root.join("retry"), "").unwrap();
                let source = root.join("次.png");
                fs::write(&source, PNG).unwrap();
                picker::TEST_IMAGE.with(|slot| *slot.borrow_mut() = Some(source));
            }
            |h| {
                assert!(get(h,413).contains("添付.png"));
                click(h,439);
                assert_eq!(get(h,684),"青");
                let mode=if std::env::var_os("KAGARI_TEST_MAXIMIZE").is_some(){3}else{1};
                unsafe{ShowWindow(h,mode);}
                assert!(reachable(h,684));
                assert_eq!(reachable(h,404),get(h,404)=="依頼を送信");
                msg(h,404,0xf5,0,0); // BM_CLICK
                if std::env::var("KAGARI_TEST_DRAFT_PHASE").unwrap()=="send" {
                    assert_eq!(get(h,404),"実行中…");
                    assert_eq!(choice(h,109),1);
                }
                click(h,439);
                click(h,404);
                if std::env::var("KAGARI_TEST_DRAFT_PHASE").unwrap() == "retry" {
                    click(h,416);
                    click(h,445);
                    click(h,445);
                    set(h,403,"次の下書き");
                    click(h,412);
                    click(h,411);
                    click(h,439);
                    msg(h,683,0x186,0,0);
                    unsafe { SendMessageW(h,0x111,(1<<16)|683,0); }
                }
                click(h,2);
            }
        }
        "reopen" => |h| {
            assert_eq!(get(h,403),"次の下書き\r\n- A1 色：赤");
            assert!(get(h,413).contains("次.png"));
            click(h,2);
        },
        _ => unreachable!(),
    };
    if phase == "notification" {
        editor::TEST_ACTION.with(|slot| *slot.borrow_mut() = Some(action));
        editor::open(0,&mut app,false).unwrap();
    } else {
        ask::TEST_ACTION.with(|slot| *slot.borrow_mut() = Some(action));
        ask::open(0,&mut app).unwrap();
    }
    if phase == "send" {
        let original_attempt = app.tasks[0].submitted_action_run.clone();
        app = App::load().unwrap();
        assert_eq!(app.tasks[0].status,1);
        app.task = Some(0);
        ask::TEST_ACTION.with(|slot| *slot.borrow_mut() = Some(|h| {
            assert_eq!(get(h,404),"実行中…");
            click(h,404);
            click(h,2);
        }));
        ask::open(0,&mut app).unwrap();
        assert_eq!(app.tasks[0].submitted_action_run,original_attempt);
        fs::write(root.join("closed"), "").unwrap();
    }
    if phase == "notification" || phase == "reopen" {
        let count = if phase == "notification" {1} else {2};
        assert_eq!(fs::read_to_string(root.join("invocations")).unwrap().lines().count(),count);
        assert_eq!(fs::read_dir(history.join("123")).unwrap().flatten()
            .filter(|e|e.path().join("request.id").exists()).count(),count);
    }
    if phase == "compose" {
        assert_eq!(App::load().unwrap().tasks[0].status,0);
        fs::remove_file(root.join("添付.png")).unwrap();
    }
    if phase == "send" || phase == "retry" {
        let mut runs: Vec<_> = fs::read_dir(history.join("123")).unwrap().map(|e|e.unwrap().path())
            .filter(|p| p.join("request.md").is_file()).collect();
        runs.sort();
        assert_eq!(runs.len(), if phase == "send" {1} else {2});
        let dir = runs.last().unwrap();
        let request_id = fs::read_to_string(runs[0].join("request.id")).unwrap();
        assert_eq!(request_id,format!("123/{}",runs[0].file_name().unwrap().to_string_lossy()));
        assert_eq!(fs::read_to_string(dir.join("request.id")).unwrap(),request_id);
        assert_eq!(fs::read_to_string(dir.join("request.md")).unwrap(), format!("{BODY}\n\n![添付画像 1](images/0001.png)"));
        assert_eq!(fs::read(dir.join("images/0001.png")).unwrap(), PNG);
        let result = if phase == "send" { "error.txt" } else {
            fs::write(root.join("finish"), "").unwrap();
            "reply.md"
        };
        let start = std::time::Instant::now();
        while !dir.join(result).exists() {
            assert!(start.elapsed().as_secs() < 30, "waiting for {result}");
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}
