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
fn nonexistent_id_errors() {
    let dir = TempDir::new().unwrap();

    let done = goal(dir.path(), &["done", "a0nonexistent"]);
    assert!(!done.status.success());

    let rm = goal(dir.path(), &["delete", "a0nonexistent"]);
    assert!(!rm.status.success());
}
