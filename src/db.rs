use anyhow::{bail, Context, Result};
use directories::ProjectDirs;
use rand::RngExt;
use rusqlite::Connection;
use std::env;
use std::fs;

#[derive(Debug, Clone, PartialEq)]
pub enum GoalKind {
    Achievable,
    Continuous,
}

#[derive(Debug, Clone)]
pub struct Goal {
    pub id: String,
    pub parent_id: Option<String>,
    pub body: String,
    pub achieved: bool,
    pub kind: GoalKind,
}

/// Encode depth as variable-length base-15 with 'f' as continuation.
/// depth 0–14 → single digit '0'–'e'
/// depth 15–29 → 'f' + '0'–'e'
/// depth 30–44 → 'ff' + '0'–'e'
/// etc.
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

fn kind_from_id(id: &str) -> GoalKind {
    match id.chars().next() {
        Some('a') => GoalKind::Achievable,
        _ => GoalKind::Continuous,
    }
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
        );",
    )?;
    Ok(conn)
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

pub fn add_goal(conn: &Connection, body: &str, parent_id: Option<&str>, kind: &GoalKind) -> Result<String> {
    let depth = if let Some(pid) = parent_id {
        parse_depth(pid) + 1
    } else {
        0
    };
    let id = generate_id(kind, depth);
    conn.execute(
        "INSERT INTO goals (id, parent_id, body) VALUES (?1, ?2, ?3)",
        rusqlite::params![id, parent_id, body],
    )?;
    Ok(id)
}

pub fn set_achieved(conn: &Connection, id: &str, achieved: bool) -> Result<()> {
    let kind = kind_from_id(id);
    if kind == GoalKind::Continuous {
        bail!("goal '{}' is continuous and cannot be marked done", id);
    }
    let changed = conn.execute(
        "UPDATE goals SET achieved = ?1 WHERE id = ?2",
        rusqlite::params![achieved as i32, id],
    )?;
    if changed == 0 {
        bail!("no goal with id '{}'", id);
    }
    Ok(())
}

pub fn remove_goal(conn: &Connection, id: &str) -> Result<()> {
    let changed = conn.execute("DELETE FROM goals WHERE id = ?1", [id])?;
    if changed == 0 {
        bail!("no goal with id '{}'", id);
    }
    Ok(())
}

pub enum ModifyParent {
    Keep,
    Detach,
    Reparent(String),
}

fn children_of(conn: &Connection, parent_id: &str) -> Result<Vec<Goal>> {
    let mut stmt = conn.prepare(
        "SELECT id, parent_id, body, achieved FROM goals WHERE parent_id = ?1 ORDER BY id",
    )?;
    let goals = stmt
        .query_map([parent_id], |row| {
            let id: String = row.get(0)?;
            let kind = kind_from_id(&id);
            Ok(Goal {
                id,
                parent_id: row.get(1)?,
                body: row.get(2)?,
                achieved: row.get::<_, i32>(3)? != 0,
                kind,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(goals)
}

fn collect_subtree(conn: &Connection, root_id: &str) -> Result<Vec<Goal>> {
    let root = {
        let mut stmt = conn.prepare(
            "SELECT id, parent_id, body, achieved FROM goals WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map([root_id], |row| {
            let id: String = row.get(0)?;
            let kind = kind_from_id(&id);
            Ok(Goal {
                id,
                parent_id: row.get(1)?,
                body: row.get(2)?,
                achieved: row.get::<_, i32>(3)? != 0,
                kind,
            })
        })?;
        rows.next().ok_or_else(|| anyhow::anyhow!("no goal with id '{}'", root_id))??
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
        conn.execute(
            "UPDATE goals SET body = ?1 WHERE id = ?2",
            rusqlite::params![new_body_str, id],
        )?;
        return Ok(id.to_string());
    }

    // Build old->new ID mapping for full subtree
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
        // Delete old rows (FK off, order doesn't matter)
        for node in &subtree {
            tx.execute("DELETE FROM goals WHERE id = ?1", [&node.id])?;
        }
        // Insert new rows
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
        tx.commit()?;
        Ok(())
    })();
    conn.execute_batch("PRAGMA foreign_keys = ON")?;
    tx_result?;

    Ok(new_root_id)
}

pub fn all_goals(conn: &Connection) -> Result<Vec<Goal>> {
    let mut stmt = conn.prepare(
        "SELECT id, parent_id, body, achieved FROM goals ORDER BY id",
    )?;
    let goals = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let kind = kind_from_id(&id);
            Ok(Goal {
                id,
                parent_id: row.get(1)?,
                body: row.get(2)?,
                achieved: row.get::<_, i32>(3)? != 0,
                kind,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(goals)
}
