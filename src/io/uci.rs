//! (De)Serialization for UCI messages

use anyhow::Result;
use crossbeam_channel::{select, unbounded, Receiver, Sender};
use std::fmt::Write;
use std::io::BufRead;
use std::sync::RwLock;
use std::time::Duration;

use crate::Move;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Position {
    /// Abbreviation for the conventional starting position
    ///
    /// Equivalent to
    /// `Position::FenString("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")`
    StartPos,

    /// Some arbitrary FEN string
    FenString(String),
}

/// The payload for EngineCommand::Go
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GoCommand {
    /// Restrict search to lines that start with this sequence of moves
    pub search_moves: Option<Vec<Move>>,

    /// Start searching in pondering mode.
    ///
    /// Do not exit the search in ponder mode, even if it's mate!  This means that the last move
    /// sent in in the position string is the ponder move.  The engine can do what it wants to do,
    /// but after a "ponderhit" command it should execute the suggested move to ponder on. This
    /// means that the ponder move sent by the GUI can be interpreted as a recommendation about
    /// which move to ponder. However, if the engine decides to ponder on a different move, it
    /// should not display any mainlines as they are likely to be misinterpreted by the GUI because
    /// the GUI expects the engine to ponder
    pub ponder: bool,

    /// White has the given amount of time left on the clock
    pub white_time: Option<Duration>,

    /// White increment per move
    pub white_increment: Option<Duration>,

    /// Black has the given amount of time left on the clock
    pub black_time: Option<Duration>,

    /// Black increment per move
    pub black_increment: Option<Duration>,

    /// There are the given number of moves left until the next time control
    pub moves_to_go: Option<u16>,

    /// Only search to a depth this many plies
    pub depth: Option<u8>,

    /// Only search this many nodes
    pub nodes: Option<u64>,

    /// Search for mate in this many moves
    pub mate: Option<u8>,

    /// Spend exactly this long searching
    pub move_time: Option<Duration>,

    /// Search until receiving the "stop" command
    pub infinite: bool,
}

/// The commands that the engine may recieve from the interface
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UciCommand {
    /// Tells the engine to use UCI
    ///
    /// This will be sent once as a first command after program boot to tell the engine to switch
    /// to uci mode.  After receiving the uci command the engine must identify itself with the "id"
    /// command and send the "option" commands to tell the GUI which engine settings the engine
    /// supports if any.  After that the engine should send "uciok" to acknowledge the uci mode.
    /// If no uciok is sent within a certain time period, the engine task will be killed by the
    /// GUI.
    Uci,

    /// Switch the debug mode of the engine on and off.
    ///
    /// In debug mode the engine should send additional infos to the GUI, e.g. with the "info
    /// string" command, to help debugging, e.g. the commands that the engine has received etc.
    /// This mode should be switched off by default and this command can be sent any time, also
    /// when the engine is thinking.
    Debug(bool),

    /// This is used to synchronize the engine with the GUI.
    ///
    /// When the GUI has sent a command or multiple commands that can take some time to complete,
    /// this command can be used to wait for the engine to be ready again or to ping the engine to
    /// find out if it is still alive.  E.g. this should be sent after setting the path to the
    /// tablebases as this can take some time.  This command is also required once before the
    /// engine is asked to do any search to wait for the engine to finish initializing.  This
    /// command must always be answered with "readyok" and can be sent also when the engine is
    /// calculating in which case the engine should also immediately answer with "readyok" without
    /// stopping the search.
    IsReady,

    /// This is sent to the engine when the user wants to change the internal parameters
    /// of the engine.
    ///
    /// For the "button" type no value is needed.  One string will be sent for each parameter and
    /// this will only be sent when the engine is waiting.  The name and value of the option in
    /// <id> should not be case sensitive and can inlude spaces.  The substrings "value" and "name"
    /// should be avoided in <id> and <x> to allow unambiguous parsing, for example do not use
    /// <name> = "draw value".
    SetOption {
        option_name: String,
        value: Option<String>,
    },

    /// This is the command to try to register an engine or to tell the engine that registration
    /// will be done later.
    ///
    /// This command should always be sent if the engine has sent "registration error" at program
    /// startup.
    Register {
        name: Option<String>,
        code: Option<String>,
    },

    /// This is sent to the engine when the next search (started with "position" and "go") will be
    /// from a different game.
    ///
    /// This can be a new game the engine should play or a new game it should analyse but also the
    /// next position from a testsuite with positions only.
    ///
    /// If the GUI hasn't sent a "ucinewgame" before the first "position" command, the engine
    /// shouldn't expect any further ucinewgame commands as the GUI is probably not supporting the
    /// ucinewgame command.  So the engine should not rely on this command even though all new GUIs
    /// should support it.  As the engine's reaction to "ucinewgame" can take some time the GUI
    /// should always send "isready" after "ucinewgame" to wait for the engine to finish its
    /// operation.
    UciNewGame,

    /// Set up the position described in fenstring on the internal board and play the moves on the
    /// internal chess board.
    ///
    /// If the game was played from the start position the string "startpos" will be sent.
    /// Note: no "new" command is needed. However, if this position is from a different game than
    /// the last position sent to the engine, the GUI should have sent a "ucinewgame" inbetween.
    Position {
        position: Position,
        moves: Vec<Move>,
    },

    /// Start calculating on the current position set up with the "position" command.
    ///
    /// There are a number of commands that can follow this command, all will be sent in the same
    /// string.  If one command is not sent its value should be interpreted as it would not
    /// influence the search.
    Go(GoCommand),

    /// Stop calculating as soon as possible
    Stop,

    /// The user has played the expected move.
    ///
    /// This will be sent if the engine was told to ponder on the same move the user has played.
    /// The engine should continue searching but switch from pondering to normal search.
    PonderHit,

    /// Quit the program as soon as possible
    Quit,
}

