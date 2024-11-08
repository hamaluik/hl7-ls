use clap::{ColorChoice, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author = clap::crate_authors!(), version, about, long_about = None, help_template = "\
{before-help}{name} {version}
by {author-with-newline}{about-with-newline}
{usage-heading} {usage}

{all-args}{after-help}
")]
#[command(propagate_version = true)]
pub struct Cli {
    #[arg(short, long, default_value_t = ColorChoice::Auto)]
    /// Control whether color is used in the output
    pub colour: ColorChoice,

    /// Enable debugging output
    ///
    /// Use multiple times to increase verbosity
    /// (e.g., -v, -vv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Enable Visual Studio Code mode
    ///
    /// This mode is intended for when running this language server through
    /// Visual Studio Code.
    #[arg(long)]
    pub vscode: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Log outout to standard error (default)
    LogToStderr,

    /// Log output to a file
    LogToFile {
        /// Path to the log file
        ///
        /// Log file will be created if it does not exist and appended to if it does.
        log_file: PathBuf,
    },
}

pub fn cli() -> Cli {
    Cli::parse()
}
