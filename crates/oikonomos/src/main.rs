use std::error::Error;

use clap::{Parser, Subcommand};
use oikonomos::{HoldArgs, HookArgs, InstallArgs, ServeArgs, UninstallArgs};

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
    Hook(HookArgs),
    Install(InstallArgs),
    Uninstall(UninstallArgs),
}

fn main() -> Result<(), Box<dyn Error>> {
    match Cli::parse().command {
        Command::Serve(args) => {
            Ok(tokio::runtime::Runtime::new()?.block_on(oikonomos::serve(args))?)
        }
        Command::Hold(args) => oikonomos::hold(args),
        Command::Hook(args) => {
            oikonomos::hook(args);
            Ok(())
        }
        Command::Install(args) => Ok(oikonomos::install(args)?),
        Command::Uninstall(args) => Ok(oikonomos::uninstall(args)?),
    }
}
