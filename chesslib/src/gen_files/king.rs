use std::io::Write;
use std::sync::LazyLock;

use crate::bitboard::BitBoard;
use crate::color::Color;
use crate::file::File;
use crate::square::Square;

static KING_MOVES: LazyLock<[BitBoard; 64]> = LazyLock::new(|| {
    let mut king_moves = [BitBoard(0); 64];
    for square in Square::all_squares() {
        king_moves[square.to_index()] = Square::all_squares()
            .filter(|dest| {
                let src_rank = square.get_rank().to_index() as i8;
                let src_file = square.get_file().to_index() as i8;
                let dest_rank = dest.get_rank().to_index() as i8;
                let dest_file = dest.get_file().to_index() as i8;

                ((src_rank - dest_rank).abs() == 1 || (src_rank - dest_rank).abs() == 0)
                    && ((src_file - dest_file).abs() == 1 || (src_file - dest_file).abs() == 0)
                    && square != *dest
            })
            .fold(BitBoard(0), |b, s| b | BitBoard::from_square(s));
    }

    king_moves
});
static KINGSIDE_CASTLE_SQUARES: LazyLock<[BitBoard; 2]> = LazyLock::new(|| {
    let mut kingside_castle_squares = [BitBoard(0); 2];
    for color in [Color::White, Color::Black] {
        kingside_castle_squares[color.to_index()] = BitBoard::set(color.starting_rank(), File::F)
            ^ BitBoard::set(color.starting_rank(), File::G);
    }
    kingside_castle_squares
});
static QUEENSIDE_CASTLE_SQUARES: LazyLock<[BitBoard; 2]> = LazyLock::new(|| {
    let mut queenside_castle_squares = [BitBoard(0); 2];
    for color in [Color::White, Color::Black] {
        queenside_castle_squares[color.to_index()] = BitBoard::set(color.starting_rank(), File::B)
            ^ BitBoard::set(color.starting_rank(), File::C)
            ^ BitBoard::set(color.starting_rank(), File::D);
    }
    queenside_castle_squares
});

fn castle_squares() -> BitBoard {
    BitBoard::from_square("c1".parse().unwrap())
        ^ BitBoard::from_square("c8".parse().unwrap())
        ^ BitBoard::from_square("e1".parse().unwrap())
        ^ BitBoard::from_square("e8".parse().unwrap())
        ^ BitBoard::from_square("g1".parse().unwrap())
        ^ BitBoard::from_square("g8".parse().unwrap())
}

pub fn write_king_moves(f: &mut std::fs::File) -> std::io::Result<()> {
    writeln!(f, "const KING_MOVES: [BitBoard; 64] = [")?;
    for i in 0..64 {
        writeln!(f, "    BitBoard({}),", KING_MOVES[i].0)?;
    }
    writeln!(f, "];")?;

    writeln!(f, "pub const KINGSIDE_CASTLE_SQUARES: [BitBoard; 2] = [")?;
    writeln!(
        f,
        " BitBoard({}), BitBoard({})];",
        KINGSIDE_CASTLE_SQUARES[0].0, KINGSIDE_CASTLE_SQUARES[1].0
    )?;

    writeln!(f, "pub const QUEENSIDE_CASTLE_SQUARES: [BitBoard; 2] = [")?;
    writeln!(
        f,
        " BitBoard({}), BitBoard({})];",
        QUEENSIDE_CASTLE_SQUARES[0].0, QUEENSIDE_CASTLE_SQUARES[1].0
    )?;

    writeln!(
        f,
        "const CASTLE_SQUARES: BitBoard = BitBoard({});",
        castle_squares().0
    )?;

    Ok(())
}
