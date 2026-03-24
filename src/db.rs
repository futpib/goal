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
