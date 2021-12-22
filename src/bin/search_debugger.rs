use std::sync::atomic::AtomicBool;

use anyhow::Result;
use clap::Parser;
use crossbeam_channel::unbounded;
use pewter::{engine::SearchControls, io::fen::parse_fen, Engine};

/// Run a single best_move search, without any UCI server logic
#[derive(Parser, Debug)]
#[clap(about, version, author, name="search_debugger")]
struct Args {
    /// Fen string to start search at
    #[clap(long)]
    fen: String,

    /// Depth to search to
    #[clap(long)]
    depth: Option<u8>,
}

fn main() -> Result<()> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%Y-%m-%dT%H:%M:%S]"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Debug)
        .chain(std::io::stderr())
        .apply()?;

    let args = Args::parse();

    let initial_state = parse_fen(&args.fen)?;

    println!("Initial board state:");
    println!("{}", initial_state.pretty_format());

    let mut engine = Engine::new();
    engine.set_board_state(initial_state);

    let (perf_tx, perf_rx) = unbounded();
    
    let perf_logging_thread = std::thread::spawn(|| {
        for msg in perf_rx {
            log::debug!("{:?}", msg)
        }
    });

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
    
    perf_logging_thread.join().unwrap();
    log::debug!("Search returned best move = {}", best_move);

    Ok(())
}
