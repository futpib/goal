use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "goal", about = "Hierarchical goal tracker")]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    Add {
        description: String,
        #[arg(long)]
        parent: Option<String>,
        #[arg(long, short)]
        continuous: bool,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        tags: Vec<String>,
    },
    Done {
        id: String,
    },
    Undone {
        id: String,
    },
    List {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        tags: Vec<String>,
    },
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
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        tags: Vec<String>,
    },
    Delete {
        id: String,
        #[arg(long, short)]
        yes: bool,
    },
    Info {
        id: String,
    },
    Annotate {
        id: String,
        text: Option<String>,
        #[arg(long, conflicts_with = "delete")]
        edit: Option<String>,
        #[arg(long, conflicts_with = "edit")]
        delete: Option<String>,
    },
    Undo,
    Log,
    Rank {
        higher: String,
        lower: String,
    },
    Unrank {
        higher: String,
        lower: String,
    },
}
