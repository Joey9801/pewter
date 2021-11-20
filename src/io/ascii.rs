use crate::coordinates::BoardPos;

pub fn pretty_format(sym: impl Fn(BoardPos) -> char) -> String {
    use crate::coordinates::File::*;
    use crate::coordinates::Rank::*;

    let mut output = String::new();
    output.push_str(" +---+---+---+---+---+---+---+---+\n");

    for rank in [R8, R7, R6, R5, R4, R3, R2, R1].iter() {
        let mut first_col = true;
        for file in [A, B, C, D, E, F, G, H].iter() {
            if first_col {
                output.push_str(" |");
            }
            output.push_str(&format!(
                " {} |",
                sym(BoardPos::from_file_rank(*file, *rank))
            ));
            first_col = false;
        }
        output.push_str(&format!(" {}\n", rank.to_num() + 1));
        output.push_str(" +---+---+---+---+---+---+---+---+\n");
    }

    output.push_str("   A   B   C   D   E   F   G   H\n");

    output
}
