use std::io::Write;
use std::{fs::File, path::PathBuf, sync::atomic::AtomicBool};

use anyhow::Result;
use clap::Parser;
use crossbeam_channel::unbounded;
use pewter_core::{io::fen::parse_fen};
use pewter_engine::{engine::search::SearchControls, Engine};

/// Run a single best_move search, without any UCI server logic
#[derive(Parser, Debug)]
#[clap(about, version, author, name = "search_debugger")]
struct Args {
    /// Fen string to start search at
    #[clap(long)]
    fen: String,

    /// Depth to search to
    #[clap(long)]
    depth: Option<u8>,

    #[clap(long)]
    node_count_histogram: bool,

    #[clap(long)]
    histogram_output_file: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.node_count_histogram {
        nodes_searched_histogram(&args)?;
    } else {
        single_search(&args)?;
    }

    Ok(())
}

fn single_search(args: &Args) -> Result<()> {
    let initial_state = parse_fen(&args.fen)?;

    println!("Initial board state:");
    println!("{}", initial_state.pretty_format());

    let mut engine = Engine::new();
    engine.set_board_state(initial_state);

    let (perf_tx, perf_rx) = unbounded();
    let max_depth = args.depth.or(Some(5));
    let best_move = engine.search_best_move(
        false,
        max_depth,
        None,
        None,
        SearchControls {
            stop: AtomicBool::new(false).into(),
            perf_info: Some(perf_tx),
        },
    )?;

    println!("Search returned best move = {}", best_move);

    let last_perf = perf_rx.into_iter().last().unwrap();
    dbg!(last_perf);

    Ok(())
}

fn nodes_searched_histogram(args: &Args) -> Result<()> {
    let initial_state = parse_fen(&args.fen)?;

    println!("Initial board state:");
    println!("{}", initial_state.pretty_format());

    let mut counts = Vec::new();

    for _ in 0..1000 {
        let mut engine = Engine::new();
        engine.set_board_state(initial_state);
        let (perf_tx, perf_rx) = unbounded();
        engine.search_best_move(
            false,
            Some(
                args.depth
                    .expect("--depth must be set for a node search histogram"),
            ),
            None,
            None,
            SearchControls {
                stop: AtomicBool::new(false).into(),
                perf_info: Some(perf_tx),
            },
        )?;

        let last_info = perf_rx
            .into_iter()
            .last()
            .expect("Expected at least one performance update from search");

        counts.push(last_info.nodes);
    }

    let mut file = File::create(args.histogram_output_file.as_ref().unwrap())?;
    for c in counts {
        writeln!(&mut file, "{}", c)?;
    }

    Ok(())
}