/// The payload for EngineMesssage::Id
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EngineId {
    Name(String),
    Author(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CopyProtectionMessage {
    Checking,
    Ok,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistrationMessage {
    Checking,
    Ok,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InfoScore {
    /// The score from the engine's point of view in centipawns
    pub centipawns: u16,

    /// Found mate in this many moves (NB moves not plies).
    pub mate: Option<u16>,

    /// The given score is just a lower bound
    pub lowerbound: bool,

    /// The given score is just an upper bound
    pub upperbound: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InfoRefutation {
    /// The move that is being refuted
    pub refuted_move: Move,

    /// The line that refutes the given move.
    ///
    /// NB this may be empty, to indicate that no refutations have been found.
    pub refutation_line: Vec<Move>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InfoCurrLine {
    /// If different CPU cores are calculating different lines, the index of the CPU core that this
    /// message refers to.
    pub cpu_number: Option<u16>,

    /// The line that is currently being considered
    pub line: Vec<Move>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InfoMessage {
    /// Search depth in plies.
    pub depth: Option<u16>,

    /// Selective depth in plies.
    ///
    /// If the engine sends this, it must also include `depth` in the same message.
    pub selective_depth: Option<u16>,

    /// The time spent searching.
    pub time: Option<Duration>,

    /// The number of nodes searched.
    ///
    /// The engine should send this regularly.
    pub nodes: Option<u64>,

    /// In MultiPV mode, the index of the pv being sent in this message
    pub multipv: Option<u16>,

    /// The best line found, or the i'th line found in MultiPV mode
    pub principal_variation: Option<Vec<Move>>,

    /// The score of the current position from the engine's point of view.
    pub score: Option<InfoScore>,

    /// Currently searching this move
    pub curr_move: Option<Move>,

    /// Currently searching this move number.
    ///
    /// Should be 1-indexed
    pub curr_move_number: Option<u16>,

    /// The hash is milli-percent full
    pub hash_full: Option<u16>,

    /// Currently searching at this many nodes per second
    pub nodes_per_second: Option<u64>,

    /// This many positions found in the endgame tablebases
    pub table_hits: Option<u64>,

    /// This many positions found in teh shredder endgame databases
    pub shredder_hits: Option<u64>,

    /// Cpu usage of the engine in milli-percent
    pub cpu_load: Option<u16>,

    /// An arbitrary string to be displayed in the GUI
    pub string: Option<String>,

    /// Found a refutation for some move
    pub refutation: Option<InfoRefutation>,

    /// The current line the engine is calculating
    pub current_line: Option<InfoCurrLine>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OptionType {
    /// A checkbox that can be either "true" or "false"
    Check,

    /// An integer in a certain range
    Spin,

    /// One of a given set of strings
    Combo,

    /// A button that can be pressed to send a command to the engine
    Button,

    /// A text field that has a string as a value
    String,
}

/// This command tells the GUI which parameters can be changed in the engine.
///
/// This should be sent once for each option at engine startup after the "uci" and the "id"
/// commands if any parameter can be changed in the engine. The GUI should parse this and build
/// a dialog for the user to change the settings.
///
/// If the user wants to change some settings, the GUI will send a "setoption" command to the
/// engine.
///
/// Note that not every option needs to appear in this dialog. Some options like "Ponder",
/// "UCI_AnalyseMode", etc. are better handled elsewhere or are set automatically.
/// Note that the GUI need not send the setoption command when starting the engine for every
/// option if it doesn't want to change the default value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptionMessage {
    pub option_name: String,

    /// Which type the option is (eg number in range, checkbox, etc..)
    pub option_type: OptionType,

    /// The value that the engine will use for this option if no matching
    /// EngineCommand::SetOption is received.
    pub default: Option<String>,

    /// For integer range options, the minimum supported values (inclusive).
    pub min: Option<i32>,

    /// For integer range options, the maximum supported values (inclusive).
    pub max: Option<i32>,

    /// For OptionType::Combo, the valid strings
    pub combo_options: Option<Vec<String>>,
}

/// The messages that the engine may send to the interface
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UciMessage {
    /// Must be sent after receiving the "uci" command to identify the engine,
    Id(EngineId),

    /// Must be sent after the id and optional options to tell the GUI that the engine has sent all
    /// infos and is ready in uci mode.
    UciOk,

    /// This must be sent when the engine has received an "isready" command and has processed all
    /// input and is ready to accept new commands now.
    ///
    /// It is usually sent after a command that can take some time to be able to wait for the
    /// engine, but it can be used anytime, even when the engine is searching, and must always be
    /// answered with "isready".
    ReadyOk,

    /// The engine has stopped searching and found `best_move` to be best in this position.
    ///
    /// The engine can send the move it likes to ponder on. The engine must not start pondering
    /// automatically.
    /// This command must always be sent if the engine stops searching, also in pondering mode if
    /// there is a "stop" command, so for every "go" command a "bestmove" command is needed!
    /// Directly before that the engine should send a final "info" command with the final search
    /// information, the the GUI has the complete statistics about the last search.
    BestMove {
        best_move: Move,
        ponder_move: Option<Move>,
    },

    /// This is needed for copyprotected engines.
    ///
    /// After the uciok command the engine can tell the GUI, that it will check the copy protection
    /// now. This is done by "copyprotection checking".  If the check is ok the engine should send
    /// "copyprotection ok", otherwise "copyprotection error".  If there is an error the engine
    /// should not function properly but should not quit alone.  If the engine reports
    /// "copyprotection error" the GUI should not use this engine and display an error message
    /// instead!
    CopyProtection(CopyProtectionMessage),

    /// This is needed for engines that need a username and/or a code to function with all features.
    ///
    /// Analog to the "copyprotection" command the engine can send "registration checking" after
    /// the uciok command followed by either "registration ok" or "registration error".  Also after
    /// every attempt to register the engine it should answer with "registration checking" and then
    /// either "registration ok" or "registration error".
    ///
    /// In contrast to the "copyprotection" command, the GUI can use the engine after the engine
    /// has reported an error, but should inform the user that the engine is not properly
    /// registered and might not use all its features.
    /// In addition the GUI should offer to open a dialog to enable registration of the engine. To
    /// try to register an engine the GUI can send the "register" command.  The GUI has to always
    /// answer with the "register" command if the engine sends "registration error" at engine
    /// startup (this can also be done with "register later") and tell the user somehow that the
    /// engine is not registered.  This way the engine knows that the GUI can deal with the
    /// registration procedure and the user will be informed that the engine is not properly
    /// registered.
    Registration(RegistrationMessage),

    /// The engine wants to send information to the GUI.
    ///
    /// This should be done whenever one of the info has changed. The engine can send only
    /// selected infos or multiple infos with one info command.
    Info(InfoMessage),

    Option(OptionMessage),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EngineCommandParseError {
    /// The command being parsed was completely empty
    EmptyCommand,

    /// Didn't recognize the starting keyword of the command
    UnrecognizedCommand(String),

    /// Recognized the command keyword, but what followed was invalid in some way
    InvalidCommand(String),
}

fn parse_setoption(cmd_str: &str) -> Result<UciCommand, EngineCommandParseError> {
    assert!(cmd_str.starts_with("setoption"));

    // TODO: this parse function is too brittle - it doesn't work if the value is given before the name
    let invalid_cmd = || EngineCommandParseError::InvalidCommand(cmd_str.to_string());
    if !cmd_str.starts_with("setoption name ") {
        Err(invalid_cmd())?;
    }
    let cmd_str = &cmd_str["setoption name ".len()..];

    let value_start = cmd_str.find("value");

    let option_name = match value_start {
        Some(idx) => cmd_str[..idx].trim().to_string(),
        None => cmd_str.trim().to_string(),
    };

    let value = value_start
        .map(|idx| idx + "value".len())
        .map(|idx| cmd_str[idx..].trim().to_string());

    Ok(UciCommand::SetOption { option_name, value })
}

fn parse_register(cmd_str: &str) -> Result<UciCommand, EngineCommandParseError> {
    assert!(cmd_str.starts_with("register"));
    let invalid_cmd = || EngineCommandParseError::InvalidCommand(cmd_str.to_string());

    if cmd_str == "register later" {
        return Ok(UciCommand::Register {
            name: None,
            code: None,
        });
    }

    let name_start = cmd_str.find("name");
    let code_start = cmd_str.find("code");

    if name_start.is_none() && code_start.is_none() {
        return Err(invalid_cmd());
    }

    let name = name_start
        .map(|ns| match code_start {
            Some(cs) if cs > ns => (ns, cs),
            _ => (ns, cmd_str.len()),
        })
        .map(|(start, end)| (start + "name".len(), end))
        .map(|(start, end)| cmd_str[start..end].trim().to_string());

    let code = code_start
        .map(|cs| match name_start {
            Some(ns) if ns > cs => (cs, ns),
            _ => (cs, cmd_str.len()),
        })
        .map(|(start, end)| (start + "code".len(), end))
        .map(|(start, end)| cmd_str[start..end].trim().to_string());

    Ok(UciCommand::Register { name, code })
}

fn parse_position(cmd_str: &str) -> Result<UciCommand, EngineCommandParseError> {
    let mut parts = cmd_str.split_ascii_whitespace();

    assert_eq!(parts.next(), Some("position"));

    let position = match parts.next() {
        Some("startpos") => Position::StartPos,
        Some("fen") => {
            // A valid FEN string has 6 whitespace separated components
            let mut fen_str = String::new();
            for idx in 0..6 {
                match parts.next() {
                    Some(part) => fen_str.push_str(part),
                    None => Err(EngineCommandParseError::InvalidCommand(cmd_str.to_string()))?,
                }

                if idx < 5 {
                    fen_str.push(' ');
                }
            }

            Position::FenString(fen_str)
        }
        _ => Err(EngineCommandParseError::InvalidCommand(cmd_str.to_string()))?,
    };

    let moves = match parts.next() {
        Some("moves") => parts
            .map(|p| Move::from_long_algebraic(p))
            .collect::<Result<_, _>>()
            .map_err(|_| EngineCommandParseError::InvalidCommand(cmd_str.to_string()))?,
        None => Vec::new(),
        Some(_) => Err(EngineCommandParseError::InvalidCommand(cmd_str.to_string()))?,
    };

    Ok(UciCommand::Position { position, moves })
}

fn parse_go(cmd_str: &str) -> Result<UciCommand, EngineCommandParseError> {
    let invalid_cmd = || EngineCommandParseError::InvalidCommand(cmd_str.to_string());

    let parse_int = |parts: &mut dyn Iterator<Item = &str>| match parts.next() {
        Some(s) => s.parse::<u64>().map_err(|_| invalid_cmd()),
        None => Err(invalid_cmd()),
    };

    let parse_milliseconds = |parts: &mut dyn Iterator<Item = &str>| match parts.next() {
        Some(s) => {
            let num = s.parse().map_err(|_| invalid_cmd())?;
            Ok(Duration::from_millis(num))
        }
        None => Err(invalid_cmd()),
    };

    let mut parts = cmd_str.split_ascii_whitespace().peekable();

    assert_eq!(parts.next(), Some("go"));

    let mut go_cmd = GoCommand::default();
    while let Some(tok) = parts.next() {
        match tok {
            "searchmoves" => {
                let mut moves = Vec::new();
                // Chomp tokens until the first one that isn't a valid algebraic move
                while let Some(tok) = parts.peek() {
                    if let Ok(m) = Move::from_long_algebraic(tok) {
                        moves.push(m);

                        // Discard the peeked token
                        parts.next();
                    } else {
                        break;
                    }
                }

                go_cmd.search_moves = Some(moves);
            }
            "ponder" => go_cmd.ponder = true,
            "wtime" => go_cmd.white_time = Some(parse_milliseconds(&mut parts)?),
            "btime" => go_cmd.black_time = Some(parse_milliseconds(&mut parts)?),
            "winc" => go_cmd.white_increment = Some(parse_milliseconds(&mut parts)?),
            "binc" => go_cmd.black_increment = Some(parse_milliseconds(&mut parts)?),
            "movestogo" => go_cmd.moves_to_go = Some(parse_int(&mut parts)? as u16),
            "depth" => go_cmd.depth = Some(parse_int(&mut parts)? as u8),
            "nodes" => go_cmd.nodes = Some(parse_int(&mut parts)?),
            "mate" => go_cmd.mate = Some(parse_int(&mut parts)? as u8),
            "movetime" => go_cmd.move_time = Some(parse_milliseconds(&mut parts)?),
            "infinite" => go_cmd.infinite = true,
            _ => Err(invalid_cmd())?,
        }
    }

    Ok(UciCommand::Go(go_cmd))
}

pub fn parse_command(cmd_str: &str) -> Result<UciCommand, EngineCommandParseError> {
    let mut parts = cmd_str.splitn(2, " ");

    let invalid_cmd = || EngineCommandParseError::InvalidCommand(cmd_str.to_string());

    let cmd = match parts.next() {
        Some("uci") => UciCommand::Uci,
        Some("debug") => {
            let arg = match parts.next() {
                Some("true") => true,
                Some("false") => false,
                _ => Err(invalid_cmd())?,
            };
            UciCommand::Debug(arg)
        }
        Some("isready") => UciCommand::IsReady,
        Some("setoption") => parse_setoption(cmd_str)?,
        Some("register") => parse_register(cmd_str)?,
        Some("ucinewgame") => UciCommand::UciNewGame,
        Some("position") => parse_position(cmd_str)?,
        Some("go") => parse_go(cmd_str)?,
        Some("stop") => UciCommand::Stop,
        Some("ponderhit") => UciCommand::PonderHit,
        Some("quit") => UciCommand::Quit,
        Some(_) => Err(invalid_cmd())?,
        None => Err(EngineCommandParseError::EmptyCommand)?,
    };

    Ok(cmd)
}

fn format_info_message(msg: InfoMessage) -> String {
    let mut out = String::from("info");

    if let Some(x) = msg.depth {
        write!(out, " depth {}", x).unwrap();
    }

    if let Some(x) = msg.selective_depth {
        write!(out, " seldepth {}", x).unwrap();
    }

    if let Some(x) = msg.time {
        write!(out, " time {}", x.as_millis()).unwrap();
    }

    if let Some(x) = msg.nodes {
        write!(out, " nodes {}", x).unwrap();
    }

    if let Some(x) = msg.principal_variation {
        write!(out, " pv").unwrap();
        for m in x {
            write!(out, " {:?}", m).unwrap();
        }
    }

    if let Some(x) = msg.multipv {
        write!(out, " multipv {}", x).unwrap();
    }

    if let Some(x) = msg.score {
        write!(out, " score {}", x.centipawns).unwrap();
        if let Some(mate) = x.mate {
            write!(out, " mate {}", mate).unwrap();
        }

        if x.lowerbound {
            write!(out, " lowerbound").unwrap();
        }

        if x.upperbound {
            write!(out, " upperbound").unwrap();
        }
    }

    if let Some(x) = msg.curr_move {
        write!(out, " currmove {:?}", x).unwrap();
    }

    if let Some(x) = msg.curr_move_number {
        write!(out, " currmovenumber {}", x).unwrap();
    }

    if let Some(x) = msg.hash_full {
        write!(out, " hashfull {}", x).unwrap();
    }

    if let Some(x) = msg.nodes_per_second {
        write!(out, " nps {}", x).unwrap();
    }

    if let Some(x) = msg.table_hits {
        write!(out, " tbhits {}", x).unwrap();
    }

    if let Some(x) = msg.shredder_hits {
        write!(out, " sbhits {}", x).unwrap();
    }

    if let Some(x) = msg.cpu_load {
        write!(out, " cpuload {}", x).unwrap();
    }

    if let Some(x) = msg.refutation {
        write!(out, " refutation {:?}", x.refuted_move).unwrap();
        for m in x.refutation_line {
            write!(out, " {:?}", m).unwrap();
        }
    }

    if let Some(x) = msg.current_line {
        write!(out, " currline").unwrap();
        if let Some(cpu_number) = x.cpu_number {
            write!(out, " {}", cpu_number).unwrap();
        }

        for m in x.line {
            write!(out, " {:?}", m).unwrap();
        }
    }

    // NB the string has to come last, as it will cause the rest of the line to be parsed as the
    // string contents.
    if let Some(x) = msg.string {
        write!(out, " string {}", x).unwrap();
    }

    out
}

fn format_option_message(msg: OptionMessage) -> String {
    let mut out = format!("option name {}", msg.option_name);

    match msg.option_type {
        OptionType::Check => write!(out, " type check"),
        OptionType::Spin => write!(out, " type spin"),
        OptionType::Combo => write!(out, " type combo"),
        OptionType::Button => write!(out, " type button"),
        OptionType::String => write!(out, " type string"),
    }
    .unwrap();

    if let Some(default) = msg.default {
        write!(out, " default {}", default).unwrap();
    }

    if let Some(min) = msg.min {
        write!(out, " min {}", min).unwrap();
    }

    if let Some(max) = msg.max {
        write!(out, " max {}", max).unwrap();
    }

    if let Some(combo_options) = msg.combo_options {
        for c in combo_options {
            write!(out, " var {}", c).unwrap();
        }
    }

    out
}

pub fn format_message(msg: UciMessage) -> String {
    match msg {
        UciMessage::Id(id) => match id {
            EngineId::Name(name) => format!("id name {}", name),
            EngineId::Author(author) => format!("id author {}", author),
        },
        UciMessage::UciOk => format!("uciok"),
        UciMessage::ReadyOk => format!("readyok"),
        UciMessage::BestMove {
            best_move,
            ponder_move,
        } => match ponder_move {
            Some(p) => format!("bestmove {:?} ponder {:?}", best_move, p),
            None => format!("bestmove {:?}", best_move),
        },
        UciMessage::CopyProtection(c) => match c {
            CopyProtectionMessage::Checking => format!("copprotection checking"),
            CopyProtectionMessage::Ok => format!("copprotection ok"),
            CopyProtectionMessage::Error => format!("copprotection error"),
        },
        UciMessage::Registration(r) => match r {
            RegistrationMessage::Checking => format!("registration checking"),
            RegistrationMessage::Ok => format!("registration ok"),
            RegistrationMessage::Error => format!("registration error"),
        },
        UciMessage::Info(i) => format_info_message(i),
        UciMessage::Option(o) => format_option_message(o),
    }
}

pub trait UciOptions: Default {
    type SetOptionError;

    fn all_options() -> Vec<OptionMessage>;
    fn set_value(&mut self, option_name: &str, value: &str) -> Result<(), Self::SetOptionError>;
}

pub struct UciInterface<Options: UciOptions> {
    pub tx: Sender<UciMessage>,
    pub rx: Receiver<UciCommand>,
    pub opts: RwLock<Options>,
}

impl<Options: UciOptions> UciInterface<Options> {
    /// Spawns the IO thread and negotiates intial setup over the interface
    pub fn startup() -> Result<Self> {
        let (messages_tx, messages_rx) = unbounded();
        let (commands_tx, commands_rx) = unbounded();

        std::thread::Builder::new()
            .name("UCI broker".to_string())
            .spawn(|| uci_interface_thread(messages_rx, commands_tx))?;

        Ok(Self {
            opts: Options::default().into(),
            tx: messages_tx,
            rx: commands_rx,
        })
    }
}

fn uci_interface_thread(messages_rx: Receiver<UciMessage>, commands_tx: Sender<UciCommand>) {
    use std::io::Write;

    let stdout = std::io::stdout();
    let mut stdout_handle = stdout.lock();

    let (stdin_lines_tx, stdin_lines_rx) = unbounded();

    // TODO: clean up this thread on shutdown explicitly.
    std::thread::Builder::new()
        .name("UCI reader".to_string())
        .spawn(move || {
            let stdin = std::io::stdin();
            let stdin_handle = stdin.lock();
            let mut lines = stdin_handle.lines();
            while let Some(Ok(line)) = lines.next() {
                stdin_lines_tx
                    .send(line)
                    .expect("Error pushing raw UCI command to internal channel");
            }
        })
        .expect("Failed to start UCI reader thread");

    loop {
        select! {
            recv(messages_rx) -> msg => {
                let msg = match msg {
                    Ok(msg) => msg,
                    Err(_) => {
                        log::info!("UCI messages channel disconnected, shutting down UCI threads");
                        break;
                    }
                };
                log::debug!("Sending message {:?}", msg);
                write!(stdout_handle, "{}\n", format_message(msg))
                    .expect("Error sending UCI message to interface");
            },
            recv(stdin_lines_rx) -> line => {
                let line = match line {
                    Ok(line) => line,
                    Err(_) => {
                        log::info!("EOF received on stdin, sending implicit Quit command and stopping UCI thread");
                        commands_tx.send(UciCommand::Quit)
                            .expect("Error pushing UCI command to internal channel");
                        break;
                    }
                };
                if let Ok(cmd) = parse_command(&line) {
                    log::debug!("Received command {:?}", cmd);
                    commands_tx.send(cmd)
                        .expect("Error pushing UCI command to internal channel")
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_uci() {
        assert_eq!(parse_command("uci"), Ok(UciCommand::Uci));
    }

    #[test]
    fn test_parse_debug() {
        assert_eq!(parse_command("debug true"), Ok(UciCommand::Debug(true)));
        assert_eq!(parse_command("debug false"), Ok(UciCommand::Debug(false)));
    }

    #[test]
    fn test_parse_isready() {
        assert_eq!(parse_command("isready"), Ok(UciCommand::IsReady));
    }

    #[test]
    fn test_parse_setoption() {
        assert_eq!(
            parse_command("setoption name foo"),
            Ok(UciCommand::SetOption {
                option_name: "foo".to_string(),
                value: None,
            })
        );
        assert_eq!(
            parse_command("setoption name foo value 123"),
            Ok(UciCommand::SetOption {
                option_name: "foo".to_string(),
                value: Some("123".to_string()),
            })
        );
    }

    #[test]
    fn test_parse_register() {
        assert_eq!(
            parse_command("register later"),
            Ok(UciCommand::Register {
                name: None,
                code: None,
            })
        );
        assert_eq!(
            parse_command("register name joerob"),
            Ok(UciCommand::Register {
                name: Some("joerob".to_string()),
                code: None,
            })
        );
        assert_eq!(
            parse_command("register code asdf"),
            Ok(UciCommand::Register {
                name: None,
                code: Some("asdf".to_string()),
            })
        );
    }

    #[test]
    fn test_parse_ucinewgame() {
        assert_eq!(parse_command("ucinewgame"), Ok(UciCommand::UciNewGame));
    }

    #[test]
    fn test_parse_position() {
        assert_eq!(
            parse_command("position startpos"),
            Ok(UciCommand::Position {
                position: Position::StartPos,
                moves: Vec::new(),
            })
        );

        let example_fen = "7k/2P5/3p4/7r/K7/8/8/8 w - - 0 1".to_string();
        assert_eq!(
            parse_command(&format!("position fen {}", &example_fen)),
            Ok(UciCommand::Position {
                position: Position::FenString(example_fen.clone()),
                moves: Vec::new(),
            })
        );

        assert_eq!(
            parse_command("position startpos moves a2a3 g7g5"),
            Ok(UciCommand::Position {
                position: Position::StartPos,
                moves: vec![
                    Move::from_long_algebraic("a2a3").unwrap(),
                    Move::from_long_algebraic("g7g5").unwrap(),
                ],
            })
        );

        assert_eq!(
            parse_command(&format!("position fen {} moves c7c8q g8g7", &example_fen)),
            Ok(UciCommand::Position {
                position: Position::FenString(example_fen.clone()),
                moves: vec![
                    Move::from_long_algebraic("c7c8q").unwrap(),
                    Move::from_long_algebraic("g8g7").unwrap(),
                ],
            })
        );
    }

    #[test]
    fn test_parse_go() {
        assert_eq!(
            parse_command("go ponder"),
            Ok(UciCommand::Go(GoCommand {
                ponder: true,
                ..GoCommand::default()
            }))
        );

        assert_eq!(
            parse_command("go infinite"),
            Ok(UciCommand::Go(GoCommand {
                infinite: true,
                ..GoCommand::default()
            }))
        );

        assert_eq!(
            parse_command("go searchmoves a2a4 movetime 1500"),
            Ok(UciCommand::Go(GoCommand {
                move_time: Some(Duration::from_millis(1500)),
                search_moves: Some(vec![Move::from_long_algebraic("a2a4").unwrap(),]),
                ..GoCommand::default()
            }))
        );

        assert_eq!(
            parse_command("go movestogo 10"),
            Ok(UciCommand::Go(GoCommand {
                moves_to_go: Some(10),
                ..GoCommand::default()
            }))
        );
    }

    #[test]
    fn test_parse_stop() {
        assert_eq!(parse_command("stop"), Ok(UciCommand::Stop));
    }

    #[test]
    fn test_parse_ponderhit() {
        assert_eq!(parse_command("ponderhit"), Ok(UciCommand::PonderHit));
    }

    #[test]
    fn test_parse_quit() {
        assert_eq!(parse_command("quit"), Ok(UciCommand::Quit));
    }
}
