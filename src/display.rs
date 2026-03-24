use crate::db::{parse_depth, Annotation, Event, Goal, GoalKind};
use std::collections::HashMap;

pub fn print_tree(goals: &[Goal], tags: &HashMap<String, Vec<String>>, ranks: &HashMap<String, usize>) {
    let mut by_parent: HashMap<Option<String>, Vec<&Goal>> = HashMap::new();
    for goal in goals {
        by_parent.entry(goal.parent_id.clone()).or_default().push(goal);
    }
    for bucket in by_parent.values_mut() {
        bucket.sort_by(|a, b| {
            let ra = ranks.get(&a.id).copied().unwrap_or(usize::MAX);
            let rb = ranks.get(&b.id).copied().unwrap_or(usize::MAX);
            ra.cmp(&rb).then(a.id.cmp(&b.id))
        });
    }
    print_node(&by_parent, tags, None, 0);
}

pub fn print_info(subtree: &[Goal], annotations: &[Annotation], tags: &HashMap<String, Vec<String>>) {
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
    let root_tags = tags.get(&root.id).map(Vec::as_slice).unwrap_or(&[]);
    if !root_tags.is_empty() {
        let tag_str: Vec<String> = root_tags.iter().map(|t| format!("+{}", t)).collect();
        println!("tags:   {}", tag_str.join(" "));
    }
    if !annotations.is_empty() {
        println!();
        println!("annotations:");
        for ann in annotations {
            println!("  {}  {}  {}", ann.id, ann.created_at, ann.body);
        }
    }
    if subtree.len() > 1 {
        println!();
        println!("subtree:");
        let mut by_parent: HashMap<Option<String>, Vec<&Goal>> = HashMap::new();
        for goal in subtree {
            by_parent.entry(goal.parent_id.clone()).or_default().push(goal);
        }
        for bucket in by_parent.values_mut() {
            bucket.sort_by(|a, b| a.id.cmp(&b.id));
        }
        let root_depth = parse_depth(&root.id) as usize;
        print_node_offset(&by_parent, tags, Some(root.id.clone()), root_depth + 1, root_depth + 1);
    }
}

fn print_node_offset(
    map: &HashMap<Option<String>, Vec<&Goal>>,
    tags: &HashMap<String, Vec<String>>,
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
        let tag_suffix = format_tags(tags.get(&goal.id).map(Vec::as_slice).unwrap_or(&[]));
        println!("{}{}  {}  {}{}", indent, marker, goal.id, goal.body, tag_suffix);
        print_node_offset(map, tags, Some(goal.id.clone()), depth + 1, base_depth);
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

fn format_tags(tags: &[String]) -> String {
    if tags.is_empty() {
        String::new()
    } else {
        let parts: Vec<String> = tags.iter().map(|t| format!("+{}", t)).collect();
        format!("  {}", parts.join(" "))
    }
}

fn print_node(map: &HashMap<Option<String>, Vec<&Goal>>, tags: &HashMap<String, Vec<String>>, parent_id: Option<String>, depth: usize) {
    let Some(children) = map.get(&parent_id) else {
        return;
    };
    for goal in children {
        let indent = "    ".repeat(depth);
        let marker = match goal.kind {
            GoalKind::Achievable => if goal.achieved { "[x]" } else { "[ ]" },
            GoalKind::Continuous => "[~]",
        };
        let tag_suffix = format_tags(tags.get(&goal.id).map(Vec::as_slice).unwrap_or(&[]));
        println!("{}{}  {}  {}{}", indent, marker, goal.id, goal.body, tag_suffix);
        print_node(map, tags, Some(goal.id.clone()), depth + 1);
    }
}
