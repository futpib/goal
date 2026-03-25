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

/// Derive a new ID from an existing one, preserving the random suffix.
/// Only the type char and depth prefix change; the random portion is reused
/// as much as possible so the new ID shares the longest common infix with
/// the old one.
pub fn derive_id(old_id: &str, new_kind: &GoalKind, new_depth: u32) -> String {
    let type_char = match new_kind {
        GoalKind::Achievable => 'a',
        GoalKind::Continuous => 'c',
    };
    let new_depth_str = encode_depth(new_depth);
    let new_prefix = format!("{}{}", type_char, new_depth_str);
    let new_random_len = 16usize.saturating_sub(new_prefix.len());

    let old_depth = parse_depth(old_id);
    let old_prefix_len = 1 + encode_depth(old_depth).len();
    let old_random = &old_id[old_prefix_len..];

    if old_random.len() >= new_random_len {
        format!("{}{}", new_prefix, &old_random[..new_random_len])
    } else {
        // New prefix is shorter than old; pad the random portion with fresh bytes
        let extra_len = new_random_len - old_random.len();
        let mut bytes = vec![0u8; (extra_len + 1) / 2];
        rand::rng().fill(&mut bytes[..]);
        let extra_hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
        format!("{}{}{}", new_prefix, old_random, &extra_hex[..extra_len])
    }
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
        );
        CREATE TABLE IF NOT EXISTS goal_tags (
            goal_id    TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
            tag        TEXT NOT NULL,
            PRIMARY KEY (goal_id, tag)
        );
        CREATE TABLE IF NOT EXISTS id_aliases (
            old_id  TEXT PRIMARY KEY,
            new_id  TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS priority_edges (
            higher_id TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
            lower_id  TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
            PRIMARY KEY (higher_id, lower_id)
        );",
    )?;
    let _ = conn.execute_batch("ALTER TABLE events ADD COLUMN snapshot_edges TEXT");
    let _ = conn.execute_batch("ALTER TABLE events_undone ADD COLUMN snapshot_edges TEXT");
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
        1 => return Ok(ids.into_iter().next().unwrap()),
        n if n > 1 => bail!("ambiguous prefix '{}' matches {} goals", prefix, ids.len()),
        _ => {}
    }

    // No direct match — try alias table
    let alias_pattern = format!("{}%", prefix);
    let mut stmt2 = conn.prepare(
        "SELECT a.old_id, a.new_id FROM id_aliases a WHERE a.old_id LIKE ?1",
    )?;
    let alias_rows: Vec<(String, String)> = stmt2
        .query_map([&alias_pattern], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    match alias_rows.len() {
        0 => bail!("no goal matching '{}'", prefix),
        1 => {
            let (old_id, new_id) = alias_rows.into_iter().next().unwrap();
            eprintln!("warning: '{}' was renamed to '{}'; using new id", old_id, new_id);
            Ok(new_id)
        }
        _ => bail!("ambiguous prefix '{}' matches {} goal aliases", prefix, alias_rows.len()),
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
    let subtree_ids: std::collections::HashSet<&str> = subtree.iter().map(|g| g.id.as_str()).collect();

    // Collect all priority edges touching the subtree
    let mut removed_edges: Vec<(String, String)> = Vec::new();
    for node in &subtree {
        let mut stmt = conn.prepare(
            "SELECT higher_id, lower_id FROM priority_edges WHERE higher_id = ?1 OR lower_id = ?1",
        )?;
        let edges: Vec<(String, String)> = stmt
            .query_map([&node.id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for edge in edges {
            if !removed_edges.contains(&edge) {
                removed_edges.push(edge);
            }
        }
    }

    // Compute transitive collapse: for each deleted node, connect its uppers to its lowers
    let mut added_edges: Vec<(String, String)> = Vec::new();
    for node in &subtree {
        let uppers: Vec<String> = {
            let mut stmt = conn.prepare(
                "SELECT higher_id FROM priority_edges WHERE lower_id = ?1",
            )?;
            stmt.query_map([&node.id], |row| row.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        let lowers: Vec<String> = {
            let mut stmt = conn.prepare(
                "SELECT lower_id FROM priority_edges WHERE higher_id = ?1",
            )?;
            stmt.query_map([&node.id], |row| row.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        for upper in &uppers {
            if subtree_ids.contains(upper.as_str()) {
                continue;
            }
            for lower in &lowers {
                if subtree_ids.contains(lower.as_str()) {
                    continue;
                }
                let new_edge = (upper.clone(), lower.clone());
                if !added_edges.contains(&new_edge) && !removed_edges.contains(&new_edge) {
                    added_edges.push(new_edge);
                }
            }
        }
    }

    let edge_snapshot = EdgeSnapshot { removed: removed_edges, added: added_edges.clone() };
    let snapshot_edges_json = serde_json::to_string(&edge_snapshot)?;

    conn.execute_batch("PRAGMA foreign_keys = OFF")?;
    let tx_result = (|| -> Result<()> {
        let tx = conn.unchecked_transaction()?;
        // Insert synthetic transitive edges before goals are deleted
        for (h, l) in &added_edges {
            tx.execute(
                "INSERT OR IGNORE INTO priority_edges (higher_id, lower_id) VALUES (?1, ?2)",
                rusqlite::params![h, l],
            )?;
        }
        // Delete all subtree nodes explicitly (foreign_keys is OFF, cascades don't fire)
        for node in &subtree {
            tx.execute("DELETE FROM goals WHERE id = ?1", [&node.id])?;
        }
        let event_id = record_event(&tx, "Delete", &subtree, None)?;
        tx.execute(
            "UPDATE events SET snapshot_edges = ?1 WHERE event_id = ?2",
            rusqlite::params![snapshot_edges_json, event_id],
        )?;
        tx.commit()?;
        Ok(())
    })();
    conn.execute_batch("PRAGMA foreign_keys = ON")?;
    tx_result?;
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
        id_map.insert(node.id.clone(), derive_id(&node.id, &node_kind, node_new_depth));
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
            // Record alias: old_id -> new_id. If old_id already has an alias
            // pointing to it, update that alias to skip the middle step.
            tx.execute(
                "INSERT INTO id_aliases (old_id, new_id) VALUES (?1, ?2)
                 ON CONFLICT(old_id) DO UPDATE SET new_id = excluded.new_id",
                rusqlite::params![node.id, new_id],
            )?;
            tx.execute(
                "UPDATE id_aliases SET new_id = ?1 WHERE new_id = ?2",
                rusqlite::params![new_id, node.id],
            )?;
            // Migrate priority edges to the new ID
            tx.execute(
                "UPDATE priority_edges SET higher_id = ?1 WHERE higher_id = ?2",
                rusqlite::params![new_id, node.id],
            )?;
            tx.execute(
                "UPDATE priority_edges SET lower_id = ?1 WHERE lower_id = ?2",
                rusqlite::params![new_id, node.id],
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
            "SELECT event_id, op, snapshot, new_id, snapshot_edges FROM events \
             ORDER BY timestamp DESC, rowid DESC LIMIT 1",
        )?;
        let mut rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })?;
        match rows.next() {
            None => bail!("nothing to undo"),
            Some(r) => r?,
        }
    };
    let (event_id, op, snapshot_json, new_id, snapshot_edges) = row;
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
                if let Some(ref edges_json) = snapshot_edges {
                    let edge_snapshot: EdgeSnapshot = serde_json::from_str(edges_json)?;
                    // Remove synthetic edges that were added during collapse
                    for (h, l) in &edge_snapshot.added {
                        tx.execute(
                            "DELETE FROM priority_edges WHERE higher_id = ?1 AND lower_id = ?2",
                            rusqlite::params![h, l],
                        )?;
                    }
                    // Re-insert original edges
                    for (h, l) in &edge_snapshot.removed {
                        tx.execute(
                            "INSERT OR IGNORE INTO priority_edges (higher_id, lower_id) VALUES (?1, ?2)",
                            rusqlite::params![h, l],
                        )?;
                    }
                }
            }
            "Prioritize" => {
                let (h, l) = parse_edge_new_id(
                    new_id.as_deref().ok_or_else(|| anyhow::anyhow!("malformed Prioritize event"))?,
                )?;
                tx.execute(
                    "DELETE FROM priority_edges WHERE higher_id = ?1 AND lower_id = ?2",
                    rusqlite::params![h, l],
                )?;
            }
            "Deprioritize" => {
                let (h, l) = parse_edge_new_id(
                    new_id.as_deref().ok_or_else(|| anyhow::anyhow!("malformed Deprioritize event"))?,
                )?;
                tx.execute(
                    "INSERT OR IGNORE INTO priority_edges (higher_id, lower_id) VALUES (?1, ?2)",
                    rusqlite::params![h, l],
                )?;
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
            "INSERT INTO events_undone (event_id, timestamp, op, snapshot, new_id, snapshot_edges) \
             SELECT event_id, timestamp, op, snapshot, new_id, snapshot_edges FROM events WHERE event_id = ?1",
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

#[derive(Serialize, Deserialize)]
struct EdgeSnapshot {
    removed: Vec<(String, String)>,
    added: Vec<(String, String)>,
}

fn would_create_cycle(conn: &Connection, higher_id: &str, lower_id: &str) -> Result<bool> {
    // A cycle exists if lower_id can reach higher_id by following existing downward edges.
    // (i.e., lower_id already ranks higher than ... higher_id somehow)
    let mut stmt = conn.prepare(
        "WITH RECURSIVE reachable(id) AS (
            SELECT pe.lower_id FROM priority_edges pe WHERE pe.higher_id = ?1
            UNION
            SELECT pe2.lower_id FROM priority_edges pe2 JOIN reachable r ON pe2.higher_id = r.id
         )
         SELECT id FROM reachable WHERE id = ?2",
    )?;
    let rows: Vec<String> = stmt
        .query_map(rusqlite::params![lower_id, higher_id], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(!rows.is_empty())
}

fn parse_edge_new_id(s: &str) -> Result<(String, String)> {
    let pos = s.find(':')
        .ok_or_else(|| anyhow::anyhow!("malformed edge new_id: '{}'", s))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

pub fn add_priority_edge(conn: &Connection, higher_id: &str, lower_id: &str) -> Result<()> {
    if higher_id == lower_id {
        bail!("cannot prioritize a goal over itself");
    }
    let already_exists: bool = conn.query_row(
        "SELECT 1 FROM priority_edges WHERE higher_id = ?1 AND lower_id = ?2",
        rusqlite::params![higher_id, lower_id],
        |_| Ok(true),
    ).unwrap_or(false);
    if already_exists {
        bail!("priority edge already exists: {} > {}", higher_id, lower_id);
    }
    if would_create_cycle(conn, higher_id, lower_id)? {
        bail!("adding this priority relation would create a cycle");
    }
    let higher_goal = get_goal(conn, higher_id)?;
    let lower_goal = get_goal(conn, lower_id)?;
    let snapshot_json = serde_json::to_string(&[&higher_goal, &lower_goal])?;
    let event_id = generate_event_id();
    let new_id_str = format!("{}:{}", higher_id, lower_id);
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO priority_edges (higher_id, lower_id) VALUES (?1, ?2)",
        rusqlite::params![higher_id, lower_id],
    )?;
    tx.execute(
        "INSERT INTO events (event_id, timestamp, op, snapshot, new_id) \
         VALUES (?1, datetime('now'), 'Prioritize', ?2, ?3)",
        rusqlite::params![event_id, snapshot_json, new_id_str],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn remove_priority_edge(conn: &Connection, higher_id: &str, lower_id: &str) -> Result<()> {
    let exists: bool = conn.query_row(
        "SELECT 1 FROM priority_edges WHERE higher_id = ?1 AND lower_id = ?2",
        rusqlite::params![higher_id, lower_id],
        |_| Ok(true),
    ).unwrap_or(false);
    if !exists {
        bail!("no priority edge {} > {}", higher_id, lower_id);
    }
    let higher_goal = get_goal(conn, higher_id)?;
    let lower_goal = get_goal(conn, lower_id)?;
    let snapshot_json = serde_json::to_string(&[&higher_goal, &lower_goal])?;
    let event_id = generate_event_id();
    let new_id_str = format!("{}:{}", higher_id, lower_id);
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "DELETE FROM priority_edges WHERE higher_id = ?1 AND lower_id = ?2",
        rusqlite::params![higher_id, lower_id],
    )?;
    tx.execute(
        "INSERT INTO events (event_id, timestamp, op, snapshot, new_id) \
         VALUES (?1, datetime('now'), 'Deprioritize', ?2, ?3)",
        rusqlite::params![event_id, snapshot_json, new_id_str],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn all_priority_edges(conn: &Connection) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT higher_id, lower_id FROM priority_edges")?;
    let edges = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(edges)
}

pub fn goal_rowids(conn: &Connection) -> Result<std::collections::HashMap<String, i64>> {
    let mut stmt = conn.prepare("SELECT id, rowid FROM goals")?;
    let mut map = std::collections::HashMap::new();
    let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?;
    for row in rows {
        let (id, rowid) = row?;
        map.insert(id, rowid);
    }
    Ok(map)
}

pub fn compute_priority_order(
    goals: &[Goal],
    edges: &[(String, String)],
    rowids: &std::collections::HashMap<String, i64>,
) -> Vec<String> {
    use std::collections::{HashMap, HashSet, BinaryHeap};
    use std::cmp::Ordering;

    let goal_ids: HashSet<&str> = goals.iter().map(|g| g.id.as_str()).collect();

    // Only consider edges where both endpoints are in the current goal set
    let active_edges: Vec<(&str, &str)> = edges.iter()
        .filter(|(h, l)| goal_ids.contains(h.as_str()) && goal_ids.contains(l.as_str()))
        .map(|(h, l)| (h.as_str(), l.as_str()))
        .collect();

    let linked_ids: HashSet<&str> = active_edges.iter()
        .flat_map(|(h, l)| [*h, *l])
        .collect();

    // Build adjacency: higher -> [lower]
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    for g in goals {
        in_degree.entry(g.id.as_str()).or_insert(0);
    }
    for (h, l) in &active_edges {
        adj.entry(h).or_default().push(l);
        *in_degree.entry(l).or_insert(0) += 1;
    }

    // Comparator: higher priority = smaller ordering value for BinaryHeap (max-heap)
    // We wrap IDs in a struct that reverses comparisons to get max-heap behavior.
    #[derive(Eq, PartialEq)]
    struct Item {
        id: String,
        linked: bool,
        rowid: i64,
    }
    impl Ord for Item {
        fn cmp(&self, other: &Self) -> Ordering {
            match (self.linked, other.linked) {
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                _ => self.rowid.cmp(&other.rowid),
            }
        }
    }
    impl PartialOrd for Item {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    let make_item = |id: &str| Item {
        id: id.to_string(),
        linked: linked_ids.contains(id),
        rowid: *rowids.get(id).unwrap_or(&0),
    };

    let mut heap: BinaryHeap<Item> = goals.iter()
        .filter(|g| in_degree.get(g.id.as_str()).copied().unwrap_or(0) == 0)
        .map(|g| make_item(g.id.as_str()))
        .collect();

    let mut result = Vec::with_capacity(goals.len());
    while let Some(item) = heap.pop() {
        if let Some(lowers) = adj.get(item.id.as_str()) {
            let lowers_copy: Vec<&str> = lowers.clone();
            for lower in lowers_copy {
                let deg = in_degree.entry(lower).or_insert(0);
                if *deg > 0 {
                    *deg -= 1;
                    if *deg == 0 {
                        heap.push(make_item(lower));
                    }
                }
            }
        }
        result.push(item.id);
    }

    // Append any goals not reached (shouldn't happen with a valid DAG, but be safe)
    let in_result: HashSet<&str> = result.iter().map(|s| s.as_str()).collect();
    let missing: Vec<String> = goals.iter()
        .filter(|g| !in_result.contains(g.id.as_str()))
        .map(|g| g.id.clone())
        .collect();
    result.extend(missing);

    result
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

/// Returns pairs of pending goal IDs where no ordering (direct or transitive) exists yet.
/// Each pair (a, b) appears only once (a < b lexicographically).
pub fn unranked_pairs(conn: &Connection) -> Result<Vec<(Goal, Goal)>> {
    let goals = all_goals(conn)?;
    let pending: Vec<&Goal> = goals.iter().filter(|g| !g.achieved).collect();
    let edges = all_priority_edges(conn)?;

    // Build transitive reachability: reachable[a] = set of goals reachable from a (a > them)
    use std::collections::{HashMap, HashSet};
    let mut reachable: HashMap<&str, HashSet<&str>> = HashMap::new();
    let pending_ids: HashSet<&str> = pending.iter().map(|g| g.id.as_str()).collect();

    for g in &pending {
        reachable.insert(g.id.as_str(), HashSet::new());
    }
    for (h, l) in &edges {
        if pending_ids.contains(h.as_str()) && pending_ids.contains(l.as_str()) {
            reachable.entry(h.as_str()).or_default().insert(l.as_str());
        }
    }
    // Expand transitively (Floyd-Warshall style)
    let ids: Vec<&str> = pending.iter().map(|g| g.id.as_str()).collect();
    loop {
        let mut changed = false;
        for &a in &ids {
            let via: Vec<&str> = reachable[a].iter().copied().collect();
            for b in via {
                let new_reach: Vec<&str> = reachable.get(b).map(|s| s.iter().copied().collect()).unwrap_or_default();
                for c in new_reach {
                    if reachable.entry(a).or_default().insert(c) {
                        changed = true;
                    }
                }
            }
        }
        if !changed { break; }
    }

    let mut pairs = Vec::new();
    for i in 0..pending.len() {
        for j in (i + 1)..pending.len() {
            let a = pending[i];
            let b = pending[j];
            let a_reaches_b = reachable.get(a.id.as_str()).map_or(false, |s| s.contains(b.id.as_str()));
            let b_reaches_a = reachable.get(b.id.as_str()).map_or(false, |s| s.contains(a.id.as_str()));
            if !a_reaches_b && !b_reaches_a {
                pairs.push((a.clone(), b.clone()));
            }
        }
    }
    Ok(pairs)
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

pub fn add_tag(conn: &Connection, goal_id: &str, tag: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO goal_tags (goal_id, tag) VALUES (?1, ?2)",
        rusqlite::params![goal_id, tag],
    )?;
    Ok(())
}

pub fn remove_tag(conn: &Connection, goal_id: &str, tag: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM goal_tags WHERE goal_id = ?1 AND tag = ?2",
        rusqlite::params![goal_id, tag],
    )?;
    Ok(())
}


pub fn all_goal_tags(conn: &Connection) -> Result<std::collections::HashMap<String, Vec<String>>> {
    let mut stmt = conn.prepare("SELECT goal_id, tag FROM goal_tags ORDER BY goal_id, tag")?;
    let mut map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (goal_id, tag) = row?;
        map.entry(goal_id).or_default().push(tag);
    }
    Ok(map)
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
