use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn goal_bin() -> String {
    std::env::var("CARGO_BIN_EXE_goal").unwrap_or_else(|_| {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        format!("{}/target/debug/goal", manifest_dir)
    })
}

fn goal(data_dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(goal_bin())
        .env("GOAL_DATA_DIR", data_dir)
        .args(args)
        .output()
        .expect("failed to run goal binary")
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn add_achievable_shows_in_list() {
    let dir = TempDir::new().unwrap();
    let out = goal(dir.path(), &["add", "learn rust"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let id = stdout(&out).trim().to_string();
    assert!(id.starts_with('a'), "id should start with 'a', got: {}", id);
    assert_eq!(&id[1..2], "0", "top-level depth should be 0, got: {}", id);

    let list = goal(dir.path(), &["list"]);
    assert!(list.status.success());
    let s = stdout(&list);
    assert!(s.contains("[ ]"), "should show [ ]");
    assert!(s.contains("learn rust"));
}

#[test]
fn add_continuous_shows_in_list() {
    let dir = TempDir::new().unwrap();
    let out = goal(dir.path(), &["add", "--continuous", "maintain health"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let id = stdout(&out).trim().to_string();
    assert!(id.starts_with('c'), "id should start with 'c', got: {}", id);

    let list = goal(dir.path(), &["list"]);
    let s = stdout(&list);
    assert!(s.contains("[~]"));
    assert!(s.contains("maintain health"));
}

#[test]
fn add_subgoal_depth_increments() {
    let dir = TempDir::new().unwrap();
    let parent_out = goal(dir.path(), &["add", "parent goal"]);
    assert!(parent_out.status.success());
    let parent_id = stdout(&parent_out).trim().to_string();

    let child_out = goal(dir.path(), &["add", "child goal", "--parent", &parent_id]);
    assert!(child_out.status.success(), "{}", stderr(&child_out));
    let child_id = stdout(&child_out).trim().to_string();

    assert!(child_id.starts_with('a'));
    assert_eq!(&child_id[1..2], "1", "subgoal depth should be 1, got: {}", child_id);

    let list = goal(dir.path(), &["list"]);
    let s = stdout(&list);
    assert!(s.contains("parent goal"));
    assert!(s.contains("child goal"));
    let parent_pos = s.find("parent goal").unwrap();
    let child_pos = s.find("child goal").unwrap();
    assert!(parent_pos < child_pos, "parent should appear before child");
}

#[test]
fn done_and_undone() {
    let dir = TempDir::new().unwrap();
    let out = goal(dir.path(), &["add", "read a book"]);
    let id = stdout(&out).trim().to_string();

    let done = goal(dir.path(), &["done", &id]);
    assert!(done.status.success(), "{}", stderr(&done));

    let list = goal(dir.path(), &["list"]);
    assert!(stdout(&list).contains("[x]"));

    let undone = goal(dir.path(), &["undone", &id]);
    assert!(undone.status.success());

    let list2 = goal(dir.path(), &["list"]);
    assert!(stdout(&list2).contains("[ ]"));
}

#[test]
fn done_on_continuous_fails() {
    let dir = TempDir::new().unwrap();
    let out = goal(dir.path(), &["add", "--continuous", "exercise"]);
    let id = stdout(&out).trim().to_string();

    let done = goal(dir.path(), &["done", &id]);
    assert!(!done.status.success());
    assert!(stderr(&done).contains("continuous"));
}

#[test]
fn rm_cascades_to_subgoals() {
    let dir = TempDir::new().unwrap();
    let parent = stdout(&goal(dir.path(), &["add", "parent"])).trim().to_string();
    goal(dir.path(), &["add", "child", "--parent", &parent]);

    let rm = goal(dir.path(), &["delete", &parent]);
    assert!(rm.status.success(), "{}", stderr(&rm));

    let list = goal(dir.path(), &["list"]);
    let s = stdout(&list);
    assert!(!s.contains("parent"));
    assert!(!s.contains("child"));
}

#[test]
fn short_prefix_resolves() {
    let dir = TempDir::new().unwrap();
    let out = goal(dir.path(), &["add", "single goal"]);
    let id = stdout(&out).trim().to_string();
    let prefix = &id[..4];

    let done = goal(dir.path(), &["done", prefix]);
    assert!(done.status.success(), "{}", stderr(&done));
}

#[test]
fn ambiguous_prefix_errors() {
    let dir = TempDir::new().unwrap();
    // Add two achievable goals — using prefix 'a' matches both
    goal(dir.path(), &["add", "goal one"]);
    goal(dir.path(), &["add", "goal two"]);

    let done = goal(dir.path(), &["done", "a"]);
    assert!(!done.status.success());
    assert!(stderr(&done).contains("ambiguous"));
}

#[test]
fn modify_body_only() {
    let dir = TempDir::new().unwrap();
    let id = stdout(&goal(dir.path(), &["add", "old body"])).trim().to_string();

    let out = goal(dir.path(), &["modify", &id, "--body", "new body"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out).trim(), id, "ID should not change on body-only modify");

    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(list.contains("new body"));
    assert!(!list.contains("old body"));
}

#[test]
fn modify_kind_achievable_to_continuous() {
    let dir = TempDir::new().unwrap();
    let id = stdout(&goal(dir.path(), &["add", "some goal"])).trim().to_string();
    assert!(id.starts_with('a'));

    let out = goal(dir.path(), &["modify", &id, "--continuous"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let new_id = stdout(&out).trim().to_string();
    assert!(new_id.starts_with('c'), "new ID should start with 'c', got: {}", new_id);

    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(list.contains("[~]"));
    assert!(list.contains("some goal"));
}

#[test]
fn modify_kind_continuous_to_achievable() {
    let dir = TempDir::new().unwrap();
    let id = stdout(&goal(dir.path(), &["add", "--continuous", "ongoing"])).trim().to_string();
    assert!(id.starts_with('c'));

    let out = goal(dir.path(), &["modify", &id, "--achievable"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let new_id = stdout(&out).trim().to_string();
    assert!(new_id.starts_with('a'), "new ID should start with 'a', got: {}", new_id);

    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(list.contains("[ ]"));
}

#[test]
fn modify_reparent() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "goal A"])).trim().to_string();
    let b = stdout(&goal(dir.path(), &["add", "goal B"])).trim().to_string();
    assert_eq!(&b[1..2], "0");

    let out = goal(dir.path(), &["modify", &b, "--parent", &a]);
    assert!(out.status.success(), "{}", stderr(&out));
    let new_b = stdout(&out).trim().to_string();
    assert_eq!(&new_b[1..2], "1", "reparented goal should have depth 1, got: {}", new_b);

    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(list.contains("goal A"));
    assert!(list.contains("goal B"));
}

#[test]
fn modify_reparent_updates_children() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "A"])).trim().to_string();
    let b = stdout(&goal(dir.path(), &["add", "B", "--parent", &a])).trim().to_string();
    let c = stdout(&goal(dir.path(), &["add", "C", "--parent", &b])).trim().to_string();
    assert_eq!(&b[1..2], "1");
    assert_eq!(&c[1..2], "2");

    // Detach B to top level; C should become depth 1
    let out = goal(dir.path(), &["modify", &b, "--no-parent"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let new_b = stdout(&out).trim().to_string();
    assert_eq!(&new_b[1..2], "0", "detached goal should be depth 0, got: {}", new_b);

    let list = stdout(&goal(dir.path(), &["list"]));
    // Old B and C IDs should be gone, list should still contain their bodies
    assert!(!list.contains(&b), "old B id should be gone");
    assert!(!list.contains(&c), "old C id should be gone");
    assert!(list.contains("B"));
    assert!(list.contains("C"));
}

#[test]
fn modify_detach() {
    let dir = TempDir::new().unwrap();
    let p = stdout(&goal(dir.path(), &["add", "parent"])).trim().to_string();
    let c = stdout(&goal(dir.path(), &["add", "child", "--parent", &p])).trim().to_string();
    assert_eq!(&c[1..2], "1");

    let out = goal(dir.path(), &["modify", &c, "--no-parent"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let new_c = stdout(&out).trim().to_string();
    assert_eq!(&new_c[1..2], "0", "detached child should be depth 0");

    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(list.contains("child"));
}

#[test]
fn modify_no_flags_errors() {
    let dir = TempDir::new().unwrap();
    let id = stdout(&goal(dir.path(), &["add", "a goal"])).trim().to_string();
    let out = goal(dir.path(), &["modify", &id]);
    assert!(!out.status.success());
}

#[test]
fn modify_parent_and_no_parent_conflict() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "A"])).trim().to_string();
    let b = stdout(&goal(dir.path(), &["add", "B"])).trim().to_string();
    let out = goal(dir.path(), &["modify", &b, "--parent", &a, "--no-parent"]);
    assert!(!out.status.success());
}

#[test]
fn modify_cycle_errors() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "A"])).trim().to_string();
    let b = stdout(&goal(dir.path(), &["add", "B", "--parent", &a])).trim().to_string();
    let out = goal(dir.path(), &["modify", &a, "--parent", &b]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("descendant"));
}

#[test]
fn modify_nonexistent_id_errors() {
    let dir = TempDir::new().unwrap();
    let out = goal(dir.path(), &["modify", "a0nonexistent", "--body", "x"]);
    assert!(!out.status.success());
}

#[test]
fn nonexistent_id_errors() {
    let dir = TempDir::new().unwrap();

    let done = goal(dir.path(), &["done", "a0nonexistent"]);
    assert!(!done.status.success());

    let rm = goal(dir.path(), &["delete", "a0nonexistent"]);
    assert!(!rm.status.success());
}
