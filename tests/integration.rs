use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn goal_bin() -> std::path::PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("goal")
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

    // done goals are excluded from list; use info to check status
    let info = goal(dir.path(), &["info", &id]);
    assert!(stdout(&info).contains("achieved"), "{}", stdout(&info));

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
    // undoing `undone` reverts to achieved; done goals are hidden from list, use info
    let info = stdout(&goal(dir.path(), &["info", &id]));
    assert!(info.contains("achieved"), "{}", info);
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

#[test]
fn prioritize_basic() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "goal A"])).trim().to_string();
    let b = stdout(&goal(dir.path(), &["add", "goal B"])).trim().to_string();

    // Without priority: B is newer, B appears first (higher rowid = higher priority)
    let list_before = stdout(&goal(dir.path(), &["list"]));
    let a_pos = list_before.find("goal A").unwrap();
    let b_pos = list_before.find("goal B").unwrap();
    assert!(b_pos < a_pos, "newer B should appear before older A before prioritize");

    // Prioritize A over B
    let out = goal(dir.path(), &["rank",&a, &b]);
    assert!(out.status.success(), "{}", stderr(&out));

    let list_after = stdout(&goal(dir.path(), &["list"]));
    let a_pos2 = list_after.find("goal A").unwrap();
    let b_pos2 = list_after.find("goal B").unwrap();
    assert!(a_pos2 < b_pos2, "A should appear before B after prioritize");
}

#[test]
fn prioritize_cycle_direct() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "A"])).trim().to_string();
    let b = stdout(&goal(dir.path(), &["add", "B"])).trim().to_string();

    goal(dir.path(), &["rank",&a, &b]);
    let out = goal(dir.path(), &["rank",&b, &a]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("cycle"), "{}", stderr(&out));
}

#[test]
fn prioritize_cycle_transitive() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "A"])).trim().to_string();
    let b = stdout(&goal(dir.path(), &["add", "B"])).trim().to_string();
    let c = stdout(&goal(dir.path(), &["add", "C"])).trim().to_string();

    goal(dir.path(), &["rank",&a, &b]);
    goal(dir.path(), &["rank",&b, &c]);
    let out = goal(dir.path(), &["rank",&c, &a]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("cycle"), "{}", stderr(&out));
}

#[test]
fn prioritize_self_loop() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "A"])).trim().to_string();
    let out = goal(dir.path(), &["rank",&a, &a]);
    assert!(!out.status.success());
}

#[test]
fn prioritize_duplicate() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "A"])).trim().to_string();
    let b = stdout(&goal(dir.path(), &["add", "B"])).trim().to_string();

    let out1 = goal(dir.path(), &["rank",&a, &b]);
    assert!(out1.status.success(), "{}", stderr(&out1));
    let out2 = goal(dir.path(), &["rank",&a, &b]);
    assert!(!out2.status.success(), "duplicate edge should fail");
}

#[test]
fn deprioritize_reverts_ordering() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "A"])).trim().to_string();
    let b = stdout(&goal(dir.path(), &["add", "B"])).trim().to_string();

    goal(dir.path(), &["rank",&a, &b]);
    let out = goal(dir.path(), &["unrank",&a, &b]);
    assert!(out.status.success(), "{}", stderr(&out));

    // After removing edge, B (newer) should rank before A again
    let list = stdout(&goal(dir.path(), &["list"]));
    let a_pos = list.find('A').unwrap();
    let b_pos = list.find('B').unwrap();
    assert!(b_pos < a_pos, "B should rank before A after deprioritize: {}", list);
}

#[test]
fn deprioritize_nonexistent_fails() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "A"])).trim().to_string();
    let b = stdout(&goal(dir.path(), &["add", "B"])).trim().to_string();
    let out = goal(dir.path(), &["unrank",&a, &b]);
    assert!(!out.status.success());
}

#[test]
fn undo_prioritize() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "A"])).trim().to_string();
    let b = stdout(&goal(dir.path(), &["add", "B"])).trim().to_string();

    goal(dir.path(), &["rank",&a, &b]);
    let out = goal(dir.path(), &["undo"]);
    assert!(out.status.success(), "{}", stderr(&out));

    // After undo, B (newer) should rank before A again
    let list = stdout(&goal(dir.path(), &["list"]));
    let a_pos = list.find('A').unwrap();
    let b_pos = list.find('B').unwrap();
    assert!(b_pos < a_pos, "B should rank before A after undo prioritize: {}", list);
}

