mod cli;
mod db;
mod display;

use anyhow::Result;
use clap::Parser;
use cli::{Args, Command};
use std::collections::HashMap;

fn main() -> Result<()> {
    let args = Args::parse();
    let conn = db::open_db()?;

    match args.command.unwrap_or(Command::List) {
        Command::Add { description, parent, continuous, tags } => {
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
            for tag in &tags {
                if let Some(name) = tag.strip_prefix('+') {
                    db::add_tag(&conn, &id, name)?;
                } else if let Some(name) = tag.strip_prefix('-') {
                    db::remove_tag(&conn, &id, name)?;
                }
            }
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
            let pending: Vec<_> = goals.into_iter().filter(|g| !g.achieved).collect();
            let tags = db::all_goal_tags(&conn)?;
            let edges = db::all_priority_edges(&conn)?;
            let rowids = db::goal_rowids(&conn)?;
            let ordered_ids = db::compute_priority_order(&pending, &edges, &rowids);
            let ranks: HashMap<String, usize> = ordered_ids.iter().enumerate()
                .map(|(i, id)| (id.clone(), i + 1))
                .collect();
            display::print_tree(&pending, &tags, &ranks);
        }
        Command::Modify { id, body, parent, no_parent, continuous, achievable, tags } => {
            if body.is_none() && parent.is_none() && !no_parent && !continuous && !achievable && tags.is_empty() {
                anyhow::bail!("modify: specify at least one of --body, --parent, --no-parent, --continuous, --achievable, or a tag");
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
            let final_id = if body.is_some() || parent.is_some() || no_parent || continuous || achievable {
                db::modify_goal(&conn, &full_id, body.as_deref(), new_parent, new_kind)?
            } else {
                full_id.clone()
            };
            for tag in &tags {
                if let Some(name) = tag.strip_prefix('+') {
                    db::add_tag(&conn, &final_id, name)?;
                } else if let Some(name) = tag.strip_prefix('-') {
                    db::remove_tag(&conn, &final_id, name)?;
                }
            }
            println!("{}", final_id);
        }
        Command::Delete { id, yes } => {
            let full_id = db::resolve_id(&conn, &id)?;
            if !yes {
                let goal = db::get_goal(&conn, &full_id)?;
                eprint!("Delete '{}'? [y/N] ", goal.body);
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    return Ok(());
                }
            }
            db::remove_goal(&conn, &full_id)?;
        }
        Command::Annotate { id, text, edit, delete } => {
            let full_id = db::resolve_id(&conn, &id)?;
            if let Some(ann_id) = delete {
                let full_ann_id = db::resolve_annotation_id(&conn, &ann_id)?;
                db::delete_annotation(&conn, &full_ann_id)?;
            } else if let Some(ann_id) = edit {
                let full_ann_id = db::resolve_annotation_id(&conn, &ann_id)?;
                let body = text.ok_or_else(|| anyhow::anyhow!("annotate --edit requires a text argument"))?;
                db::edit_annotation(&conn, &full_ann_id, &body)?;
            } else {
                let body = text.ok_or_else(|| anyhow::anyhow!("annotate requires a text argument"))?;
                let ann_id = db::add_annotation(&conn, &full_id, &body)?;
                println!("{}", ann_id);
            }
        }
        Command::Info { id } => {
            let full_id = db::resolve_id(&conn, &id)?;
            let subtree = db::collect_subtree(&conn, &full_id)?;
            let annotations = db::annotations_for(&conn, &full_id)?;
            let tags = db::all_goal_tags(&conn)?;
            display::print_info(&subtree, &annotations, &tags);
        }
        Command::Undo => {
            db::undo_last(&conn)?;
        }
        Command::Log => {
            let events = db::list_events(&conn)?;
            display::print_log(&events);
        }
        Command::Rank { higher, lower } => {
            let h = db::resolve_id(&conn, &higher)?;
            let l = db::resolve_id(&conn, &lower)?;
            db::add_priority_edge(&conn, &h, &l)?;
        }
        Command::Unrank { higher, lower } => {
            let h = db::resolve_id(&conn, &higher)?;
            let l = db::resolve_id(&conn, &lower)?;
            db::remove_priority_edge(&conn, &h, &l)?;
        }
    }

    Ok(())
}
