use std::fs::File;
use std::io::Write;
use std::sync::LazyLock;

use crate::bitboard::BitBoard;
use crate::color::Color;
use crate::file::ALL_FILES;
use crate::rank::Rank;
use crate::square::Square;

static PAWN_MOVES: LazyLock<[[BitBoard; 64]; 2]> = LazyLock::new(|| {
    let mut pawn_moves = [[BitBoard(0); 64]; 2];
    for color in [Color::White, Color::Black] {
        for square in Square::all_squares() {
            if square.get_rank() == color.starting_rank().forward(color) {
                pawn_moves[color.to_index()][square.to_index()] =
                    BitBoard::from_square(square.forward(color).unwrap())
                        ^ BitBoard::from_square(
                            square.forward(color).unwrap().forward(color).unwrap(),
                        );
            } else {
                match square.forward(color) {
                    None => pawn_moves[color.to_index()][square.to_index()] = BitBoard(0),
                    Some(x) => {
                        pawn_moves[color.to_index()][square.to_index()] = BitBoard::from_square(x)
                    }
                };
            }
        }
    }
    pawn_moves
});
static PAWN_ATTACKS: LazyLock<[[BitBoard; 64]; 2]> = LazyLock::new(|| {
    let mut pawn_attacks = [[BitBoard(0); 64]; 2];
    for color in [Color::White, Color::Black] {
        for square in Square::all_squares() {
            pawn_attacks[color.to_index()][square.to_index()] = BitBoard(0);

            if let Some(f) = square.forward(color) {
                if let Some(fl) = f.left() {
                    pawn_attacks[color.to_index()][square.to_index()] ^= BitBoard::from_square(fl)
                };
                if let Some(fr) = f.right() {
                    pawn_attacks[color.to_index()][square.to_index()] ^= BitBoard::from_square(fr)
                };
            };
        }
    }
    pawn_attacks
});

fn source_double_moves() -> BitBoard {
    let mut result = BitBoard(0);
    for rank in [Rank::Second, Rank::Seventh] {
        for file in ALL_FILES.iter() {
            result ^= BitBoard::set(rank, *file);
        }
    }
    result
}

fn dest_double_moves() -> BitBoard {
    let mut result = BitBoard(0);
    for rank in [Rank::Fourth, Rank::Fifth] {
        for file in ALL_FILES.iter() {
            result ^= BitBoard::set(rank, *file);
        }
    }
    result
}

pub fn write_pawn_moves(f: &mut File) -> std::io::Result<()> {
    writeln!(f, "const PAWN_MOVES: [[BitBoard; 64]; 2] = [[")?;
    for white_move in PAWN_MOVES[0].iter() {
        writeln!(f, "    BitBoard({}),", white_move.0)?;
    }
    writeln!(f, "  ], [")?;

    for black_move in PAWN_MOVES[1].iter() {
        writeln!(f, "    BitBoard({}),", black_move.0)?;
    }
    writeln!(f, "]];")?;

    Ok(())
}

pub fn write_pawn_attacks(f: &mut File) -> std::io::Result<()> {
    writeln!(f, "const PAWN_ATTACKS: [[BitBoard; 64]; 2] = [[")?;
    for white_attack in PAWN_ATTACKS[0].iter() {
        writeln!(f, "    BitBoard({}),", white_attack.0)?;
    }
    writeln!(f, "  ], [")?;

    for black_attack in PAWN_ATTACKS[1].iter() {
        writeln!(f, "    BitBoard({}),", black_attack.0)?;
    }
    writeln!(f, "]];")?;

    writeln!(
        f,
        "const PAWN_SOURCE_DOUBLE_MOVES: BitBoard = BitBoard({0});",
        source_double_moves().0
    )?;

    writeln!(
        f,
        "const PAWN_DEST_DOUBLE_MOVES: BitBoard = BitBoard({0});",
        dest_double_moves().0
    )?;

    Ok(())
}