#[test]
fn undo_deprioritize() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "A"])).trim().to_string();
    let b = stdout(&goal(dir.path(), &["add", "B"])).trim().to_string();

    goal(dir.path(), &["rank",&a, &b]);
    goal(dir.path(), &["unrank",&a, &b]);
    let out = goal(dir.path(), &["undo"]);
    assert!(out.status.success(), "{}", stderr(&out));

    // After undoing deprioritize, A should be ranked before B again
    let list = stdout(&goal(dir.path(), &["list"]));
    let a_pos = list.find('A').unwrap();
    let b_pos = list.find('B').unwrap();
    assert!(a_pos < b_pos, "A should rank before B after undo deprioritize: {}", list);
}

#[test]
fn undo_delete_restores_edges() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "goal A"])).trim().to_string();
    let b = stdout(&goal(dir.path(), &["add", "goal B"])).trim().to_string();

    goal(dir.path(), &["rank",&a, &b]);
    goal(dir.path(), &["delete", "--yes", &a]);
    let out = goal(dir.path(), &["undo"]);
    assert!(out.status.success(), "{}", stderr(&out));

    // After undo: A is restored and edge A>B should be restored too
    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(list.contains("goal A"), "A should be restored");
    assert!(list.contains("goal B"), "B should still be there");
    let a_pos = list.find("goal A").unwrap();
    let b_pos = list.find("goal B").unwrap();
    assert!(a_pos < b_pos, "A should rank before B after edge restored: {}", list);
}

#[test]
fn delete_middle_collapses_transitively() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "goal A"])).trim().to_string();
    let b = stdout(&goal(dir.path(), &["add", "goal B"])).trim().to_string();
    let c = stdout(&goal(dir.path(), &["add", "goal C"])).trim().to_string();

    goal(dir.path(), &["rank",&a, &b]);
    goal(dir.path(), &["rank",&b, &c]);

    // Delete B; A>C should be synthesized
    let out = goal(dir.path(), &["delete", "--yes", &b]);
    assert!(out.status.success(), "{}", stderr(&out));

    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(!list.contains("goal B"), "B should be deleted");
    let a_pos = list.find("goal A").unwrap();
    let c_pos = list.find("goal C").unwrap();
    assert!(a_pos < c_pos, "A should still rank before C after transitive collapse: {}", list);
}

#[test]
fn undo_delete_middle_restores_original_edges() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "goal A"])).trim().to_string();
    let b = stdout(&goal(dir.path(), &["add", "goal B"])).trim().to_string();
    let c = stdout(&goal(dir.path(), &["add", "goal C"])).trim().to_string();

    goal(dir.path(), &["rank",&a, &b]);
    goal(dir.path(), &["rank",&b, &c]);
    goal(dir.path(), &["delete", "--yes", &b]);

    let out = goal(dir.path(), &["undo"]);
    assert!(out.status.success(), "{}", stderr(&out));

    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(list.contains("goal B"), "B should be restored");
    // A>B>C chain should be restored (A first, then B, then C)
    let a_pos = list.find("goal A").unwrap();
    let b_pos = list.find("goal B").unwrap();
    let c_pos = list.find("goal C").unwrap();
    assert!(a_pos < b_pos, "A should rank before B");
    assert!(b_pos < c_pos, "B should rank before C");
}

#[test]
fn tiebreak_newer_higher_priority() {
    let dir = TempDir::new().unwrap();
    // A added first, B added second; no priority edges
    goal(dir.path(), &["add", "goal A"]);
    goal(dir.path(), &["add", "goal B"]);

    let list = stdout(&goal(dir.path(), &["list"]));
    let a_pos = list.find("goal A").unwrap();
    let b_pos = list.find("goal B").unwrap();
    assert!(b_pos < a_pos, "newer B should rank before older A by default: {}", list);
}

#[test]
fn linked_beats_unlinked() {
    let dir = TempDir::new().unwrap();
    // Add A first (older), then B (newer), then C (newest)
    let a = stdout(&goal(dir.path(), &["add", "goal A"])).trim().to_string();
    let b = stdout(&goal(dir.path(), &["add", "goal B"])).trim().to_string();
    let c = stdout(&goal(dir.path(), &["add", "goal C"])).trim().to_string();

    // Link A>B; C is unlinked
    goal(dir.path(), &["rank",&a, &b]);

    let list = stdout(&goal(dir.path(), &["list"]));
    let a_pos = list.find("goal A").unwrap();
    let c_pos = list.find("goal C").unwrap();
    // A is linked, C is unlinked; A should rank before C regardless of age
    assert!(a_pos < c_pos, "linked A should rank before unlinked C: {}", list);
    let _ = (b, c);
}

