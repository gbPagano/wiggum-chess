use std::fs::File;
use std::io::Write;

use super::between::{are_squares_diagonal, are_squares_linear};
use crate::bitboard::BitBoard;
use crate::square::Square;
use std::sync::LazyLock;

static LINES: LazyLock<[[BitBoard; 64]; 64]> = LazyLock::new(|| {
    let mut lines = [[BitBoard(0); 64]; 64];
    for src in Square::all_squares() {
        for dest in Square::all_squares() {
            if src == dest
                || (!are_squares_diagonal(&src, &dest) && !are_squares_linear(&src, &dest))
            {
                continue;
            }

            lines[src.to_index()][dest.to_index()] = Square::all_squares()
                .filter(|test| {
                    if are_squares_diagonal(&src, &dest) {
                        are_squares_diagonal(&src, test) && are_squares_diagonal(&dest, test)
                    } else {
                        is_on_straight_line(&src, test, &dest)
                    }
                })
                .fold(BitBoard(0), |board, square| {
                    board | BitBoard::from_square(square)
                });
        }
    }
    lines
});

fn is_on_straight_line(src: &Square, test: &Square, dest: &Square) -> bool {
    let src_rank = src.get_rank().to_index() as i8;
    let src_file = src.get_file().to_index() as i8;
    let dest_rank = dest.get_rank().to_index() as i8;
    let dest_file = dest.get_file().to_index() as i8;
    let test_rank = test.get_rank().to_index() as i8;
    let test_file = test.get_file().to_index() as i8;

    let same_horizontal = src_rank == test_rank && dest_rank == test_rank;

    let same_vertical = src_file == test_file && dest_file == test_file;

    same_horizontal || same_vertical
}

pub fn write_lines(f: &mut File) -> std::io::Result<()> {
    writeln!(f, "const LINES: [[BitBoard; 64]; 64] = [[")?;
    for i in 0..64 {
        for j in 0..64 {
            writeln!(f, "    BitBoard({}),", LINES[i][j].0)?;
        }
        if i != 63 {
            writeln!(f, "  ], [")?;
        }
    }
    writeln!(f, "]];")?;
    Ok(())
}
