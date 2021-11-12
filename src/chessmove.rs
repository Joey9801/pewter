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
