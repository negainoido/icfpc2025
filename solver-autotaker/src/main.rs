use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Read, Write};

#[derive(Parser, Debug)]
#[command(name = "solver-autotaker")]
#[command(about = "ICFPC 2025 solver skeleton by autotaker", long_about = None)]
struct Args {
    /// Verbose output level
    #[arg(short, long, default_value_t = 0)]
    verbose: u8,

    /// Optional random seed
    #[arg(long)]
    seed: Option<u64>,

    /// Input JSON path ("-" for stdin)
    #[arg(short, long, default_value = "-")]
    input: String,

    /// Output JSON path ("-" for stdout)
    #[arg(short, long, default_value = "-")]
    output: String,
}

#[derive(Debug, Deserialize)]
struct InputProblem {
    plans: Vec<String>,
    results: Vec<Vec<u8>>, // each length should be plan.len()+1
    #[serde(rename = "N")]
    n: usize,
    #[serde(rename = "startingRoom")]
    starting_room: usize,
}

#[derive(Debug, Serialize)]
struct PortRef {
    room: usize,
    door: usize,
}

#[derive(Debug, Serialize)]
struct Connection {
    from: PortRef,
    to: PortRef,
}

#[derive(Debug, Serialize)]
struct OutputMap {
    rooms: Vec<u8>,
    #[serde(rename = "startingRoom")]
    starting_room: usize,
    connections: Vec<Connection>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    if args.verbose > 0 {
        eprintln!("solver-autotaker: starting up (seed={:?})", args.seed);
    }
    // Read input JSON
    let mut input_json = String::new();
    if args.input == "-" {
        io::stdin().read_to_string(&mut input_json)?;
    } else {
        input_json = fs::read_to_string(&args.input)
            .with_context(|| format!("failed to read input file: {}", &args.input))?;
    }

    let problem: InputProblem =
        serde_json::from_str(&input_json).context("failed to parse input JSON")?;

    // Build a trivial self-loop mapping as a placeholder.
    // Per request: for 0 <= room < 6 and 0 <= door < 5
    let room_count = problem.n.min(6);
    let door_max_exclusive = 5usize; // doors 0..4

    let mut connections = Vec::new();
    for room in 0..room_count {
        for door in 0..door_max_exclusive {
            connections.push(Connection {
                from: PortRef { room, door },
                to: PortRef { room, door },
            });
        }
    }

    // Placeholder rooms labels: zeros of length N
    let rooms = vec![0u8; problem.n];
    let out = OutputMap {
        rooms: rooms,
        starting_room: problem.starting_room,
        connections,
    };

    let serialized = serde_json::to_string_pretty(&out)?;

    if args.output == "-" {
        let mut stdout = io::stdout().lock();
        stdout.write_all(serialized.as_bytes())?;
        stdout.write_all(b"\n")?;
    } else {
        fs::write(&args.output, serialized)
            .with_context(|| format!("failed to write output file: {}", &args.output))?;
    }

    Ok(())
}
