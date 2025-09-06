use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "ICFPC 2025 Flowlight Solver")]
#[command(about = "Flowlight's personal solver (independent crate)", long_about = None)]
struct Args {
    /// Problem name (free-form for now)
    #[arg(short, long, default_value = "probatio")]
    problem: String,

    /// Optional verbosity flag
    #[arg(short, long, default_value_t = 0)]
    verbose: u8,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    println!("=== ICFPC 2025 My Solver ===");
    println!("Problem: {}", args.problem);
    println!("Verbose: {}", args.verbose);

    // TODO: Implement your own exploration/solving logic here.
    // This crate is intentionally independent from other solvers.

    Ok(())
}
