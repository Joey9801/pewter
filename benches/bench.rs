use criterion::{black_box, criterion_group, criterion_main, Criterion};
use pewter::{io::fen::parse_fen, Move};

struct ApplyMoveBenchmark {
    name: &'static str,
    fen_str: &'static str,
    move_str: &'static str,
}

static APPLY_MOVE_BENCHMARKS: &[ApplyMoveBenchmark] = &[
    ApplyMoveBenchmark {
        name: "single-pawn-push",
        fen_str: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        move_str: "a2a3",
    },
    ApplyMoveBenchmark {
        name: "double-pawn-push",
        fen_str: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        move_str: "a2a4",
    },
    ApplyMoveBenchmark {
        name: "kingside-castle",
        fen_str: "rnbqkbnr/pppppppp/8/8/1N6/4B3/PPPPPPPP/RNBQK2R w KQkq - 0 1",
        move_str: "e1g1",
    },
    ApplyMoveBenchmark {
        name: "queenside-castle",
        fen_str: "rnbqkbnr/pppppppp/8/8/8/1NQ1B3/PPPPPPPP/R3KBNR w KQkq - 0 1",
        move_str: "e1c1",
    },
    ApplyMoveBenchmark {
        name: "new-pinned-piece",
        fen_str: "rnbqkbnr/pppppppp/8/8/1N1R4/4B3/PPPPPPPP/RNBQK3 w Qkq - 0 1",
        move_str: "d4e4",
    },
];

pub fn apply_move(c: &mut Criterion) {
    let mut group = c.benchmark_group("State::apply_move");
    for bench_def in APPLY_MOVE_BENCHMARKS {
        let state = parse_fen(bench_def.fen_str)
            .expect("Expected benchmark definition to have a valid FEN string");
        let m = Move::from_long_algebraic(bench_def.move_str)
            .expect("Expected benchmark definition to have a valid move string");

        group.bench_function(bench_def.name, |b| {
            b.iter(|| black_box(state.apply_move(black_box(m))))
        });
    }
}

pub fn generate_legal_moves(c: &mut Criterion) {
    let positions = &[
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "r2q1rk1/pp1nppbp/2pp1np1/8/2PPPB2/2N2B1P/PP3PP1/R2Q1RK1 w - - 3 11"
    ];

    let mut group = c.benchmark_group("movegen::legal_moves");
    for pos in positions {
        let state = parse_fen(pos)
            .expect("Expected benchmark definition to have a valid FEN string");

        group.bench_function(*pos, |b| {
            b.iter(|| black_box(pewter::movegen::legal_moves(black_box(&state))));
        });
    }
}

criterion_group!(benches, apply_move, generate_legal_moves);
criterion_main!(benches);
