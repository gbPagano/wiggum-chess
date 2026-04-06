use anyhow::Result;
use std::fs::File;
use std::io::Write;
use std::sync::LazyLock;

use crate::bitboard::BitBoard;
use crate::square::Square;

static BETWEEN: LazyLock<[[BitBoard; 64]; 64]> = LazyLock::new(|| {
    let mut between = [[BitBoard(0); 64]; 64];
    for src in Square::all_squares() {
        for dest in Square::all_squares() {
            if src == dest
                || (!are_squares_diagonal(&src, &dest) && !are_squares_linear(&src, &dest))
            {
                continue;
            }
            between[src.to_index()][dest.to_index()] = Square::all_squares()
                .filter(|test| {
                    if are_squares_diagonal(&src, &dest) {
                        is_on_diagonal_between(&src, test, &dest)
                    } else {
                        is_on_line_between(&src, test, &dest)
                    }
                })
                .fold(BitBoard(0), |board, square| {
                    board | BitBoard::from_square(square)
                });
        }
    }
    between
});

fn is_between(a: i8, value: i8, b: i8) -> bool {
    let (min, max) = if a < b { (a, b) } else { (b, a) };
    min < value && value < max
}

pub fn are_squares_diagonal(src: &Square, dest: &Square) -> bool {
    let src_rank = src.get_rank().to_index() as i8;
    let src_file = src.get_file().to_index() as i8;
    let dest_rank = dest.get_rank().to_index() as i8;
    let dest_file = dest.get_file().to_index() as i8;

    (src_rank - dest_rank).abs() == (src_file - dest_file).abs()
}

pub fn are_squares_linear(src: &Square, dest: &Square) -> bool {
    let src_rank = src.get_rank().to_index() as i8;
    let src_file = src.get_file().to_index() as i8;
    let dest_rank = dest.get_rank().to_index() as i8;
    let dest_file = dest.get_file().to_index() as i8;

    src_rank == dest_rank || src_file == dest_file
}

fn is_on_diagonal_between(src: &Square, test: &Square, dest: &Square) -> bool {
    let src_rank = src.get_rank().to_index() as i8;
    let dest_rank = dest.get_rank().to_index() as i8;
    let test_rank = test.get_rank().to_index() as i8;

    are_squares_diagonal(src, test)
        && are_squares_diagonal(dest, test)
        && is_between(src_rank, test_rank, dest_rank)
}

fn is_on_line_between(src: &Square, test: &Square, dest: &Square) -> bool {
    let src_rank = src.get_rank().to_index() as i8;
    let src_file = src.get_file().to_index() as i8;
    let dest_rank = dest.get_rank().to_index() as i8;
    let dest_file = dest.get_file().to_index() as i8;
    let test_rank = test.get_rank().to_index() as i8;
    let test_file = test.get_file().to_index() as i8;

    let same_horizontal = src_rank == test_rank
        && dest_rank == test_rank
        && is_between(src_file, test_file, dest_file);

    let same_vertical = src_file == test_file
        && dest_file == test_file
        && is_between(src_rank, test_rank, dest_rank);

    same_horizontal || same_vertical
}

pub fn write_between(f: &mut File) -> Result<()> {
    writeln!(f, "const BETWEEN: [[BitBoard; 64]; 64] = [[")?;
    for i in 0..64 {
        for j in 0..64 {
            writeln!(f, "    BitBoard({}),", BETWEEN[i][j].0)?;
        }
        if i != 63 {
            writeln!(f, "  ], [")?;
        }
    }
    writeln!(f, "]];")?;
    Ok(())
}