#[test]
fn done_goals_excluded_from_list() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "goal A"])).trim().to_string();
    goal(dir.path(), &["add", "goal B"]);

    goal(dir.path(), &["done", &a]);

    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(!list.contains("goal A"), "done goal A should be excluded from list: {}", list);
    assert!(list.contains("goal B"), "pending goal B should appear: {}", list);
}

#[test]
fn long_chain_ordering() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "goal A"])).trim().to_string();
    let b = stdout(&goal(dir.path(), &["add", "goal B"])).trim().to_string();
    let c = stdout(&goal(dir.path(), &["add", "goal C"])).trim().to_string();
    let d = stdout(&goal(dir.path(), &["add", "goal D"])).trim().to_string();
    let e = stdout(&goal(dir.path(), &["add", "goal E"])).trim().to_string();

    goal(dir.path(), &["rank",&a, &b]);
    goal(dir.path(), &["rank",&b, &c]);
    goal(dir.path(), &["rank",&c, &d]);
    goal(dir.path(), &["rank",&d, &e]);

    let list = stdout(&goal(dir.path(), &["list"]));
    let a_pos = list.find("goal A").unwrap();
    let b_pos = list.find("goal B").unwrap();
    let c_pos = list.find("goal C").unwrap();
    let d_pos = list.find("goal D").unwrap();
    let e_pos = list.find("goal E").unwrap();
    assert!(a_pos < b_pos, "A before B");
    assert!(b_pos < c_pos, "B before C");
    assert!(c_pos < d_pos, "C before D");
    assert!(d_pos < e_pos, "D before E");
}

#[test]
fn diamond_ordering() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "goal A"])).trim().to_string();
    let b = stdout(&goal(dir.path(), &["add", "goal B"])).trim().to_string();
    let c = stdout(&goal(dir.path(), &["add", "goal C"])).trim().to_string();
    let d = stdout(&goal(dir.path(), &["add", "goal D"])).trim().to_string();

    // Diamond: A > B, A > C, B > D, C > D
    goal(dir.path(), &["rank",&a, &b]);
    goal(dir.path(), &["rank",&a, &c]);
    goal(dir.path(), &["rank",&b, &d]);
    goal(dir.path(), &["rank",&c, &d]);

    let list = stdout(&goal(dir.path(), &["list"]));
    let a_pos = list.find("goal A").unwrap();
    let d_pos = list.find("goal D").unwrap();
    assert!(a_pos < d_pos, "A should come before D in diamond");
}

#[test]
fn reparent_migrates_edges() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "goal A"])).trim().to_string();
    let b = stdout(&goal(dir.path(), &["add", "goal B"])).trim().to_string();

    goal(dir.path(), &["rank",&a, &b]);

    // Reparent A (A gets a new ID)
    let out = goal(dir.path(), &["modify", &a, "--continuous"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let new_a = stdout(&out).trim().to_string();
    assert_ne!(a, new_a, "id should change after modify kind");

    // Edge should have migrated: adding the reverse edge (new_a < b) should fail with cycle error
    let cycle_out = goal(dir.path(), &["rank",&b, &new_a]);
    assert!(!cycle_out.status.success(), "reverse edge should be rejected as a cycle (edge was migrated)");
    assert!(stderr(&cycle_out).contains("cycle"), "{}", stderr(&cycle_out));
}

#[test]
fn prioritize_then_done() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "goal A"])).trim().to_string();
    let b = stdout(&goal(dir.path(), &["add", "goal B"])).trim().to_string();

    goal(dir.path(), &["rank",&a, &b]);
    goal(dir.path(), &["done", &a]);

    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(!list.contains("goal A"), "done A should be excluded");
    assert!(list.contains("goal B"), "B should still appear");
}

