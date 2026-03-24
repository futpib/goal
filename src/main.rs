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

    match args.command.unwrap_or(Command::List) {
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
        Command::Modify { id, body, parent, no_parent, continuous, achievable } => {
            if body.is_none() && parent.is_none() && !no_parent && !continuous && !achievable {
                anyhow::bail!("modify: specify at least one of --body, --parent, --no-parent, --continuous, --achievable");
            }
            let full_id = db::resolve_id(&conn, &id)?;
            let new_parent = if no_parent {
                db::ModifyParent::Detach
            } else if let Some(ref p) = parent {
                db::ModifyParent::Reparent(db::resolve_id(&conn, p)?)
            } else {
                db::ModifyParent::Keep
            };
            let new_kind = match (continuous, achievable) {
                (true, _) => Some(db::GoalKind::Continuous),
                (_, true) => Some(db::GoalKind::Achievable),
                _ => None,
            };
            let final_id = db::modify_goal(&conn, &full_id, body.as_deref(), new_parent, new_kind)?;
            println!("{}", final_id);
        }
        Command::Delete { id } => {
            let full_id = db::resolve_id(&conn, &id)?;
            db::remove_goal(&conn, &full_id)?;
        }
        Command::Info { id } => {
            let full_id = db::resolve_id(&conn, &id)?;
            let subtree = db::collect_subtree(&conn, &full_id)?;
            display::print_info(&subtree);
        }
        Command::Undo => {
            db::undo_last(&conn)?;
        }
        Command::Log => {
            let events = db::list_events(&conn)?;
            display::print_log(&events);
        }
    }

    Ok(())
}
