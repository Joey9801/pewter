use std::iter::FromIterator;

use arrayvec::ArrayVec;

use crate::BitBoard;
use crate::BoardPos;
use crate::Piece;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
    pub fn from_long_algebraic(algebraic_str: &str) -> Result<Self, ParseLongAlgebraicError> {
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
            let p = match &algebraic_str[4..5] {
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
                Piece::Rook => 'r',
                Piece::Knight => 'n',
                Piece::Bishop => 'b',
                Piece::Queen => 'q',
                _ => unreachable!(),
            });
        }

        out
    }
}

impl std::fmt::Debug for Move {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_long_algebraic())
    }
}

impl std::fmt::Display for Move {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Represents a set of moves originating from a single position
#[derive(Clone, Copy, Debug)]
pub struct MoveSetChunk {
    pub source: BoardPos,
    pub dest_set: BitBoard,
    pub promotion: bool,
}

impl MoveSetChunk {
    pub const fn new_empty(source: BoardPos) -> Self {
        Self {
            source,
            dest_set: BitBoard::new_empty(),
            promotion: false,
        }
    }

    pub fn iter(self) -> MoveSetChunkIter {
        MoveSetChunkIter {
            inner: self,
            promotion_idx: 0,
        }
    }

    pub fn len(self) -> u8 {
        self.dest_set.count() * if self.promotion { 4 } else { 1 }
    }
    
    pub fn any(self) -> bool {
        self.dest_set.any()
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
                promotion: None,
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
            } else {
                self.inner.dest_set.clear(dest);
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

impl ExactSizeIterator for MoveSetChunkIter {}

#[derive(Clone, Debug)]
pub struct MoveSet {
    // 16, as there is at most one chunk per piece
    chunks: ArrayVec<MoveSetChunk, 16>,
}

impl MoveSet {
    pub fn new_empty() -> Self {
        Self {
            chunks: ArrayVec::new(),
        }
    }
    
    pub fn push(&mut self, chunk: MoveSetChunk) {
        if chunk.dest_set.any() {
            self.chunks.push(chunk);
        }
    }

    pub fn len(&self) -> usize {
        self.chunks.iter().map(|c| c.len() as usize).sum()
    }

    // TODO: This iterator isn't an ExactSizeIterator, but notionally could be
    // Probably doesn't matter, but perhaps worth exploring when optimizing performance
    pub fn iter(&self) -> impl Iterator<Item = Move> + '_ {
        self.chunks.iter().flat_map(|c| c.iter())
    }
}

impl FromIterator<MoveSetChunk> for MoveSet {
    fn from_iter<T: IntoIterator<Item = MoveSetChunk>>(iter: T) -> Self {
        let mut ms = Self::new_empty();
        for chunk in iter {
            ms.chunks.push(chunk);
        }
        ms
    }
}