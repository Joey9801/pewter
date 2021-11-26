pub mod bmi;
pub mod pseudo_legal;
pub mod legal;


pub fn perft(state: crate::State, depth: u8) -> usize {
    match depth {
        0 => 1,
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
            let total = breakdown.iter().map(|(_m, count)| count).sum::<usize>();
            
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
        perft_helper(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            &[
                20,
                400,
                8_902,
                197_281,
            ]
        );
    }
    
    #[test]
    fn perft_test_pos_2() {
        perft_helper(
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            &[
                48,
                2_039,
                97_862,
            ]
        )
    }
    
    #[test]
    fn perft_test_pos_3() {
        perft_helper(
            "r1b1k1nr/ppq2p1p/6pb/4p3/2BpP3/5QPP/PPPN4/R3K2R w KQkq - 1 13",
            &[
                47,
                1_598,
                66_482,
            ]
        )
    }
    
    #[test]
    fn perft_test_pos_4() {
        // NB: explicitly testing an en-passant move that is illegal as it reveals a check
        perft_helper(
            "7k/3p4/8/K1P4r/8/8/8/8 w - - 0 1",
            &[
                5,
                80,
                514
            ]
        )
    }
    
    #[test]
    fn perft_test_pos_5() {
        // NB: At depth 5 ends up testing that you are allowed to capture a pawn that is giving check en-passant
        perft_helper(
            "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
            &[
                14,
                191,
                2_812,
                43_238,
                674_624
            ]
        )
    }
}