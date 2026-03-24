use crate::db::{parse_depth, Event, Goal, GoalKind};
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

pub fn print_info(subtree: &[Goal]) {
    let root = &subtree[0];
    let kind_str = match root.kind {
        GoalKind::Achievable => "achievable",
        GoalKind::Continuous => "continuous",
    };
    let status = match root.kind {
        GoalKind::Achievable => if root.achieved { "achieved" } else { "not achieved" },
        GoalKind::Continuous => "continuous",
    };
    println!("id:     {}", root.id);
    println!("kind:   {}", kind_str);
    println!("status: {}", status);
    println!("depth:  {}", parse_depth(&root.id));
    println!("parent: {}", root.parent_id.as_deref().unwrap_or("none"));
    println!("body:   {}", root.body);
    if subtree.len() > 1 {
        println!();
        println!("subtree:");
        // Print the subtree rooted at root, reusing print_node logic
        let mut by_parent: HashMap<Option<String>, Vec<&Goal>> = HashMap::new();
        for goal in subtree {
            by_parent.entry(goal.parent_id.clone()).or_default().push(goal);
        }
        for bucket in by_parent.values_mut() {
            bucket.sort_by(|a, b| a.id.cmp(&b.id));
        }
        let root_depth = parse_depth(&root.id) as usize;
        print_node_offset(&by_parent, Some(root.id.clone()), root_depth + 1, root_depth + 1);
    }
}

fn print_node_offset(
    map: &HashMap<Option<String>, Vec<&Goal>>,
    parent_id: Option<String>,
    depth: usize,
    base_depth: usize,
) {
    let Some(children) = map.get(&parent_id) else { return };
    for goal in children {
        let indent = "    ".repeat(depth - base_depth);
        let marker = match goal.kind {
            GoalKind::Achievable => if goal.achieved { "[x]" } else { "[ ]" },
            GoalKind::Continuous => "[~]",
        };
        println!("{}{}  {}  {}", indent, marker, goal.id, goal.body);
        print_node_offset(map, Some(goal.id.clone()), depth + 1, base_depth);
    }
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
