use anyhow::{bail, Context, Result};
use directories::ProjectDirs;
use rand::RngExt;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GoalKind {
    Achievable,
    Continuous,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub id: String,
    pub parent_id: Option<String>,
    pub body: String,
    pub achieved: bool,
    pub kind: GoalKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub id: String,
    pub goal_id: String,
    pub body: String,
    pub created_at: String,
}

pub struct Event {
    pub event_id: String,
    pub timestamp: String,
    pub op: String,
    pub goal_id: String,
    pub goal_body: String,
}

pub enum ModifyParent {
    Keep,
    Detach,
    Reparent(String),
}

/// Encode depth as variable-length base-15 with 'f' as continuation.
fn encode_depth(mut depth: u32) -> String {
    let mut out = String::new();
    loop {
        if depth < 15 {
            out.push(char::from_digit(depth, 16).unwrap());
            break;
        }
        out.push('f');
        depth -= 15;
    }
    out
}

pub fn parse_depth(id: &str) -> u32 {
    let chars: Vec<char> = id.chars().skip(1).collect();
    let mut depth = 0u32;
    for ch in chars {
        if ch == 'f' {
            depth += 15;
        } else {
            depth += ch.to_digit(16).unwrap_or(0);
            break;
        }
    }
    depth
}

pub fn generate_id(kind: &GoalKind, depth: u32) -> String {
    let type_char = match kind {
        GoalKind::Achievable => 'a',
        GoalKind::Continuous => 'c',
    };
    let depth_str = encode_depth(depth);
    let prefix = format!("{}{}", type_char, depth_str);
    let random_len = 16usize.saturating_sub(prefix.len());
    let mut bytes = vec![0u8; (random_len + 1) / 2];
    rand::rng().fill(&mut bytes[..]);
    let random_hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
    format!("{}{}", prefix, &random_hex[..random_len])
}

pub fn generate_event_id() -> String {
    let mut bytes = [0u8; 8];
    rand::rng().fill(&mut bytes);
    let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
    format!("e{}", &hex[..15])
}

fn kind_from_id(id: &str) -> GoalKind {
    match id.chars().next() {
        Some('a') => GoalKind::Achievable,
        _ => GoalKind::Continuous,
    }
}

fn row_to_goal(row: &rusqlite::Row) -> rusqlite::Result<Goal> {
    let id: String = row.get(0)?;
    let kind = kind_from_id(&id);
    Ok(Goal {
        id,
        parent_id: row.get(1)?,
        body: row.get(2)?,
        achieved: row.get::<_, i32>(3)? != 0,
        kind,
    })
}

pub fn open_db() -> Result<Connection> {
    let data_dir = if let Ok(dir) = env::var("GOAL_DATA_DIR") {
        std::path::PathBuf::from(dir)
    } else {
        let proj = ProjectDirs::from("", "", "goal")
            .context("could not determine data directory")?;
        proj.data_dir().to_path_buf()
    };
    fs::create_dir_all(&data_dir)
        .with_context(|| format!("could not create data directory {}", data_dir.display()))?;
    let db_path = data_dir.join("goals.db");
    let conn = Connection::open(&db_path)
        .with_context(|| format!("could not open database {}", db_path.display()))?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS goals (
            id        TEXT PRIMARY KEY,
            parent_id TEXT REFERENCES goals(id) ON DELETE CASCADE,
            body      TEXT NOT NULL,
            achieved  INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS events (
            event_id   TEXT PRIMARY KEY,
            timestamp  TEXT NOT NULL,
            op         TEXT NOT NULL,
            snapshot   TEXT NOT NULL,
            new_id     TEXT
        );
        CREATE TABLE IF NOT EXISTS events_undone (
            event_id   TEXT PRIMARY KEY,
            timestamp  TEXT NOT NULL,
            op         TEXT NOT NULL,
            snapshot   TEXT NOT NULL,
            new_id     TEXT
        );
        CREATE TABLE IF NOT EXISTS annotations (
            id         TEXT PRIMARY KEY,
            goal_id    TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
            body       TEXT NOT NULL,
            created_at TEXT NOT NULL
        );",
    )?;
    Ok(conn)
}

