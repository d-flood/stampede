use clap::{Parser, Subcommand};
use oikonomos::ServeArgs;

#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Serve(ServeArgs),
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    match Cli::parse().command {
        Command::Serve(args) => oikonomos::serve(args).await,
    }
}
