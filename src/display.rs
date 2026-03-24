use crate::db::{Event, Goal, GoalKind};
use std::collections::HashMap;

pub fn print_tree(goals: &[Goal]) {
    let mut by_parent: HashMap<Option<String>, Vec<&Goal>> = HashMap::new();
    for goal in goals {
        by_parent.entry(goal.parent_id.clone()).or_default().push(goal);
    }
    for bucket in by_parent.values_mut() {
        bucket.sort_by(|a, b| a.id.cmp(&b.id));
    }
    print_node(&by_parent, None, 0);
}

pub fn print_log(events: &[Event]) {
    for e in events {
        println!(
            "{}  {}  {:<8}  {}  \"{}\"",
            e.event_id, e.timestamp, e.op, e.goal_id, e.goal_body
        );
    }
}

fn print_node(map: &HashMap<Option<String>, Vec<&Goal>>, parent_id: Option<String>, depth: usize) {
    let Some(children) = map.get(&parent_id) else {
        return;
    };
    for goal in children {
        let indent = "    ".repeat(depth);
        let marker = match goal.kind {
            GoalKind::Achievable => if goal.achieved { "[x]" } else { "[ ]" },
            GoalKind::Continuous => "[~]",
        };
        println!("{}{}  {}  {}", indent, marker, goal.id, goal.body);
        print_node(map, Some(goal.id.clone()), depth + 1);
    }
}