pub fn get_goal(conn: &Connection, id: &str) -> Result<Goal> {
    let mut stmt = conn.prepare(
        "SELECT id, parent_id, body, achieved FROM goals WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map([id], row_to_goal)?;
    rows.next()
        .ok_or_else(|| anyhow::anyhow!("no goal with id '{}'", id))?
        .map_err(Into::into)
}

pub fn resolve_id(conn: &Connection, prefix: &str) -> Result<String> {
    let pattern = format!("{}%", prefix);
    let mut stmt = conn.prepare("SELECT id FROM goals WHERE id LIKE ?1")?;
    let ids: Vec<String> = stmt
        .query_map([&pattern], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    match ids.len() {
        0 => bail!("no goal matching '{}'", prefix),
        1 => Ok(ids.into_iter().next().unwrap()),
        _ => bail!("ambiguous prefix '{}' matches {} goals", prefix, ids.len()),
    }
}

fn children_of(conn: &Connection, parent_id: &str) -> Result<Vec<Goal>> {
    let mut stmt = conn.prepare(
        "SELECT id, parent_id, body, achieved FROM goals WHERE parent_id = ?1 ORDER BY id",
    )?;
    let goals = stmt
        .query_map([parent_id], row_to_goal)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(goals)
}

pub fn collect_subtree(conn: &Connection, root_id: &str) -> Result<Vec<Goal>> {
    let root = {
        let mut stmt = conn.prepare(
            "SELECT id, parent_id, body, achieved FROM goals WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map([root_id], row_to_goal)?;
        rows.next()
            .ok_or_else(|| anyhow::anyhow!("no goal with id '{}'", root_id))??
    };
    let mut result = vec![root];
    let mut i = 0;
    while i < result.len() {
        let id = result[i].id.clone();
        let kids = children_of(conn, &id)?;
        result.extend(kids);
        i += 1;
    }
    Ok(result)
}

fn record_event(
    conn: &Connection,
    op: &str,
    snapshot: &[Goal],
    new_id: Option<&str>,
) -> Result<String> {
    let event_id = generate_event_id();
    let snapshot_json = serde_json::to_string(snapshot)?;
    conn.execute(
        "INSERT INTO events (event_id, timestamp, op, snapshot, new_id) \
         VALUES (?1, datetime('now'), ?2, ?3, ?4)",
        rusqlite::params![event_id, op, snapshot_json, new_id],
    )?;
    Ok(event_id)
}

pub fn add_goal(conn: &Connection, body: &str, parent_id: Option<&str>, kind: &GoalKind) -> Result<String> {
    let depth = if let Some(pid) = parent_id {
        parse_depth(pid) + 1
    } else {
        0
    };
    let id = generate_id(kind, depth);
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO goals (id, parent_id, body) VALUES (?1, ?2, ?3)",
        rusqlite::params![id, parent_id, body],
    )?;
    let snapshot_goal = Goal {
        id: id.clone(),
        parent_id: parent_id.map(String::from),
        body: body.to_string(),
        achieved: false,
        kind: kind.clone(),
    };
    let snapshot_json = serde_json::to_string(&[&snapshot_goal])?;
    let event_id = generate_event_id();
    tx.execute(
        "INSERT INTO events (event_id, timestamp, op, snapshot, new_id) \
         VALUES (?1, datetime('now'), 'Add', ?2, ?3)",
        rusqlite::params![event_id, snapshot_json, id],
    )?;
    tx.commit()?;
    Ok(id)
}

pub fn set_achieved(conn: &Connection, id: &str, achieved: bool) -> Result<()> {
    let kind = kind_from_id(id);
    if kind == GoalKind::Continuous {
        bail!("goal '{}' is continuous and cannot be marked done", id);
    }
    let tx = conn.unchecked_transaction()?;
    let goal = {
        let mut stmt = tx.prepare(
            "SELECT id, parent_id, body, achieved FROM goals WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map([id], row_to_goal)?;
        rows.next()
            .ok_or_else(|| anyhow::anyhow!("no goal with id '{}'", id))??
    };
    let changed = tx.execute(
        "UPDATE goals SET achieved = ?1 WHERE id = ?2",
        rusqlite::params![achieved as i32, id],
    )?;
    if changed == 0 {
        bail!("no goal with id '{}'", id);
    }
    let op = if achieved { "Done" } else { "Undone" };
    record_event(&tx, op, &[goal], None)?;
    tx.commit()?;
    Ok(())
}

pub fn remove_goal(conn: &Connection, id: &str) -> Result<()> {
    let subtree = collect_subtree(conn, id)?;
    let tx = conn.unchecked_transaction()?;
    let changed = tx.execute("DELETE FROM goals WHERE id = ?1", [id])?;
    if changed == 0 {
        bail!("no goal with id '{}'", id);
    }
    record_event(&tx, "Delete", &subtree, None)?;
    tx.commit()?;
    Ok(())
}

pub fn modify_goal(
    conn: &Connection,
    id: &str,
    new_body: Option<&str>,
    new_parent: ModifyParent,
    new_kind_opt: Option<GoalKind>,
) -> Result<String> {
    let subtree = collect_subtree(conn, id)?;
    let root = &subtree[0];

    let new_kind = new_kind_opt.unwrap_or_else(|| root.kind.clone());
    let new_parent_id: Option<String> = match &new_parent {
        ModifyParent::Keep => root.parent_id.clone(),
        ModifyParent::Detach => None,
        ModifyParent::Reparent(pid) => {
            if subtree.iter().any(|g| &g.id == pid) {
                bail!("cannot reparent a goal under one of its own descendants");
            }
            Some(pid.clone())
        }
    };

    let old_depth = parse_depth(id) as i32;
    let new_depth = match &new_parent_id {
        Some(pid) => parse_depth(pid) as i32 + 1,
        None => 0,
    };
    let depth_delta = new_depth - old_depth;
    let id_changes = new_kind != root.kind || depth_delta != 0;
    let new_body_str = new_body.unwrap_or(&root.body);

    if !id_changes {
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE goals SET body = ?1 WHERE id = ?2",
            rusqlite::params![new_body_str, id],
        )?;
        // snapshot is pre-state (root before body change)
        record_event(&tx, "Modify", &subtree, Some(id))?;
        tx.commit()?;
        return Ok(id.to_string());
    }

    let mut id_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for node in &subtree {
        let node_kind = if node.id == id { new_kind.clone() } else { node.kind.clone() };
        let node_old_depth = parse_depth(&node.id) as i32;
        let node_new_depth = (node_old_depth + depth_delta) as u32;
        id_map.insert(node.id.clone(), generate_id(&node_kind, node_new_depth));
    }

    let new_root_id = id_map[id].clone();

    conn.execute_batch("PRAGMA foreign_keys = OFF")?;
    let tx_result = (|| -> Result<()> {
        let tx = conn.unchecked_transaction()?;
        for node in &subtree {
            tx.execute("DELETE FROM goals WHERE id = ?1", [&node.id])?;
        }
        for node in &subtree {
            let new_id = &id_map[&node.id];
            let new_node_parent_id: Option<String> = if node.id == id {
                new_parent_id.clone()
            } else {
                node.parent_id.as_ref().map(|pid| id_map[pid].clone())
            };
            let node_body = if node.id == id { new_body_str } else { &node.body };
            tx.execute(
                "INSERT INTO goals (id, parent_id, body, achieved) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![new_id, new_node_parent_id, node_body, node.achieved as i32],
            )?;
        }
        record_event(&tx, "Modify", &subtree, Some(&new_root_id))?;
        tx.commit()?;
        Ok(())
    })();
    conn.execute_batch("PRAGMA foreign_keys = ON")?;
    tx_result?;

    Ok(new_root_id)
}

pub fn undo_last(conn: &Connection) -> Result<()> {
    let row = {
        let mut stmt = conn.prepare(
            "SELECT event_id, op, snapshot, new_id FROM events \
             ORDER BY timestamp DESC, rowid DESC LIMIT 1",
        )?;
        let mut rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;
        match rows.next() {
            None => bail!("nothing to undo"),
            Some(r) => r?,
        }
    };
    let (event_id, op, snapshot_json, new_id) = row;
    let snapshot: Vec<Goal> = serde_json::from_str(&snapshot_json)?;

    conn.execute_batch("PRAGMA foreign_keys = OFF")?;
    let tx_result = (|| -> Result<()> {
        let tx = conn.unchecked_transaction()?;
        match op.as_str() {
            "Add" => {
                let target = new_id.as_deref()
                    .ok_or_else(|| anyhow::anyhow!("malformed Add event: missing new_id"))?;
                tx.execute("DELETE FROM goals WHERE id = ?1", [target])?;
            }
            "Done" | "Undone" => {
                let goal = snapshot.first()
                    .ok_or_else(|| anyhow::anyhow!("malformed event: empty snapshot"))?;
                tx.execute(
                    "UPDATE goals SET achieved = ?1 WHERE id = ?2",
                    rusqlite::params![goal.achieved as i32, goal.id],
                )?;
            }
            "Delete" => {
                for g in &snapshot {
                    tx.execute(
                        "INSERT INTO goals (id, parent_id, body, achieved) VALUES (?1, ?2, ?3, ?4)",
                        rusqlite::params![g.id, g.parent_id, g.body, g.achieved as i32],
                    )?;
                }
            }
            "Modify" => {
                let target = new_id.as_deref()
                    .ok_or_else(|| anyhow::anyhow!("malformed Modify event: missing new_id"))?;
                tx.execute("DELETE FROM goals WHERE id = ?1", [target])?;
                for g in &snapshot {
                    tx.execute(
                        "INSERT INTO goals (id, parent_id, body, achieved) VALUES (?1, ?2, ?3, ?4)",
                        rusqlite::params![g.id, g.parent_id, g.body, g.achieved as i32],
                    )?;
                }
            }
            _ => bail!("unknown op '{}'", op),
        }
        // Archive the event before deleting it
        tx.execute(
            "INSERT INTO events_undone SELECT * FROM events WHERE event_id = ?1",
            [&event_id],
        )?;
        tx.execute("DELETE FROM events WHERE event_id = ?1", [&event_id])?;
        tx.commit()?;
        Ok(())
    })();
    conn.execute_batch("PRAGMA foreign_keys = ON")?;
    tx_result?;
    Ok(())
}

pub fn list_events(conn: &Connection) -> Result<Vec<Event>> {
    let mut stmt = conn.prepare(
        "SELECT event_id, timestamp, op, snapshot, new_id \
         FROM events ORDER BY timestamp ASC, rowid ASC",
    )?;
    let events = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut result = Vec::new();
    for (event_id, timestamp, op, snapshot_json, new_id) in events {
        let snapshot: Vec<Goal> = serde_json::from_str(&snapshot_json)
            .unwrap_or_default();
        let first = snapshot.first();
        let goal_id = first
            .map(|g| g.id.clone())
            .or(new_id)
            .unwrap_or_default();
        let goal_body = first
            .map(|g| g.body.clone())
            .unwrap_or_default();
        result.push(Event { event_id, timestamp, op, goal_id, goal_body });
    }
    Ok(result)
}

pub fn all_goals(conn: &Connection) -> Result<Vec<Goal>> {
    let mut stmt = conn.prepare(
        "SELECT id, parent_id, body, achieved FROM goals ORDER BY id",
    )?;
    let goals = stmt
        .query_map([], row_to_goal)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(goals)
}

fn generate_annotation_id() -> String {
    let mut bytes = [0u8; 8];
    rand::rng().fill(&mut bytes);
    let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
    format!("n{}", &hex[..15])
}

pub fn add_annotation(conn: &Connection, goal_id: &str, body: &str) -> Result<String> {
    let id = generate_annotation_id();
    conn.execute(
        "INSERT INTO annotations (id, goal_id, body, created_at) VALUES (?1, ?2, ?3, datetime('now'))",
        rusqlite::params![id, goal_id, body],
    )?;
    Ok(id)
}

pub fn edit_annotation(conn: &Connection, ann_id: &str, body: &str) -> Result<()> {
    let changed = conn.execute(
        "UPDATE annotations SET body = ?1 WHERE id = ?2",
        rusqlite::params![body, ann_id],
    )?;
    if changed == 0 {
        bail!("no annotation with id '{}'", ann_id);
    }
    Ok(())
}

pub fn delete_annotation(conn: &Connection, ann_id: &str) -> Result<()> {
    let changed = conn.execute("DELETE FROM annotations WHERE id = ?1", [ann_id])?;
    if changed == 0 {
        bail!("no annotation with id '{}'", ann_id);
    }
    Ok(())
}

pub fn annotations_for(conn: &Connection, goal_id: &str) -> Result<Vec<Annotation>> {
    let mut stmt = conn.prepare(
        "SELECT id, goal_id, body, created_at FROM annotations WHERE goal_id = ?1 ORDER BY created_at ASC, rowid ASC",
    )?;
    let anns = stmt
        .query_map([goal_id], |row| {
            Ok(Annotation {
                id: row.get(0)?,
                goal_id: row.get(1)?,
                body: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(anns)
}

pub fn resolve_annotation_id(conn: &Connection, prefix: &str) -> Result<String> {
    let pattern = format!("{}%", prefix);
    let mut stmt = conn.prepare("SELECT id FROM annotations WHERE id LIKE ?1")?;
    let ids: Vec<String> = stmt
        .query_map([&pattern], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    match ids.len() {
        0 => bail!("no annotation matching '{}'", prefix),
        1 => Ok(ids.into_iter().next().unwrap()),
        _ => bail!("ambiguous annotation prefix '{}' matches {} annotations", prefix, ids.len()),
    }
}
