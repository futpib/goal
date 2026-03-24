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

fn goal_stdin(data_dir: &Path, args: &[&str], input: &str) -> std::process::Output {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new(goal_bin())
        .env("GOAL_DATA_DIR", data_dir)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn goal binary");
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    child.wait_with_output().expect("failed to wait on goal binary")
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

    let rm = goal(dir.path(), &["delete", "--yes", &parent]);
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
fn undo_add() {
    let dir = TempDir::new().unwrap();
    goal(dir.path(), &["add", "learn rust"]);
    let out = goal(dir.path(), &["undo"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(!list.contains("learn rust"));
}

#[test]
fn undo_done() {
    let dir = TempDir::new().unwrap();
    let id = stdout(&goal(dir.path(), &["add", "read a book"])).trim().to_string();
    goal(dir.path(), &["done", &id]);
    let out = goal(dir.path(), &["undo"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(list.contains("[ ]"));
    assert!(!list.contains("[x]"));
}

#[test]
fn undo_undone() {
    let dir = TempDir::new().unwrap();
    let id = stdout(&goal(dir.path(), &["add", "read a book"])).trim().to_string();
    goal(dir.path(), &["done", &id]);
    goal(dir.path(), &["undone", &id]);
    let out = goal(dir.path(), &["undo"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(list.contains("[x]"));
}

#[test]
fn undo_delete_single() {
    let dir = TempDir::new().unwrap();
    let id = stdout(&goal(dir.path(), &["add", "my task"])).trim().to_string();
    goal(dir.path(), &["delete", "--yes", &id]);
    let out = goal(dir.path(), &["undo"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(list.contains("my task"));
}

#[test]
fn undo_delete_subtree() {
    let dir = TempDir::new().unwrap();
    let pid = stdout(&goal(dir.path(), &["add", "parent"])).trim().to_string();
    goal(dir.path(), &["add", "child", "--parent", &pid]);
    goal(dir.path(), &["delete", "--yes", &pid]);
    let out = goal(dir.path(), &["undo"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(list.contains("parent"));
    assert!(list.contains("child"));
}

#[test]
fn undo_modify_body() {
    let dir = TempDir::new().unwrap();
    let id = stdout(&goal(dir.path(), &["add", "old body"])).trim().to_string();
    goal(dir.path(), &["modify", &id, "--body", "new body"]);
    let out = goal(dir.path(), &["undo"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(list.contains("old body"));
    assert!(!list.contains("new body"));
}

#[test]
fn undo_modify_reparent() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "A"])).trim().to_string();
    let b = stdout(&goal(dir.path(), &["add", "B"])).trim().to_string();
    assert_eq!(&b[1..2], "0");
    goal(dir.path(), &["modify", &b, "--parent", &a]);
    let out = goal(dir.path(), &["undo"]);
    assert!(out.status.success(), "{}", stderr(&out));
    // B should be back at depth 0 (original id restored)
    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(list.contains(&b), "old B id should be restored: {}", b);
}

#[test]
fn double_undo() {
    let dir = TempDir::new().unwrap();
    goal(dir.path(), &["add", "X"]);
    goal(dir.path(), &["add", "Y"]);
    goal(dir.path(), &["undo"]);
    let list1 = stdout(&goal(dir.path(), &["list"]));
    assert!(list1.contains("X"));
    assert!(!list1.contains("Y"));
    goal(dir.path(), &["undo"]);
    let list2 = stdout(&goal(dir.path(), &["list"]));
    assert!(!list2.contains("X"));
}

#[test]
fn undo_nothing_errors() {
    let dir = TempDir::new().unwrap();
    let out = goal(dir.path(), &["undo"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("nothing to undo"));
}

#[test]
fn log_shows_history() {
    let dir = TempDir::new().unwrap();
    let id = stdout(&goal(dir.path(), &["add", "learn rust"])).trim().to_string();
    goal(dir.path(), &["done", &id]);
    let log = stdout(&goal(dir.path(), &["log"]));
    assert!(log.contains("Add"));
    assert!(log.contains("Done"));
    assert!(log.contains("learn rust"));
}

#[test]
fn log_after_undo_removes_entry() {
    let dir = TempDir::new().unwrap();
    goal(dir.path(), &["add", "learn rust"]);
    goal(dir.path(), &["undo"]);
    let log = stdout(&goal(dir.path(), &["log"]));
    assert!(!log.contains("Add"), "undone event should not appear in log: {}", log);
}

#[test]
fn info_single_goal() {
    let dir = TempDir::new().unwrap();
    let id = stdout(&goal(dir.path(), &["add", "learn rust"])).trim().to_string();
    let out = goal(dir.path(), &["info", &id]);
    assert!(out.status.success(), "{}", stderr(&out));
    let s = stdout(&out);
    assert!(s.contains(&id));
    assert!(s.contains("learn rust"));
    assert!(s.contains("achievable"));
    assert!(s.contains("not achieved"));
    assert!(s.contains("depth:  0"));
}

#[test]
fn info_with_subtree() {
    let dir = TempDir::new().unwrap();
    let parent = stdout(&goal(dir.path(), &["add", "parent goal"])).trim().to_string();
    let child = stdout(&goal(dir.path(), &["add", "child goal", "--parent", &parent])).trim().to_string();
    let out = goal(dir.path(), &["info", &parent]);
    assert!(out.status.success(), "{}", stderr(&out));
    let s = stdout(&out);
    assert!(s.contains("subtree"));
    assert!(s.contains(&child));
    assert!(s.contains("child goal"));
}

#[test]
fn info_short_prefix() {
    let dir = TempDir::new().unwrap();
    let id = stdout(&goal(dir.path(), &["add", "my goal"])).trim().to_string();
    let out = goal(dir.path(), &["info", &id[..4]]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("my goal"));
}

#[test]
fn nonexistent_id_errors() {
    let dir = TempDir::new().unwrap();

    let done = goal(dir.path(), &["done", "a0nonexistent"]);
    assert!(!done.status.success());

    let rm = goal(dir.path(), &["delete", "a0nonexistent"]);
    assert!(!rm.status.success());
}

#[test]
fn delete_yes_flag_skips_prompt() {
    let dir = TempDir::new().unwrap();
    let id = stdout(&goal(dir.path(), &["add", "to delete"])).trim().to_string();
    let out = goal(dir.path(), &["delete", "--yes", &id]);
    assert!(out.status.success(), "{}", stderr(&out));
    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(!list.contains("to delete"));
}

#[test]
fn delete_prompt_confirmed_deletes() {
    let dir = TempDir::new().unwrap();
    let id = stdout(&goal(dir.path(), &["add", "to delete"])).trim().to_string();
    let out = goal_stdin(dir.path(), &["delete", &id], "y\n");
    assert!(out.status.success(), "{}", stderr(&out));
    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(!list.contains("to delete"));
}

#[test]
fn delete_prompt_declined_keeps_goal() {
    let dir = TempDir::new().unwrap();
    let id = stdout(&goal(dir.path(), &["add", "keep me"])).trim().to_string();
    let out = goal_stdin(dir.path(), &["delete", &id], "n\n");
    assert!(out.status.success(), "{}", stderr(&out));
    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(list.contains("keep me"));
}

#[test]
fn no_args_shows_list() {
    let dir = TempDir::new().unwrap();
    let id = stdout(&goal(dir.path(), &["add", "default list goal"])).trim().to_string();

    let out = goal(dir.path(), &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    let s = stdout(&out);
    assert!(s.contains(&id));
    assert!(s.contains("default list goal"));
}

#[test]
fn annotate_add_shows_in_info() {
    let dir = TempDir::new().unwrap();
    let id = stdout(&goal(dir.path(), &["add", "learn rust"])).trim().to_string();
    let out = goal(dir.path(), &["annotate", &id, "this is a note"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let ann_id = stdout(&out).trim().to_string();
    assert!(ann_id.starts_with('n'), "annotation id should start with 'n', got: {}", ann_id);

    let info = stdout(&goal(dir.path(), &["info", &id]));
    assert!(info.contains("annotations:"));
    assert!(info.contains("this is a note"));
    assert!(info.contains(&ann_id));
}

#[test]
fn annotate_multiple() {
    let dir = TempDir::new().unwrap();
    let id = stdout(&goal(dir.path(), &["add", "goal"])).trim().to_string();
    goal(dir.path(), &["annotate", &id, "first note"]);
    goal(dir.path(), &["annotate", &id, "second note"]);

    let info = stdout(&goal(dir.path(), &["info", &id]));
    assert!(info.contains("first note"));
    assert!(info.contains("second note"));
}

#[test]
fn annotate_edit() {
    let dir = TempDir::new().unwrap();
    let id = stdout(&goal(dir.path(), &["add", "goal"])).trim().to_string();
    let ann_id = stdout(&goal(dir.path(), &["annotate", &id, "original"])).trim().to_string();

    let out = goal(dir.path(), &["annotate", &id, "--edit", &ann_id, "updated"]);
    assert!(out.status.success(), "{}", stderr(&out));

    let info = stdout(&goal(dir.path(), &["info", &id]));
    assert!(info.contains("updated"));
    assert!(!info.contains("original"));
}

#[test]
fn annotate_delete() {
    let dir = TempDir::new().unwrap();
    let id = stdout(&goal(dir.path(), &["add", "goal"])).trim().to_string();
    let ann_id = stdout(&goal(dir.path(), &["annotate", &id, "to be deleted"])).trim().to_string();

    let out = goal(dir.path(), &["annotate", &id, "--delete", &ann_id]);
    assert!(out.status.success(), "{}", stderr(&out));

    let info = stdout(&goal(dir.path(), &["info", &id]));
    assert!(!info.contains("to be deleted"));
    assert!(!info.contains("annotations:"));
}

#[test]
fn annotate_cascade_on_goal_delete() {
    let dir = TempDir::new().unwrap();
    let id = stdout(&goal(dir.path(), &["add", "goal"])).trim().to_string();
    goal(dir.path(), &["annotate", &id, "some note"]);
    goal(dir.path(), &["delete", "--yes", &id]);

    let out = goal(dir.path(), &["info", &id]);
    assert!(!out.status.success());
}

#[test]
fn annotate_short_prefix() {
    let dir = TempDir::new().unwrap();
    let id = stdout(&goal(dir.path(), &["add", "goal"])).trim().to_string();
    let ann_id = stdout(&goal(dir.path(), &["annotate", &id, "note"])).trim().to_string();
    let prefix = &ann_id[..4];

    let out = goal(dir.path(), &["annotate", &id, "--delete", prefix]);
    assert!(out.status.success(), "{}", stderr(&out));
}

#[test]
fn tags_add_shows_in_list() {
    let dir = TempDir::new().unwrap();
    let id = stdout(&goal(dir.path(), &["add", "learn rust", "+work", "+rust"])).trim().to_string();
    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(list.contains("+work"), "list should show +work: {}", list);
    assert!(list.contains("+rust"), "list should show +rust: {}", list);
    let _ = id;
}

#[test]
fn tags_add_shows_in_info() {
    let dir = TempDir::new().unwrap();
    let id = stdout(&goal(dir.path(), &["add", "learn rust", "+work"])).trim().to_string();
    let info = stdout(&goal(dir.path(), &["info", &id]));
    assert!(info.contains("tags:"), "info should have tags section: {}", info);
    assert!(info.contains("+work"), "info should show +work: {}", info);
}

#[test]
fn tags_modify_add_and_remove() {
    let dir = TempDir::new().unwrap();
    let id = stdout(&goal(dir.path(), &["add", "goal", "+work", "+rust"])).trim().to_string();

    let out = goal(dir.path(), &["modify", &id, "-rust", "+personal"]);
    assert!(out.status.success(), "{}", stderr(&out));

    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(list.contains("+work"), "should still have +work: {}", list);
    assert!(list.contains("+personal"), "should now have +personal: {}", list);
    assert!(!list.contains("+rust"), "should not have +rust anymore: {}", list);
}

#[test]
fn tags_only_modify_does_not_require_other_flags() {
    let dir = TempDir::new().unwrap();
    let id = stdout(&goal(dir.path(), &["add", "goal"])).trim().to_string();

    let out = goal(dir.path(), &["modify", &id, "+newtag"]);
    assert!(out.status.success(), "{}", stderr(&out));

    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(list.contains("+newtag"), "should have +newtag: {}", list);
}

#[test]
fn tags_cascade_on_goal_delete() {
    let dir = TempDir::new().unwrap();
    let id = stdout(&goal(dir.path(), &["add", "goal", "+work"])).trim().to_string();
    goal(dir.path(), &["delete", "--yes", &id]);

    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(!list.contains("+work"), "deleted goal's tags should not appear: {}", list);
}

#[test]
fn old_id_resolves_with_warning_after_reparent() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "A"])).trim().to_string();
    let b = stdout(&goal(dir.path(), &["add", "B"])).trim().to_string();

    // Reparent B under A — B gets a new ID
    let out = goal(dir.path(), &["modify", &b, "--parent", &a]);
    assert!(out.status.success(), "{}", stderr(&out));
    let new_b = stdout(&out).trim().to_string();
    assert_ne!(b, new_b, "id should change on reparent");

    // Reference B by its old id — should succeed with a warning on stderr
    let info = goal(dir.path(), &["info", &b]);
    assert!(info.status.success(), "old id should still resolve: {}", stderr(&info));
    assert!(stdout(&info).contains("B"), "should show B's body");
    assert!(stderr(&info).contains("warning"), "should print a warning about the renamed id");
    assert!(stderr(&info).contains(&b), "warning should mention old id");
    assert!(stderr(&info).contains(&new_b), "warning should mention new id");
}

#[test]
fn old_id_prefix_resolves_with_warning_after_reparent() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "A"])).trim().to_string();
    let b = stdout(&goal(dir.path(), &["add", "B"])).trim().to_string();
    let prefix = &b[..6];

    goal(dir.path(), &["modify", &b, "--parent", &a]);

    let info = goal(dir.path(), &["info", prefix]);
    assert!(info.status.success(), "old id prefix should resolve: {}", stderr(&info));
    assert!(stderr(&info).contains("warning"));
}

#[test]
fn old_id_after_double_reparent_resolves_to_final() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "A"])).trim().to_string();
    let b = stdout(&goal(dir.path(), &["add", "B"])).trim().to_string();
    let c = stdout(&goal(dir.path(), &["add", "C"])).trim().to_string();

    // B -> under A
    let out1 = goal(dir.path(), &["modify", &b, "--parent", &a]);
    let b1 = stdout(&out1).trim().to_string();
    // B -> under C (detach first then reparent, or just reparent under C)
    let out2 = goal(dir.path(), &["modify", &b1, "--parent", &c]);
    let b2 = stdout(&out2).trim().to_string();

    // Original B id should resolve directly to b2 (chained alias collapsed)
    let info = goal(dir.path(), &["info", &b]);
    assert!(info.status.success(), "original id should resolve after double reparent: {}", stderr(&info));
    assert!(stdout(&info).contains("B"));
    assert!(stderr(&info).contains("warning"), "should print a warning");
    assert!(stderr(&info).contains(&b2), "warning should point to final id");
}

#[test]
fn tags_no_tags_no_suffix() {
    let dir = TempDir::new().unwrap();
    goal(dir.path(), &["add", "plain goal"]);
    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(!list.contains('+'), "goal with no tags should not show '+': {}", list);
}
