use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use anyhow::Result;
use crossbeam_channel::{unbounded, Receiver, Sender};

use pewter_core::{Move, State};

use super::{PerfInfo, SearchControls, Timings, EngineError};

#[derive(Clone, Copy, Debug)]
struct BeginSearchArgs {
    /// Ignore all time controls, and compute until the Stop command is received
    infinite: bool,

    /// Automatically stop after searching this deep
    max_depth: Option<u8>,

    /// Automatically stop after searching this many nodes
    max_nodes: Option<u64>,

    /// The most up-to-date time control information for this search
    ///
    /// Completely ignored if an infinite search is requested
    timings: Option<Timings>,
}

/// Used internally in the engine server to give instructions to the main engine thread
#[derive(Clone, Debug)]
enum EngineCommand {
    SetState(State),
    BeginSearch(BeginSearchArgs),
    Exit,
}

pub struct EngineServer {
    cmd_tx: Sender<EngineCommand>,
    search_stopper: Arc<AtomicBool>,
    pub perf_rx: Receiver<PerfInfo>,
    pub best_move_rx: Receiver<Move>,
}

impl EngineServer {
    pub fn startup() -> Result<Self> {
        let (cmd_tx, cmd_rx) = unbounded();
        let (perf_tx, perf_rx) = unbounded();
        let (best_move_tx, best_move_rx) = unbounded();

        let search_stopper = Arc::new(AtomicBool::new(false));
        let search_stopper_clone = search_stopper.clone();

        std::thread::Builder::new()
            .name("EngineServer main".to_string())
            .spawn(|| engine_main_thread(cmd_rx, perf_tx, best_move_tx, search_stopper_clone))?;

        Ok(Self {
            cmd_tx,
            perf_rx,
            best_move_rx,
            search_stopper,
        })
    }

    pub fn set_state(&mut self, new_state: State) -> Result<()> {
        self.cmd_tx.send(EngineCommand::SetState(new_state))?;
        Ok(())
    }

    pub fn begin_search(
        &mut self,
        infinite: bool,
        max_depth: Option<u8>,
        max_nodes: Option<u64>,
        timings: Option<Timings>,
    ) -> Result<()> {
        let args = BeginSearchArgs {
            infinite,
            max_depth,
            max_nodes,
            timings,
        };

        self.search_stopper.store(false, Ordering::Relaxed);

        self.cmd_tx.send(EngineCommand::BeginSearch(args))?;

        Ok(())
    }

    pub fn stop_search(&mut self) -> Result<()> {
        self.search_stopper.store(true, Ordering::Relaxed);
        Ok(())
    }
}

impl Drop for EngineServer {
    fn drop(&mut self) {
        self.search_stopper.store(true, Ordering::Relaxed);
        let _ = self.cmd_tx.send(EngineCommand::Exit);
    }
}

fn engine_main_thread(
    cmd_rx: Receiver<EngineCommand>,
    perf_tx: Sender<PerfInfo>,
    best_move_tx: Sender<Move>,
    search_stopper: Arc<AtomicBool>,
) -> Result<()> {
    let r = engine_main_thread_inner(cmd_rx, perf_tx, best_move_tx, search_stopper);

    if let Err(e) = r.as_ref() {
        log::error!("Engine main thread exiting because: {:?}", e);
    } else {
        log::info!("Engine main thread exiting");
    }
    
    r
}

fn engine_main_thread_inner(
    cmd_rx: Receiver<EngineCommand>,
    perf_tx: Sender<PerfInfo>,
    best_move_tx: Sender<Move>,
    search_stopper: Arc<AtomicBool>,
) -> Result<()> {
    let mut engine = super::Engine::new();

    // TODO: Options/configuration system rather than hardcoded DB path
    let db_path = Path::new("./opening_db.dat");
    let workdir = std::env::current_dir().unwrap();
    let workdir = workdir.to_string_lossy();
    if db_path.exists() {
        log::debug!("Loading opening db from {} in {}", db_path.to_string_lossy(), workdir);
        engine.load_opening_db(Path::new("./opening_db.dat"))?;
    } else{
        log::debug!("Couldn't find opening db at {} in {}, proceeding without", db_path.to_string_lossy(), workdir);
    }

    for cmd in cmd_rx {
        match cmd {
            EngineCommand::SetState(state) => engine.set_board_state(state),
            EngineCommand::BeginSearch(args) => {
                let controls = SearchControls {
                    stop: search_stopper.clone(),
                    perf_info: Some(perf_tx.clone()),
                };

                let best_move = match engine.search_best_move(
                    args.infinite,
                    args.max_depth,
                    args.max_nodes,
                    args.timings,
                    controls,
                ) {
                    Ok(m) => m,
                    Err(EngineError::EarlyStop) => {
                        log::warn!("Search stop was requested before the first move was found");
                        continue;
                    },
                    other => other?,
                };

                best_move_tx.send(best_move)?;
            }
            EngineCommand::Exit => break,
        }
    }

    Ok(())
}
