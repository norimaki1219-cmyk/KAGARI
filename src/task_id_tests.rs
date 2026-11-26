use super::*;

// Derive expectations directly from the old format, before calling the migrator.
fn check_migration(legacy: &str) {
    let mut tasks = Vec::new();
    let mut parents = Vec::new();
    let mut sessions = Vec::new();
    for line in legacy.lines() {
        let f: Vec<_> = line.split('\t').map(decode).collect();
        match f[0].as_str() {
            "T" => {
                assert!(f.len() <= 20);
                tasks.push(f);
            }
            "TP" => parents.push((f[1].parse::<usize>().unwrap(), f[2].parse::<usize>().unwrap())),
            "S" => sessions.push((tasks.len()-1, f)),
            _ => (),
        }
    }
    let directory = std::env::temp_dir().join(format!("kagari-{}",TaskId::default()));
    let history = directory.join("kpi-history");
    fs::create_dir_all(&history).unwrap();
    let point = r#"{"Received":"/Date(1234567890000)/","All":{"Total":3,"Done":1},"Projects":[],"Agents":[],"Memo":"改善策\n確認待ちを毎日確認"}"#;
    fs::write(history.join("observation.json"), point).unwrap();
    let path = directory.join("workspace.tsv");
    fs::write(&path, legacy).unwrap();
    let mut app = App::load_from(&path).unwrap();
    let ids: Vec<_> = app.tasks.iter().map(|t|t.id).collect();
    assert_eq!(ids.len(),tasks.len());
    assert_eq!(ids.iter().collect::<std::collections::HashSet<_>>().len(), ids.len());
    let check = |app: &App, deleted: Option<usize>| {
        for (old,f) in tasks.iter().enumerate() {
            let row = app.task_row(ids[old]);
            if deleted == Some(old) {
                assert!(row.is_none());
                continue;
            }
            let row = row.unwrap();
            let task = &app.tasks[row];
            assert_eq!(task.title,f[2]);
            let parent = parents.iter().find(|(child,_)|*child==old).map(|&(_,p)|p);
            let parent = parent.and_then(|p| if Some(p)==deleted {
                parents.iter().find(|(child,_)|*child==p).map(|&(_,gp)|gp)
            } else {Some(p)});
            assert_eq!(task.parent,parent.map(|p|ids[p]));
            let verify_session = |agent: &str, session: &str, key: &str| {
                if !session.is_empty() {assert!(task.contains_session(agent.parse().unwrap(),session));}
                if !key.is_empty() {assert_eq!(app.notification_row(key),Some(row));}
            };
            verify_session(&f[3],f.get(14).map_or("",String::as_str),f.get(12).map_or("",String::as_str));
            let additional: Vec<_> = sessions.iter().filter(|(owner,_)|*owner==old).collect();
            assert_eq!(task.other_sessions.len(),additional.len());
            for (_,s) in additional {verify_session(&s[1],&s[2],&s[5]);}
        }
    };
    check(&app,None);
    let points = fs::read_dir(directory.join("restore-points")).unwrap()
        .map(|item|item.unwrap().path()).collect::<Vec<_>>();
    assert_eq!(points.len(),1);
    let restored = directory.join("restored");
    let restore = || Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-ExecutionPolicy","RemoteSigned","-File"])
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/recovery.ps1"))
        .arg("-Mode").arg("Restore").arg("-Source").arg(&points[0])
        .arg("-Destination").arg(&restored).output().unwrap();
    let result = restore();
    assert!(result.status.success(),"{}",String::from_utf8_lossy(&result.stderr));
    let restored_path = restored.join("workspace.tsv");
    assert_eq!(fs::read_to_string(&restored_path).unwrap(),legacy);
    assert_eq!(fs::read_to_string(restored.join("kpi-history/observation.json")).unwrap(),point);
    let restored_app = App::load_from(&restored_path).unwrap();
    for (i,task) in restored_app.tasks.iter().enumerate() {
        assert_eq!(task.title,tasks[i][2]);
        assert_eq!(task.parent.and_then(|id|restored_app.task_row(id)),
            parents.iter().find(|(child,_)|*child==i).map(|&(_,parent)|parent));
        assert_eq!(task.session,app.tasks[i].session);
        assert!(task.other_sessions.iter().map(|s|(&s.id,s.agent,&s.history_key))
            .eq(app.tasks[i].other_sessions.iter().map(|s|(&s.id,s.agent,&s.history_key))));
    }
    fs::remove_dir_all(&restored).unwrap();
    fs::write(points[0].join("kpi-history/observation.json"),"{}").unwrap();
    let result = restore();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("observation.json"));
    assert!(!restored.exists());
    // Reorder the actual T records independently from their saved session records.
    let saved = fs::read_to_string(&path).unwrap();
    let (mut rows,other): (Vec<_>,Vec<_>) = saved.lines().partition(|l|l.starts_with("T\t"));
    rows.reverse();
    rows.extend(other);
    fs::write(&path, rows.join("\n")).unwrap();
    app = App::load_from(&path).unwrap();
    check(&app,None);
    let deleted = parents[0].1;
    let row = app.task_row(ids[deleted]).unwrap();
    let key = app.tasks[row].history_key.clone();
    app.remove_task_row(row);
    let added = Task { title:tasks[deleted][2].clone(), project:tasks[deleted][1].clone(), ..Default::default() };
    assert!(!ids.contains(&added.id));
    let new_id = added.id;
    app.tasks.insert(0,added);
    app.save_to(&path).unwrap();
    let app = App::load_from(&path).unwrap();
    assert_eq!(app.tasks[0].id,new_id);
    assert_eq!(app.notification_row(&key),None);
    check(&app,Some(deleted));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn failed_checkpoint_preserves_legacy_workspace_and_bad_row_is_identified() {
    let directory = std::env::temp_dir().join(format!("kagari-{}",TaskId::default()));
    fs::create_dir(&directory).unwrap();
    let path = directory.join("workspace.tsv");
    let legacy = ["T","project","parent","0","0","","","","","",""].join("\t");
    fs::write(&path,&legacy).unwrap();
    fs::write(directory.join("restore-points"),"blocked").unwrap();
    assert!(App::load_from(&path).err().unwrap().contains("復元点"));
    assert_eq!(fs::read_to_string(&path).unwrap(),legacy);
    fs::write(&path,format!("{legacy}\nH\tproject\t0\t0\tbad\t0\t0")).unwrap();
    let error = App::load_from(&path).err().unwrap();
    assert!(error.contains("workspace.tsv 2行目 (H)"),"{error}");
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn migrate_reorder_delete_and_add() {
    let task = |name,key,session| ["T","project",name,"0","0","","","","","","","0",key,"",session].join("\t");
    check_migration(&[task("same","11-01","one"),task("same","22-01","two"),
        "S\t1\tthree\tother\t\t33-01\t\t".into(),task("child","",""),
        "TP\t2\t1".into(),"TP\t1\t0".into()].join("\n"));
}

#[test]
#[ignore = "requires a read-only legacy workspace snapshot via KAGARI_LEGACY_SNAPSHOT"]
fn verify_existing_city_snapshot() {
    let legacy = fs::read_to_string(std::env::var_os("KAGARI_LEGACY_SNAPSHOT").unwrap()).unwrap();
    for ticket in 301..=324 {
        assert_eq!(legacy.lines().filter(|l|l.starts_with("T\t") &&
            l.split('\t').nth(2).unwrap().starts_with(&format!("Q{ticket} "))).count(),1);
    }
    check_migration(&legacy);
}
