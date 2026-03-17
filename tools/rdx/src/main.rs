mod commands;
mod config;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "rdx", version, about = "The Redox language CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Print verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Suppress output
    #[arg(short, long, global = true)]
    quiet: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Create a new Redox project
    New {
        /// Project name
        name: String,
        /// Create a library project
        #[arg(long)]
        lib: bool,
    },
    /// Initialize Redox in the current directory
    Init {
        /// Create a library project
        #[arg(long)]
        lib: bool,
    },
    /// Compile the project
    Build {
        /// Build with optimizations
        #[arg(long)]
        release: bool,
    },
    /// Type-check without codegen
    Check,
    /// Run tests
    Test {
        /// Filter tests by name
        filter: Option<String>,
    },
    /// Format source code
    Fmt {
        /// Check formatting without writing changes
        #[arg(long)]
        check: bool,
    },
    /// Run the project
    Run {
        /// Arguments to pass to the program
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Run rust2rdx on a Rust project
    Migrate {
        /// Path to Rust source file or directory
        path: String,
        /// Show diff instead of writing files
        #[arg(long)]
        diff: bool,
        /// Show token statistics
        #[arg(long)]
        stats: bool,
    },
    /// Start RAP language server
    Rap {
        /// Address to bind
        #[arg(default_value = "127.0.0.1:9876")]
        addr: String,
    },
    /// Query Safety Knowledge Base
    Skb {
        /// Query string (e.g. "ownership:move")
        query: Option<String>,
        /// Validate SKB rules against project
        #[arg(long)]
        validate: bool,
    },
    /// Show cost oracle data for a function
    Cost {
        /// Function name
        function: String,
    },
    /// Run the end-to-end pipeline (parse → check → MLIR)
    Pipeline {
        /// Path to .rdx file
        path: String,
    },
    /// Generate documentation
    Doc {
        /// Open docs in browser after generating
        #[arg(long)]
        open: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::New { name, lib } => commands::new_project(&name, lib, cli.verbose),
        Command::Init { lib } => commands::init_project(lib, cli.verbose),
        Command::Build { release } => commands::build(release, cli.verbose),
        Command::Check => commands::check(cli.verbose),
        Command::Test { filter } => commands::test(filter.as_deref(), cli.verbose),
        Command::Fmt { check } => commands::fmt(check, cli.verbose),
        Command::Run { args } => commands::run(&args, cli.verbose),
        Command::Migrate { path, diff, stats } => {
            commands::migrate(&path, diff, stats, cli.verbose)
        }
        Command::Rap { addr } => commands::rap(&addr, cli.verbose),
        Command::Skb { query, validate } => commands::skb(query.as_deref(), validate, cli.verbose),
        Command::Cost { function } => commands::cost(&function, cli.verbose),
        Command::Pipeline { path } => commands::pipeline(&path, cli.verbose),
        Command::Doc { open } => commands::doc(open, cli.verbose),
    };
    if let Err(e) = result {
        if !cli.quiet {
            eprintln!("\x1b[31merror\x1b[0m: {e}");
        }
        std::process::exit(1);
    }
}
