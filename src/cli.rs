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
    Rm {
        id: String,
    },
}
