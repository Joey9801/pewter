use crate::bitboard::BitBoard;
use crate::coordinates::BoardPos;
use crate::Piece;

#[derive(Clone, Copy, Debug)]
pub struct Move {
    pub from: BoardPos,
    pub to: BoardPos,
    pub promotion: Option<Piece>,
}

#[derive(Clone, Copy, Debug)]
pub enum ParseLongAlgebraicError {
    MissingChars,
    NonAsciiBytes,
    InvalidSquare,
    BadPromotion,
}

impl Move {
    pub fn from_long_algebraic(
        algebraic_str: &str,
    ) -> Result<Self, ParseLongAlgebraicError> {
        if algebraic_str.len() < 4 {
            return Err(ParseLongAlgebraicError::MissingChars);
        }
        if !algebraic_str.is_ascii() {
            return Err(ParseLongAlgebraicError::NonAsciiBytes);
        }

        let from = BoardPos::from_algebraic(&algebraic_str[0..2])
            .ok_or(ParseLongAlgebraicError::InvalidSquare)?;
        let to = BoardPos::from_algebraic(&algebraic_str[2..4])
            .ok_or(ParseLongAlgebraicError::InvalidSquare)?;

        let mut m = Move {
            from,
            to,
            promotion: None,
        };

        if algebraic_str.as_bytes().len() > 4 {
            let p = match &algebraic_str[5..6] {
                "q" => Piece::Queen,
                "r" => Piece::Rook,
                "b" => Piece::Bishop,
                "n" => Piece::Knight,
                _ => return Err(ParseLongAlgebraicError::BadPromotion),
            };

            m.promotion = Some(p);
        }

        Ok(m)
    }

    pub fn format_long_algebraic(&self) -> String {
        let mut out = format!("{}{}", self.from.to_algebraic(), self.to.to_algebraic());

        if let Some(promotion) = self.promotion {
            out.push(match promotion {
                Piece::Rook => 'q',
                Piece::Knight => 'n',
                Piece::Bishop => 'b',
                Piece::Queen => 'q',
                _ => unreachable!(),
            });
        }

        out
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MoveSet {
    pub source: BoardPos,
    pub dest_set: BitBoard,
    pub promotion: bool,
}

impl MoveSet {
    pub fn iter(&self) -> impl Iterator<Item=Move> {
        MoveSetIter {
            move_set: *self,
            promotion_idx: 0,
        }
    }
}

pub struct MoveSetIter {
    move_set: MoveSet,
    promotion_idx: u8,
}

impl Iterator for MoveSetIter {
    type Item = Move;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(dest) = self.move_set.dest_set.first_set() {
            let mut m = Move {
                from: self.move_set.source,
                to: dest,
                promotion: None
            };

            if self.move_set.promotion {
                match self.promotion_idx % 4 {
                    0 => m.promotion = Some(Piece::Queen),
                    1 => m.promotion = Some(Piece::Rook),
                    2 => m.promotion = Some(Piece::Bishop),
                    3 => {
                        m.promotion = Some(Piece::Knight);
                        self.move_set.dest_set.clear(dest);
                    }
                    _ => {
                        unreachable!()
                    }
                }

                self.promotion_idx += 1;
            }

            Some(m)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let s = self.move_set.dest_set.count() as usize;

        // 4x as many items if iterating promotions
        let s = s << if self.move_set.promotion { 2 } else { 0 };

        (s, Some(s))
    }
}

impl ExactSizeIterator for MoveSetIter {
    fn len(&self) -> usize {
        let (lower, upper) = self.size_hint();
        debug_assert_eq!(upper, Some(lower));
        lower
    }
}