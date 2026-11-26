use super::*;

// The writer publishes a complete file by rename. Both the desktop and the
// offline command apply requests through App::save_to while owning the app mutex.
pub(super) fn consume(app: &mut App, request: &Path, state: &Path) -> Result<bool> {
    let body = fs::read_to_string(request).map_err(|e| e.to_string())?;
    let fields: Vec<_> = body.trim_end_matches(['\r','\n']).split('\t').collect();
    let parsed = (|| -> Result<_> {
        let [key, raw_id, old, new, origin @ ..] = fields.as_slice() else {return Err("invalid request".into())};
        let key = TaskId::parse(key)?.to_string();
        let id = TaskId::parse(raw_id)?;
        let status = |s: &str| s.parse::<usize>().ok().filter(|n| *n < STATES.len()).ok_or("invalid status".to_string());
        if origin.len()>1 {return Err("invalid request".into())}
        Ok((key, id, status(old)?, status(new)?,origin.first().copied()))
    })();
    let result = parsed.and_then(|(key,id,old,new,origin)| {
        let dedup = format!("status-{key}");
        if app.imported_batches.contains(&dedup) {return Ok(None)}
        let index = app.tasks.iter().position(|t| t.id == id).ok_or("task not found")?;
        let task=&app.tasks[index];
        if let Some(origin)=origin {
            let (history,attempt)=origin.split_once('/').ok_or("invalid attempt")?;
            if history!=task.history_key || history.is_empty() || !history.bytes().all(|b|b.is_ascii_digit() || b==b'-') {return Err("task session changed".into())}
            let latest=fs::read_dir(state.with_file_name("history").join(history)).map_err(|e|e.to_string())?
                .flatten().filter(|e|e.path().join("request.id").is_file())
                .map(|e|e.file_name().to_string_lossy().into_owned()).max();
            if latest.as_deref()!=Some(attempt){return Err("a newer request exists".into())}
        }
        let current = task.status;
        if current != old && current != new {return Err("task status changed".into())}
        let mut next = app.clone();
        next.tasks[index].status = new;
        next.imported_batches.insert(dedup);
        Ok(Some(next))
    });
    let changed = matches!(result, Ok(Some(_)));
    let receipt = match result {
        Ok(Some(mut next)) => {
            next.save_to(state)?;
            *app = next;
            "saved".to_owned()
        }
        Ok(None) => "saved".to_owned(),
        Err(e) => format!("rejected: {e}"),
    };
    let tmp = request.with_extension("receipt-tmp");
    fs::write(&tmp, receipt).and_then(|_|fs::rename(tmp,request.with_extension("receipt")))
        .and_then(|_|fs::remove_file(request)).map_err(|e|e.to_string())?;
    Ok(changed)
}

// Called by Set-TaskStatus.ps1 only after it acquires the desktop's named mutex.
#[no_mangle]
pub extern "system" fn kagari_apply_pending_offline() -> i32 {
    let result = App::load().and_then(|mut app| queue_import::collect(&mut app));
    if result.is_ok() {0} else {1}
}
