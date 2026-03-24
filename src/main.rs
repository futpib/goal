mod cli;
mod db;
mod display;
#[cfg(test)]
mod tests;

use anyhow::Result;
use clap::Parser;
use cli::{Args, Command};

fn main() -> Result<()> {
    let args = Args::parse();
    let conn = db::open_db()?;

    match args.command {
        Command::Add { description, parent, continuous } => {
            let kind = if continuous {
                db::GoalKind::Continuous
            } else {
                db::GoalKind::Achievable
            };
            let parent_id = match parent {
                Some(ref prefix) => Some(db::resolve_id(&conn, prefix)?),
                None => None,
            };
            let id = db::add_goal(&conn, &description, parent_id.as_deref(), &kind)?;
            println!("{}", id);
        }
        Command::Done { id } => {
            let full_id = db::resolve_id(&conn, &id)?;
            db::set_achieved(&conn, &full_id, true)?;
        }
        Command::Undone { id } => {
            let full_id = db::resolve_id(&conn, &id)?;
            db::set_achieved(&conn, &full_id, false)?;
        }
        Command::List => {
            let goals = db::all_goals(&conn)?;
            display::print_tree(&goals);
        }
        Command::Delete { id } => {
            let full_id = db::resolve_id(&conn, &id)?;
            db::remove_goal(&conn, &full_id)?;
        }
    }

    Ok(())
}
