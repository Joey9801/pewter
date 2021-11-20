use arrayvec::ArrayVec;

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
pub struct MoveSetChunk {
    pub source: BoardPos,
    pub dest_set: BitBoard,
    pub promotion: bool,
}

impl MoveSetChunk {
    pub fn iter(self) -> MoveSetChunkIter {
        MoveSetChunkIter {
            inner: self,
            promotion_idx: 0
        }
    }
}

pub struct MoveSetChunkIter {
    inner: MoveSetChunk,
    promotion_idx: u8,
}

impl Iterator for MoveSetChunkIter {
    type Item = Move;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(dest) = self.inner.dest_set.first_set() {
            let mut m = Move {
                from: self.inner.source,
                to: dest,
                promotion: None
            };

            if self.inner.promotion {
                match self.promotion_idx % 4 {
                    0 => m.promotion = Some(Piece::Queen),
                    1 => m.promotion = Some(Piece::Rook),
                    2 => m.promotion = Some(Piece::Bishop),
                    3 => {
                        m.promotion = Some(Piece::Knight);
                        self.inner.dest_set.clear(dest);
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
        let s = self.inner.dest_set.count() as usize;

        // 4x as many items if iterating promotions
        let s = s << if self.inner.promotion { 2 } else { 0 };

        (s, Some(s))
    }
}

impl ExactSizeIterator for MoveSetChunkIter { }

#[derive(Clone, Debug)]
pub struct MoveSet {
    chunks: ArrayVec<MoveSetChunk, 16>,
}

impl MoveSet {
    pub fn iter(&self) -> impl Iterator<Item=Move> + '_ {
        self.chunks
            .iter()
            .map(|c| c.iter())
            .flatten()
    }
}