use std::fs::File;
use std::io::Write;
use std::sync::LazyLock;

use crate::bitboard::BitBoard;
use crate::square::Square;

static KNIGHT_MOVES: LazyLock<[BitBoard; 64]> = LazyLock::new(|| {
    let mut knight_moves = [BitBoard(0); 64];
    for square in Square::all_squares() {
        knight_moves[square.to_index()] = Square::all_squares()
            .filter(|dest| {
                let src_rank = square.get_rank().to_index() as i8;
                let src_file = square.get_file().to_index() as i8;
                let dest_rank = dest.get_rank().to_index() as i8;
                let dest_file = dest.get_file().to_index() as i8;

                ((src_rank - dest_rank).abs() == 2 && (src_file - dest_file).abs() == 1)
                    || ((src_rank - dest_rank).abs() == 1 && (src_file - dest_file).abs() == 2)
            })
            .fold(BitBoard(0), |b, s| b | BitBoard::from_square(s));
    }
    knight_moves
});

pub fn write_knight_moves(f: &mut File) -> std::io::Result<()> {
    writeln!(f, "const KNIGHT_MOVES: [BitBoard; 64] = [")?;
    for knight_move in KNIGHT_MOVES.iter() {
        writeln!(f, "    BitBoard({}),", knight_move.0)?;
    }
    writeln!(f, "];")?;
    Ok(())
}
