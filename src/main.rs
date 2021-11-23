use pewter::bitboard::masks;
use pewter::coordinates::consts::*;

fn main() {
    println!("{}", masks::between(B2, G2).pretty_format());
    println!("{}", masks::between(B2, B7).pretty_format());
    println!("{}", masks::between(C2, F5).pretty_format());
    println!("{}", masks::between(B6, G1).pretty_format());
}
