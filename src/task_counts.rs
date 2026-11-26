use super::{ask, Task, TaskId, STATES};
use std::collections::HashMap;

pub(super) const FOLLOWUP: usize = STATES.len();
pub(super) const PROGRESS_STATES: [&str; 5] = [STATES[0], STATES[1], STATES[2], STATES[3], "次の作業依頼待ち"];

pub(super) fn progress_state(task: &Task) -> usize {
    if ask::awaiting_followup(task) { FOLLOWUP } else { task.status }
}

pub(super) fn task_counts(
    tasks: &[Task],
) -> (HashMap<&str, [usize; 5]>, HashMap<TaskId, [usize; 4]>) {
    let mut projects = HashMap::new();
    let mut children = HashMap::new();
    for task in tasks {
        projects.entry(task.project.as_str()).or_insert([0; 5])[progress_state(task)] += 1;
        if let Some(parent) = task.parent {
            children.entry(parent).or_insert([0; 4])[task.status] += 1;
        }
    }
    (projects, children)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_children_and_project_totals_follow_independent_task_states() {
        let mut tasks = [
            ("a", None, 1),
            ("a", Some(0), 3),
            ("a", Some(0), 2),
            ("a", Some(1), 3),
            ("b", None, 2),
        ]
        .into_iter()
        .enumerate()
        .map(|(row, (project, parent, status))| Task {
            id: TaskId(row as u128),
            project: project.into(),
            parent: parent.map(TaskId),
            status,
            ..Default::default()
        })
        .collect::<Vec<_>>();
        let (projects, before) = task_counts(&tasks);
        assert_eq!(projects["a"], [0, 1, 1, 2, 0]);
        assert_eq!(projects["b"], [0, 0, 1, 0, 0]);
        assert_eq!(before[&tasks[0].id], [0, 0, 1, 1]);
        assert_eq!(before[&tasks[1].id], [0, 0, 0, 1]);
        tasks[0].status = 3;
        let (projects, after) = task_counts(&tasks);
        assert_eq!(before, after);
        assert_eq!(projects["a"], [0, 0, 1, 3, 0]);
        tasks[2].status = 3;
        let (projects, children) = task_counts(&tasks);
        assert_eq!(children[&tasks[0].id], [0, 0, 0, 2]);
        assert_eq!(projects["a"], [0, 0, 0, 4, 0]);
    }
}
