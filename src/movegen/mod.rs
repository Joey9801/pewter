pub mod bmi;
pub mod pseudo_legal;
pub mod legal;


pub fn perft(state: crate::State, depth: u8) -> usize {
    match depth {
        0 => 0,
        1 => legal::legal_moves(&state).len(),
        _ => legal::legal_moves(&state)
            .iter()
            .map(|m| perft(state.apply_move(m), depth - 1))
            .sum()
    }
}

pub fn perft_breakdown(state: crate::State, depth: u8) -> Vec<(crate::Move, usize)> {
    match depth {
        0 => vec![],
        1 => legal::legal_moves(&state).iter().map(|m| (m, 1)).collect(),
        _ => legal::legal_moves(&state).iter().map(|m| (m, perft(state.apply_move(m), depth - 1))).collect()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use crate::io::fen::parse_fen;

    use super::*;

    fn perft_helper_inner(initial_state: crate::State, expected_values: &[usize]) {
        for (depth, expected) in expected_values.iter().enumerate() {
            let depth = depth + 1;
            println!("Testing depth {}", depth);
            let sw = Instant::now();

            let breakdown = perft_breakdown(initial_state, depth as u8);
            let total = breakdown.iter().map(|(m, count)| count).sum::<usize>();
            
            // Print the breakdown for debugging iff the assert is going to fail
            if total != *expected {
                dbg!(depth);
                for (m, count) in breakdown.iter() {
                    println!("{}: {}", m.format_long_algebraic(), count);
                }
            }
            assert_eq!(total, *expected);
            println!("   Depth {} successful, time = {:?}", depth, sw.elapsed());
        }
    }
    
    fn perft_helper(fen_str: &str, expected_values: &[usize]) {
        let initial_state = parse_fen(fen_str)
            .expect("Expected unit test to have valid FEN string");
        dbg!(fen_str);
        perft_helper_inner(initial_state, expected_values);
    }
    
    #[test]
    fn perft_test_starting() {
        // Only test deeper cases when in release mode, to stop this test taking too long
        #[cfg(debug_assertions)]
        let max_depth = 4;

        #[cfg(not(debug_assertions))]
        let max_depth = 6;

        perft_helper(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            &[
                20,
                400,
                8_902,
                197_281,
                4_865_609,
                119_060_324,
            ][..max_depth]
        );
    }
    
    // #[test]
    // fn foo() {
    //     let mut s = parse_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();

    //     for m in ["a2a4", "b7b5", "a4a5", "b5b4"].iter() {
    //         let m = crate::Move::from_long_algebraic(m).unwrap();
    //         s = s.apply_move(m);
    //     }
    //     
    //     perft_helper_inner(s, &[20, 436]);
    // }
    
    #[test]
    fn perft_test_pos_2() {
        perft_helper(
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            &[
                48,
                2_039,
                97_682,
                4_085_603,
                193_690_690,
            ]
        )
    }
}