use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "goal", about = "Hierarchical goal tracker")]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Add {
        description: String,
        #[arg(long)]
        parent: Option<String>,
        #[arg(long, short)]
        continuous: bool,
    },
    Done {
        id: String,
    },
    Undone {
        id: String,
    },
    List,
    Modify {
        id: String,
        #[arg(long)]
        body: Option<String>,
        #[arg(long, conflicts_with = "no_parent")]
        parent: Option<String>,
        #[arg(long = "no-parent", conflicts_with = "parent")]
        no_parent: bool,
        #[arg(long, short, conflicts_with = "achievable")]
        continuous: bool,
        #[arg(long, conflicts_with = "continuous")]
        achievable: bool,
    },
    Delete {
        id: String,
    },
    Undo,
    Log,
}
