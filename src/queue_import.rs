use super::*;

pub(super) const AGENT_INSTRUCTIONS: &str = r#"KAGARI作業キューへの登録方法：利用者が作業の分割登録や都市別タスクの作成・キューへの表示を依頼した場合、回答の末尾に次の形式を出力するとKAGARIが選択プロジェクトに登録する。GUI操作や保存ファイルの直接編集は不要。進捗確認・相談だけの場合は出力しない。1件ごとに固有のタイトルと具体的な次の作業を記載する。既存の作業票番号、都市名、保留条件を保持する。登録は未着手で、作業自体は開始しない。
```kagari-queue
[{"title":"作業票番号と都市名など","action":"次に行うこと・資料","condition":"作業票や依頼に基づく完了条件を簡潔に（不明なら空欄）"}]
```"#;

fn ticket(title:&str)->Option<&str> {
    title.split(|c:char| !c.is_ascii_alphanumeric()).find(|word| word.strip_prefix('Q').is_some_and(|n| !n.is_empty() && n.bytes().all(|b|b.is_ascii_digit())))
}

#[derive(Clone)]
pub(super) struct Receipt {
    pub project: String,
    pub parent_key: String,
    pub added: Vec<String>,
    pub existing: usize,
    pub failed: usize,
}

pub(super) fn collect(app: &mut App) -> Result<bool> {
    let inbox=state_path()?.with_file_name("queue-inbox");
    let files=match fs::read_dir(inbox) {
        Ok(files)=>files,
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(false),
        Err(e)=>return Err(e.to_string()),
    };
    let mut changed=false;
    for file in files.flatten() {
        let path=file.path();
        if path.extension().is_some_and(|x| x == "docs") {
            let body=fs::read_to_string(&path).map_err(|e|e.to_string())?;
            let apply=(|| -> Result<()> {
                let fields: Vec<_>=body.trim_end_matches(['\r','\n']).split('\t').collect();
                let [id,spec,log,result]=fields.as_slice() else{return Err("invalid document request".into())};
                let id=TaskId::parse(id)?;
                let mut next=app.clone();
                let task=next.tasks.iter_mut().find(|t|t.id==id).ok_or("task missing")?;
                for (slot,rel) in [spec,log,result].iter().enumerate(){
                    if rel.is_empty(){continue}
                    if !task.docs[slot].is_empty() && task.docs[slot]!=**rel {return Err("document already assigned".into())}
                    let file=doc_path(&task.project,rel)?;
                    let resolved=file.canonicalize().map_err(|e|e.to_string())?;
                    let root=Path::new(&task.project).canonicalize().map_err(|e|e.to_string())?;
                    if !resolved.starts_with(root) || !resolved.is_file(){return Err("project document missing".into())}
                    task.docs[slot]=rel.to_string();
                }
                next.save()?;*app=next;Ok(())
            })();
            let receipt=match apply{Ok(())=>{changed=true;"saved".to_owned()},Err(e)=>format!("rejected: {e}")};
            fs::write(path.with_extension("receipt"),receipt).and_then(|_|fs::remove_file(&path)).map_err(|e|e.to_string())?;
            continue;
        }
        if path.extension().is_some_and(|x| x == "status") {
            changed |= super::task_status::consume(app, &path, &state_path()?)?;
            continue;
        }
        if path.extension().is_none_or(|x|x!="queue"){continue;}
        let key=file.file_name().to_string_lossy().into_owned();
        if !app.imported_batches.contains(&key) {
            let body=fs::read_to_string(&path).map_err(|e|e.to_string())?;
            let mut lines=body.lines();
            let Some(project)=lines.next().and_then(|p|app.projects.iter().find(|v| v.eq_ignore_ascii_case(p))) else {continue};
            let agent=match lines.next(){Some("claude")=>1,Some("codex")=>0,_=>continue};
            let mut lines=lines.peekable();
            let parent_key=lines.peek().and_then(|s|s.strip_prefix("parent:")).filter(|key|!key.contains('\t'));
            if parent_key.is_some(){lines.next();}
            let parent=parent_key.filter(|k|!k.is_empty()).and_then(|key|
                app.tasks.iter().position(|t|t.project.eq_ignore_ascii_case(project)
                    && (t.history_key==key || t.other_sessions.iter().any(|s|s.history_key==key))));
            let mut receipt=Receipt{project:project.clone(),parent_key:parent_key.unwrap_or("").into(),added:Vec::new(),existing:0,failed:0};
            let mut next=app.clone();
            for row in lines {
                let mut fields=row.split('\t');
                let title=fields.next().unwrap_or_default();
                let action=fields.next().unwrap_or_default();
                let condition=fields.next().unwrap_or_default().trim();
                if title.trim().is_empty() || action.trim().is_empty() || condition.chars().count()>240 || fields.next().is_some() {
                    receipt.failed+=1;
                    continue;
                };
                if parent_key.is_some() && parent.is_none(){receipt.failed+=1;continue;}
                let city_ticket = (project_alias(Path::new(project)) == Some("chromarch")).then(||ticket(title)).flatten();
                if next.tasks.iter().any(|t|t.project.eq_ignore_ascii_case(project)
                    && (t.title.eq_ignore_ascii_case(title) || city_ticket.is_some_and(|id|ticket(&t.title)==Some(id)))) {receipt.existing+=1;continue;}

                next.tasks.push(Task{parent:parent.map(|i|app.tasks[i].id),project:project.clone(),title:title.into(),next_action:action.into(),completion_condition:condition.into(),agent,..Default::default()});
                receipt.added.push(title.into());
            }
            next.imported_batches.insert(key);
            next.queue_receipts.push(receipt);
            next.save()?;
            *app=next;
            changed=true;
        }
        let _=fs::remove_file(path);
    }
    Ok(changed)
}
