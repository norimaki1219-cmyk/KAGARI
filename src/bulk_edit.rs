use super::*;
use std::collections::{BTreeSet, HashSet};

pub(super) fn selection(app: &App, ids: &str) -> Result<BTreeSet<usize>> {
    let visible: HashSet<_> = app.indices().into_iter().collect();
    ids.split(',').map(|s| s.parse().ok().filter(|i| visible.contains(i))
        .ok_or_else(|| "表示中のタスクを選んでください".into())).collect()
}

fn choice(value: &str, limit: usize) -> Result<Option<usize>> {
    if value == "keep" { return Ok(None); }
    value.parse().ok().filter(|&i| i < limit).map(Some)
        .ok_or_else(|| "変更値が不正です".into())
}

pub(super) fn apply(app: &mut App, input: &str) -> Result<Vec<(usize, Task)>> {
    let parts: Vec<_> = input.split('|').collect();
    let [ids, parent, agent, state] = parts.as_slice() else { return Err("変更項目が不正です".into()); };
    let ids = selection(app, ids)?;
    let parent = match *parent {
        "root" => Some(None),
        value => choice(value, app.tasks.len())?.map(Some),
    };
    if let Some(Some(p)) = parent {
        if ids.iter().any(|&i| !app.can_parent(i, p)) {
            return Err("全対象に共通する親タスクを選んでください".into());
        }
    }
    let agent = choice(agent, 2)?;
    let state = choice(state, STATES.len())?;
    let parent = parent.map(|p| p.map(|row| app.tasks[row].id));
    let mut originals = Vec::new();
    for i in ids {
        let t = &mut app.tasks[i];
        if parent.is_some_and(|p| p != t.parent) || agent.is_some_and(|v| v != t.agent)
            || state.is_some_and(|v| v != t.status) {
            originals.push((i, t.clone()));
            if let Some(p) = parent { t.parent = p; }
            if let Some(a) = agent { t.select_agent(a); }
            if let Some(s) = state { t.status = s; }
        }
    }
    if originals.is_empty() { return Err("選択したタスクはすでに指定の値です".into()); }
    Ok(originals)
}

pub(super) fn undo(app: &mut App) -> Result<()> {
    if !app.bulk_undo.as_ref().is_some_and(|(p, _)| *p == app.project) {
        return Err("取り消せる一括変更がありません".into());
    }
    for (id, original) in app.bulk_undo.take().unwrap().1 {
        app.tasks[id] = original;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restores_each_tasks_values_and_active_session() {
        let mut app = App::default();
        app.projects.push("project".into());
        app.tasks = (0..4).map(|i| Task { project: "project".into(), title: i.to_string(), ..Default::default() }).collect();
        let first = app.tasks[0].id;
        let last = app.tasks[3].id;
        app.tasks[1].parent = Some(first);
        app.tasks[1].session = "original".into();
        app.tasks[2].agent = 1;
        app.tasks[2].status = 1;
        assert!(apply(&mut app, "1,2|1|0|3").is_err());
        let originals = apply(&mut app, "1,2|3|1|3").unwrap();
        for t in &app.tasks[1..=2] {
            assert_eq!((t.parent,t.agent,t.status), (Some(last),1,3));
        }
        app.bulk_undo = Some((0, originals));
        undo(&mut app).unwrap();
        assert_eq!((app.tasks[1].parent,app.tasks[1].agent,app.tasks[1].status), (Some(first),0,0));
        assert_eq!((app.tasks[2].parent,app.tasks[2].agent,app.tasks[2].status), (None,1,1));
        assert_eq!(app.tasks[1].session, "original");
        assert!(app.tasks[1].other_sessions.is_empty());
        assert!(undo(&mut app).is_err());
    }
}
