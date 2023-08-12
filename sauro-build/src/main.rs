mod cmd;
mod expand;
mod syntax;

use clap::{Parser, Subcommand};

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Build(cmd) => cmd.run(),
    }
}

#[derive(Parser)]
#[clap(
    arg_required_else_help = true,
    disable_help_flag = true,
    disable_version_flag = true,
    help_expected = true,
    subcommand_required = true
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Build(cmd::BuildCommand),
}
