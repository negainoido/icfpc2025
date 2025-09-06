use anyhow::Result;
use clap::Parser;

const N: usize = 12;
const PLAN: &str = "544054004012505045403132415045121344434123550012251105504250114353144151425322121105300020103405512351411245432153525343350045401333125514304052100010425231512023352345105152105305102520145332443052344120054345522511";
const RESULT: [i32; 217] = [
    0, 1, 1, 1, 2, 1, 3, 2, 2, 0, 0, 3, 3, 0, 2, 1, 2, 0, 1, 1, 1, 0, 0, 2, 0, 0, 3, 3, 2, 3, 3, 0,
    3, 1, 0, 3, 0, 3, 3, 3, 1, 3, 1, 1, 3, 2, 2, 2, 1, 3, 3, 1, 3, 1, 1, 3, 2, 3, 1, 3, 2, 0, 0, 0,
    0, 2, 2, 2, 3, 2, 2, 2, 2, 0, 2, 1, 2, 1, 3, 0, 3, 1, 1, 1, 0, 0, 0, 2, 0, 0, 3, 1, 2, 3, 2, 0,
    1, 0, 3, 3, 3, 3, 1, 3, 1, 3, 3, 0, 1, 1, 1, 1, 1, 0, 0, 2, 3, 0, 0, 0, 0, 2, 1, 2, 0, 3, 0, 0,
    0, 3, 1, 2, 2, 2, 1, 1, 3, 1, 3, 3, 1, 3, 2, 1, 3, 0, 2, 0, 3, 1, 2, 0, 3, 0, 3, 3, 0, 1, 1, 1,
    1, 3, 3, 1, 1, 1, 1, 1, 1, 1, 2, 1, 1, 3, 1, 1, 2, 1, 0, 3, 0, 3, 1, 3, 3, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 3, 3, 1, 1, 1, 1, 1, 3, 1, 3, 1, 2, 1, 3, 3, 3, 3, 3, 1, 1, 3, 1, 3,
];

/// Convert a plan string like "0325" or "123" into Vec<usize>.
/// - Accepts only digits 0..5, or only digits 1..6 (normalized to 0..5).
/// - Returns an error on invalid or mixed digits.
fn parse_plan(plan: &str) -> anyhow::Result<Vec<usize>> {
    if plan.is_empty() {
        return Ok(Vec::new());
    }

    let is_0_5 = plan
        .chars()
        .all(|c| matches!(c, '0' | '1' | '2' | '3' | '4' | '5'));
    let is_1_6 = plan
        .chars()
        .all(|c| matches!(c, '1' | '2' | '3' | '4' | '5' | '6'));

    if is_0_5 {
        return Ok(plan.chars().map(|c| (c as u8 - b'0') as usize).collect());
    }
    if is_1_6 {
        return Ok(plan.chars().map(|c| (c as u8 - b'1') as usize).collect());
    }

    Err(anyhow::anyhow!(
        "Plan contains invalid digits (expect only 0-5 or only 1-6): {}",
        plan
    ))
}

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

    // Convert constant PLAN to Vec<usize>
    let plan_vec = parse_plan(PLAN)?;
    if args.verbose > 0 {
        println!("PLAN length: {}", plan_vec.len());
    }

    // TODO: Implement your own exploration/solving logic here.
    // This crate is intentionally independent from other solvers.

    Ok(())
}
