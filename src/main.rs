// src/main.rs
mod schema;
mod template;
mod executor;

use anyhow::{Context, Result};
use clap::Parser;
use schema::Document;

#[derive(Parser, Debug)]
#[command(name="rust-runner", version, about="YAML-gesteuerte Ausführung")]
struct Cli {
    /// Pfad zur YAML-Datei
    file: String,
    /// Dry-Run (nichts ausführen)
    #[arg(long)]
    dry_run: bool,
    /// Verbose Logging
    #[arg(long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let raw = std::fs::read_to_string(&cli.file).context("YAML lesen")?;
    let doc: Document = serde_yaml::from_str(&raw).context("YAML parsen")?;

    let exec = executor::Executor::new(doc.globals, cli.verbose, cli.dry_run);

    for (i, step) in doc.steps.iter().enumerate() {
        if let Err(e) = exec.run_step(step, i).await {
            eprintln!("Fehler in Schritt {}: {:?}", i + 1, e);
            std::process::exit(1);
        }
    }
    Ok(())
}
