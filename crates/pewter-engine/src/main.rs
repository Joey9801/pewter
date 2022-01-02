use std::time::Duration;

use anyhow::Result;
use crossbeam_channel::{select, Sender};

use pewter_engine::engine::engine_server::EngineServer;
use pewter_engine::engine::PerfInfo;
use pewter_core::{Move, io::uci::*};

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
    let mut engine = EngineServer::startup()?;

    loop {
        select! {
            recv(uci.rx) -> uci_msg => if handle_uci_cmd(uci_msg?, &uci.tx, &mut engine)? {
                break Ok(());
            },
            recv(engine.perf_rx) -> perf => handle_engine_perf(perf?, &uci.tx)?,
            recv(engine.best_move_rx) -> m => handle_engine_best_move(m?, &uci.tx)?,
        }
    }
}

fn handle_uci_cmd(
    msg: UciCommand,
    uci_tx: &Sender<UciMessage>,
    engine: &mut EngineServer,
) -> Result<bool> {
    match msg {
        UciCommand::Uci => {
            uci_tx.send(UciMessage::UciOk)?;
            uci_tx.send(UciMessage::Id(EngineId::Name("pewter".to_string())))?;
            uci_tx.send(UciMessage::Id(EngineId::Author("Joe Roberts".to_string())))?;
        }
        UciCommand::IsReady => uci_tx.send(UciMessage::ReadyOk)?,
        UciCommand::Quit => {
            log::info!("Received quit command, shutting down");
            return Ok(true);
        }
        UciCommand::Position { position, moves } => {
            // Parse the position, and resolve any moves passed in
            let fen = match &position {
                Position::StartPos => "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                Position::FenString(s) => s,
            };
            let mut state = pewter_core::io::fen::parse_fen(fen)?;
            for m in moves {
                state = state.apply_move(m);
            }

            log::info!(
                "Setting position to \"{}\"",
                pewter_core::io::fen::format_fen(&state)
            );
            engine.set_state(state)?;
        }
        UciCommand::Go(go) => {
            let timings = pewter_engine::engine::Timings {
                white_remaining: go.white_time,
                black_remaining: go.black_time,
                white_increment: go.white_increment.unwrap_or(Duration::ZERO),
                black_increment: go.black_increment.unwrap_or(Duration::ZERO),
            };

            engine.begin_search(go.infinite, go.depth, go.nodes, Some(timings))?;
        }
        UciCommand::Stop => engine.stop_search()?,
        _ => (),
    }

    Ok(false)
}

fn handle_engine_perf(msg: PerfInfo, uci_tx: &Sender<UciMessage>) -> Result<()> {
    uci_tx.send(UciMessage::Info(InfoMessage {
        nodes: Some(msg.nodes),
        nodes_per_second: Some(msg.nodes_per_second as u64),
        hash_full: Some((msg.transposition_load * 100_000f32) as u16),
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
