use std::sync::RwLock;
use std::time::Duration;

use anyhow::Result;
use crossbeam_channel::{select, Sender};

use pewter_core::{io::uci::*, Move};
use pewter_engine::engine::engine_server::EngineServer;
use pewter_engine::engine::eval::{self, Evaluation};
use pewter_engine::engine::transposition::DEFAULT_HASH_MB;
use pewter_engine::engine::PerfInfo;
use tracing_subscriber::prelude::*;

#[derive(Clone, Debug)]
struct Options {
    debug: bool,

    /// Requested transposition table size, in mebibytes.
    hash_mb: usize,

    /// Root-move randomisation margin, in centipawns (0 disables it).
    wobble_cp: i32,

    /// Number of opening plies over which to apply the wobble.
    wobble_plies: u8,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            debug: false,
            hash_mb: DEFAULT_HASH_MB,
            wobble_cp: 0,
            wobble_plies: 0,
        }
    }
}

// TODO: implementing this trait might be better handled by a macro
impl UciOptions for Options {
    type SetOptionError = ();

    fn all_options() -> Vec<OptionMessage> {
        vec![
            OptionMessage {
                option_name: "debug".to_string(),
                option_type: OptionType::Check,
                default: Some("off".to_string()),
                min: None,
                max: None,
                combo_options: None,
            },
            OptionMessage {
                option_name: "Hash".to_string(),
                option_type: OptionType::Spin,
                default: Some(DEFAULT_HASH_MB.to_string()),
                min: Some(1),
                max: Some(4096),
                combo_options: None,
            },
            OptionMessage {
                option_name: "Wobble".to_string(),
                option_type: OptionType::Spin,
                default: Some("0".to_string()),
                min: Some(0),
                max: Some(1000),
                combo_options: None,
            },
            OptionMessage {
                option_name: "WobblePlies".to_string(),
                option_type: OptionType::Spin,
                default: Some("0".to_string()),
                min: Some(0),
                max: Some(40),
                combo_options: None,
            },
        ]
    }

    fn set_value(&mut self, option_name: &str, value: &str) -> Result<(), Self::SetOptionError> {
        match option_name {
            "debug" => match value {
                "on" => self.debug = true,
                "off" => self.debug = false,
                _ => Err(())?,
            },
            "Hash" => {
                let mb = value.parse::<usize>().map_err(|_| ())?;
                self.hash_mb = mb.clamp(1, 4096);
            }
            "Wobble" => {
                let cp = value.parse::<i32>().map_err(|_| ())?;
                self.wobble_cp = cp.clamp(0, 1000);
            }
            "WobblePlies" => {
                let plies = value.parse::<i32>().map_err(|_| ())?;
                self.wobble_plies = plies.clamp(0, 40) as u8;
            }
            _ => Err(())?,
        }

        Ok(())
    }
}

fn main() -> Result<()> {
    let file = tracing_appender::rolling::hourly("./logs", "pewter.log");
    let file_layer = tracing_subscriber::fmt::layer().with_writer(file);

    tracing_subscriber::registry()
        .with(file_layer)
        .init();

    tracing::info!("Starting up pewter-engine");

    let uci = UciInterface::<Options>::startup()?;
    let mut engine = EngineServer::startup()?;

    let res = move || -> Result<()> {
        loop {
            select! {
                recv(uci.rx) -> uci_msg => if handle_uci_cmd(uci_msg?, &uci.tx, &uci.opts, &mut engine)? {
                    break Ok(());
                },
                recv(engine.perf_rx) -> perf => handle_engine_perf(perf?, &uci.tx)?,
                recv(engine.best_move_rx) -> m => handle_engine_best_move(m?, &uci.tx)?,
            }
        }
    }();

    tracing::info!(?res, "Shutting down pewter-engine");

    res
}

