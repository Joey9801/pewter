use std::time::Duration;

use anyhow::Result;
use crossbeam_channel::{select, Sender};

use pewter::engine::{Engine, EngineCommand, EngineMessage};
use pewter::io::uci::*;

#[derive(Clone, Debug, Default)]
struct Options {
    debug: bool,
}

// TODO: implementing this trait might be better handled by a macro
impl UciOptions for Options {
    type SetOptionError = ();

    fn all_options() -> Vec<OptionMessage> {
        vec![OptionMessage {
            option_name: "debug".to_string(),
            option_type: OptionType::Check,
            default: Some("off".to_string()),
            min: None,
            max: None,
            combo_options: None,
        }]
    }

    fn set_value(&mut self, option_name: &str, value: &str) -> Result<(), Self::SetOptionError> {
        match option_name {
            "debug" => match value {
                "on" => self.debug = true,
                "off" => self.debug = false,
                _ => Err(())?,
            },
            _ => Err(())?,
        }

        Ok(())
    }
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
        .chain(fern::log_file("pewter.log")?)
        .apply()?;

    let uci = UciInterface::<Options>::startup()?;
    let (engine_tx, engine_rx) = Engine::startup()?;

    loop {
        select! {
            recv(uci.rx) -> uci_msg => if handle_uci_cmd(uci_msg?, &uci.tx, &engine_tx)? {
                break Ok(());
            },
            recv(engine_rx) -> engine_msg => handle_engine_msg(engine_msg?, &uci.tx)?,
        }
    }
}


fn handle_uci_cmd(msg: UciCommand, uci_tx: &Sender<UciMessage>, engine_tx: &Sender<EngineCommand>) -> Result<bool> {
    match msg {
        UciCommand::Uci => uci_tx.send(UciMessage::UciOk)?,
        UciCommand::IsReady => uci_tx.send(UciMessage::ReadyOk)?,
        UciCommand::Quit => {
            log::info!("Received quit command, shutting down");
            return Ok(true);
        }
        UciCommand::Position { position, moves } => {
            let fen = match &position {
                Position::StartPos => {
                    "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
                }
                Position::FenString(s) => s,
            };
            let mut state = pewter::io::fen::parse_fen(fen)?;
            for m in moves {
                state = state.apply_move(m);
            }

            log::info!("Setting position to \"{}\"", pewter::io::fen::format_fen(&state));
            engine_tx.send(EngineCommand::SetState(state))?;
        }
        UciCommand::Go(go) => {
            let timings = pewter::engine::Timings {
                white_remaining: go.white_time,
                black_remaining: go.black_time,
                white_increment: go.white_increment.unwrap_or(Duration::ZERO),
                black_increment: go.black_increment.unwrap_or(Duration::ZERO),
            };
            
            engine_tx.send(EngineCommand::UpdateTimings(timings))?;
            
            let infinite = go.infinite;
            let max_depth = go.depth.map(|x| x as u8);
            let max_nodes = go.nodes;
            engine_tx.send(EngineCommand::BeginSearch {
                infinite, max_depth, max_nodes
            })?;
        }
        _ => (),
    }
    
    Ok(false)
}

fn handle_engine_msg(msg: EngineMessage, uci_tx: &Sender<UciMessage>) -> Result<()> {
    match msg {
        EngineMessage::PerfInfo {
            transposition_load,
            nodes,
            nodes_per_second: _,
            table_hits: _,
            shredder_hits: _
        } => uci_tx.send(UciMessage::Info(InfoMessage {
            nodes: Some(nodes),
            hash_full: Some((transposition_load * 100_000f32) as u16),
            ..InfoMessage::default()
        }))?,
        EngineMessage::BestMove { best_move, ponder_move } => uci_tx.send(
            UciMessage::BestMove { best_move, ponder_move }
        )?,
        EngineMessage::Error(e) => log::warn!("{}", e),
    }
    
    Ok(())
}