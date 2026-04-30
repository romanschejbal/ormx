#![allow(clippy::pedantic)]

//! CLI tool for the ferriorm ORM.
//!
//! Provides commands for initializing projects, generating the Rust client,
//! managing migrations, and introspecting databases. Run `ferriorm --help` for
//! full usage information.

mod commands;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "ferriorm",
    about = "Prisma-like ORM for Rust\n\nDocs: https://romanschejbal.github.io/ferriorm",
    version
)]
struct Cli {
    /// Path to the schema file
    #[arg(long, default_value = "schema.ferriorm")]
    schema: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new ferriorm project
    Init {
        /// Database provider (postgresql, sqlite)
        #[arg(long, default_value = "postgresql")]
        provider: String,
    },

    /// Generate the Rust client from the schema
    Generate,

    /// Format `.ferriorm` files in place (or check formatting with `--check`)
    Format {
        /// Print a diff for each file that would change and exit 1 (no writes).
        #[arg(long)]
        check: bool,
        /// Read source from stdin and write the formatted output to stdout.
        #[arg(long)]
        stdin: bool,
        /// Files or directories to format. Defaults to `--schema` when empty.
        paths: Vec<String>,
    },

    /// Migration management
    Migrate {
        #[command(subcommand)]
        command: MigrateCommands,
    },

    /// Database operations
    Db {
        #[command(subcommand)]
        command: DbCommands,
    },
}

#[derive(Subcommand)]
enum MigrateCommands {
    /// Create a migration, apply it, and regenerate client (development)
    Dev {
        /// Migration name
        #[arg(long)]
        name: Option<String>,

        /// Use snapshot strategy instead of shadow database (offline mode)
        #[arg(long)]
        snapshot: bool,
    },
    /// Apply pending migrations (production)
    Deploy,
    /// Show migration status
    Status,
}

#[derive(Subcommand)]
enum DbCommands {
    /// Introspect a live database and generate a schema.ferriorm file
    Pull,
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Init { provider } => commands::init::run(&provider).await,
        Commands::Generate => commands::generate::run(&cli.schema).await,
        Commands::Format {
            check,
            stdin,
            paths,
        } => commands::format::run(&cli.schema, paths, check, stdin).await,
        Commands::Migrate { command } => match command {
            MigrateCommands::Dev { name, snapshot } => {
                commands::migrate::dev(&cli.schema, name.as_deref(), snapshot).await
            }
            MigrateCommands::Deploy => commands::migrate::deploy(&cli.schema).await,
            MigrateCommands::Status => commands::migrate::status(&cli.schema).await,
        },
        Commands::Db { command } => match command {
            DbCommands::Pull => commands::db::pull(&cli.schema).await,
        },
    }
}
