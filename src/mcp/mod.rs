use clap::{ArgMatches, Command};

mod server;
mod tools;
mod analysis;

pub use server::run;

/// MCP server configuration
pub struct Config {
    pub verbose: u8,
}

impl TryFrom<ArgMatches> for Config {
    type Error = String;

    fn try_from(args: ArgMatches) -> Result<Self, String> {
        Ok(Config {
            verbose: *args.get_one::<u8>("VERBOSE").unwrap_or(&0),
        })
    }
}

/// Create the MCP subcommand
pub fn command() -> Command {
    Command::new("mcp")
        .about("Run Rezolus MCP server for AI analysis")
        .arg(
            clap::Arg::new("VERBOSE")
                .long("verbose")
                .short('v')
                .help("Increase verbosity")
                .action(clap::ArgAction::Count),
        )
}