fn handle_uci_cmd(
    msg: UciCommand,
    uci_tx: &Sender<UciMessage>,
    opts: &RwLock<Options>,
    engine: &mut EngineServer,
) -> Result<bool> {
    match msg {
        UciCommand::Uci => {
            uci_tx.send(UciMessage::Id(EngineId::Name("pewter".to_string())))?;
            uci_tx.send(UciMessage::Id(EngineId::Author("Joe Roberts".to_string())))?;
            for option in Options::all_options() {
                uci_tx.send(UciMessage::Option(option))?;
            }
            uci_tx.send(UciMessage::UciOk)?;
        }
        UciCommand::IsReady => uci_tx.send(UciMessage::ReadyOk)?,
        UciCommand::Quit => {
            tracing::info!("Received quit command, shutting down");
            return Ok(true);
        }
        UciCommand::SetOption { option_name, value } => {
            if let Some(value) = value {
                let mut opts = opts.write().unwrap();
                if opts.set_value(&option_name, &value).is_ok() {
                    match option_name.as_str() {
                        "Hash" => engine.set_hash_size(opts.hash_mb)?,
                        "Wobble" | "WobblePlies" => {
                            engine.set_wobble(opts.wobble_cp, opts.wobble_plies)?
                        }
                        _ => (),
                    }
                }
            }
        }
        UciCommand::UciNewGame => engine.new_game()?,
        UciCommand::Position { position, moves } => {
            // Parse the position, and resolve any moves passed in, recording the
            // Zobrist hash of every position along the way for repetition
            // detection during search.
            let fen = match &position {
                Position::StartPos => "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                Position::FenString(s) => s,
            };
            let mut state = pewter_core::io::fen::parse_fen(fen)?;
            let mut history = vec![state.zobrist];
            for m in moves {
                state = state.apply_move(m);
                history.push(state.zobrist);
            }

            tracing::info!(
                "Setting position to \"{}\"",
                pewter_core::io::fen::format_fen(&state)
            );
            engine.set_state(state, history)?;
        }
        UciCommand::Go(go) => {
            let timings = pewter_engine::engine::Timings {
                white_remaining: go.white_time,
                black_remaining: go.black_time,
                white_increment: go.white_increment.unwrap_or(Duration::ZERO),
                black_increment: go.black_increment.unwrap_or(Duration::ZERO),
                move_time: go.move_time,
            };

            engine.begin_search(go.infinite, go.depth, go.nodes, Some(timings))?;
        }
        UciCommand::Stop => engine.stop_search()?,
        _ => (),
    }

    Ok(false)
}

/// Convert an internal engine score into the UCI reporting form, expressing
/// forced mates as a move distance where possible.
fn info_score(score: Evaluation) -> InfoScore {
    match eval::mate_in_moves(score) {
        Some(moves) => InfoScore {
            // GUIs read the `mate` field for mate scores; centipawns is ignored.
            centipawns: 0,
            mate: Some(moves.unsigned_abs() as u16),
            lowerbound: false,
            upperbound: false,
        },
        None => InfoScore {
            centipawns: score.clamp(0, u16::MAX as Evaluation) as u16,
            mate: None,
            lowerbound: false,
            upperbound: false,
        },
    }
}

fn handle_engine_perf(msg: PerfInfo, uci_tx: &Sender<UciMessage>) -> Result<()> {
    uci_tx.send(UciMessage::Info(InfoMessage {
        depth: msg.depth.map(|d| d as u16),
        nodes: Some(msg.nodes),
        nodes_per_second: Some(msg.nodes_per_second as u64),
        // `hash_full` is reported in per-mille (parts per thousand).
        hash_full: Some((msg.transposition_load * 1000.0) as u16),
        score: msg.score.map(info_score),
        ..InfoMessage::default()
    }))?;

    Ok(())
}

fn handle_engine_best_move(best_move: Move, uci_tx: &Sender<UciMessage>) -> Result<()> {
    uci_tx.send(UciMessage::BestMove {
        best_move,
        ponder_move: None,
    })?;

    Ok(())
}
