use std::error::Error;

use clap::{Parser, Subcommand};
use stampede::{ChatToolsArgs, CliCommand, HoldArgs, HoldChatArgs, InstallArgs, ServeArgs, UninstallArgs};

#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Serve(ServeArgs),
    Hold(HoldArgs),
    HoldChat(HoldChatArgs),
    ChatTools(ChatToolsArgs),
    Install(InstallArgs),
    Uninstall(UninstallArgs),
    #[command(flatten)]
    Cli(CliCommand),
}

fn main() -> Result<(), Box<dyn Error>> {
    match Cli::parse().command {
        Command::Serve(args) => {
            Ok(tokio::runtime::Runtime::new()?.block_on(stampede::serve(args))?)
        }
        Command::Hold(args) => stampede::hold(args),
        Command::HoldChat(args) => stampede::hold_chat(args),
        Command::ChatTools(args) => Ok(stampede::chat_tools(args)?),
        Command::Install(args) => Ok(stampede::install(args)?),
        Command::Uninstall(args) => Ok(stampede::uninstall(args)?),
        Command::Cli(command) => {
            if let Err(error) = stampede::cli(command) {
                eprintln!("error: {error}");
                std::process::exit(1);
            }
            Ok(())
        }
    }
}