#[test]
fn list_tag_filter_shows_only_matching() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "goal A", "+work"])).trim().to_string();
    let _b = stdout(&goal(dir.path(), &["add", "goal B", "+personal"])).trim().to_string();
    assert!(!a.is_empty());

    let list = stdout(&goal(dir.path(), &["list", "+work"]));
    assert!(list.contains("goal A"), "tagged goal should appear");
    assert!(!list.contains("goal B"), "untagged goal should not appear");
}

#[test]
fn list_tag_filter_includes_ancestors() {
    let dir = TempDir::new().unwrap();
    let parent = stdout(&goal(dir.path(), &["add", "parent goal"])).trim().to_string();
    let child = stdout(&goal(dir.path(), &["add", "child goal", &format!("--parent={}", parent), "+work"])).trim().to_string();
    assert!(!child.is_empty());

    let list = stdout(&goal(dir.path(), &["list", "+work"]));
    assert!(list.contains("child goal"), "tagged child should appear");
    assert!(list.contains("parent goal"), "ancestor of tagged child should appear");
}

#[test]
fn rank_interactive_records_edge() {
    let dir = TempDir::new().unwrap();
    let _a = stdout(&goal(dir.path(), &["add", "goal A"])).trim().to_string();
    let _b = stdout(&goal(dir.path(), &["add", "goal B"])).trim().to_string();

    // Choose "1" — whichever goal is presented first gets higher priority
    let out = goal_stdin(dir.path(), &["rank-interactive"], "1\n");
    assert!(out.status.success(), "{}", stderr(&out));

    // After ranking, rank-interactive should report no more pairs
    let out2 = goal_stdin(dir.path(), &["rank-interactive"], "");
    assert!(out2.status.success(), "{}", stderr(&out2));
    let s = stdout(&out2);
    assert!(s.contains("already ordered"), "should report all ordered after ranking, got: {}", s);
}

#[test]
fn rank_interactive_skip() {
    let dir = TempDir::new().unwrap();
    let _a = stdout(&goal(dir.path(), &["add", "goal A"])).trim().to_string();
    let _b = stdout(&goal(dir.path(), &["add", "goal B"])).trim().to_string();

    let out = goal_stdin(dir.path(), &["rank-interactive"], "s\n");
    assert!(out.status.success(), "{}", stderr(&out));

    // No edge added; both should still appear
    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(list.contains("goal A"));
    assert!(list.contains("goal B"));
}

#[test]
fn rank_interactive_quit() {
    let dir = TempDir::new().unwrap();
    let _a = stdout(&goal(dir.path(), &["add", "goal A"])).trim().to_string();
    let _b = stdout(&goal(dir.path(), &["add", "goal B"])).trim().to_string();
    let _c = stdout(&goal(dir.path(), &["add", "goal C"])).trim().to_string();

    // q on first pair — no edges added
    let out = goal_stdin(dir.path(), &["rank-interactive"], "q\n");
    assert!(out.status.success(), "{}", stderr(&out));

    let list = stdout(&goal(dir.path(), &["list"]));
    assert!(list.contains("goal A"));
    assert!(list.contains("goal B"));
    assert!(list.contains("goal C"));
}

#[test]
fn rank_interactive_skips_transitively_ranked() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "goal A"])).trim().to_string();
    let b = stdout(&goal(dir.path(), &["add", "goal B"])).trim().to_string();
    let c = stdout(&goal(dir.path(), &["add", "goal C"])).trim().to_string();
    // A > B > C; A > C is implied
    goal(dir.path(), &["rank", &a, &b]);
    goal(dir.path(), &["rank", &b, &c]);

    // Only pair not yet ordered is none — all are transitively covered
    let out = goal_stdin(dir.path(), &["rank-interactive"], "");
    assert!(out.status.success(), "{}", stderr(&out));
    let s = stdout(&out);
    assert!(s.contains("already ordered"), "A>B>C covers all pairs, got: {}", s);
}

#[test]
fn rank_interactive_no_pairs_when_all_ranked() {
    let dir = TempDir::new().unwrap();
    let a = stdout(&goal(dir.path(), &["add", "goal A"])).trim().to_string();
    let b = stdout(&goal(dir.path(), &["add", "goal B"])).trim().to_string();
    goal(dir.path(), &["rank", &a, &b]);

    let out = goal_stdin(dir.path(), &["rank-interactive"], "");
    assert!(out.status.success(), "{}", stderr(&out));
    let s = stdout(&out);
    assert!(s.contains("already ordered"), "should say all ordered, got: {}", s);
}
