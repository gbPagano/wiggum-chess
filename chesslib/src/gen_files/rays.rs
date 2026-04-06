use std::fs::File;
use std::io::Write;
use std::sync::LazyLock;

use crate::bitboard::BitBoard;
use crate::pieces::Piece;
use crate::square::Square;

pub static ROOK_RAYS: LazyLock<[BitBoard; 64]> = LazyLock::new(|| {
    let mut rook_rays = [BitBoard(0); 64];
    for square in Square::all_squares() {
        let ray = Square::all_squares()
            .filter(|dest| {
                (square.get_rank() == dest.get_rank() || square.get_file() == dest.get_file())
                    && square != *dest
            })
            .fold(BitBoard(0), |bb, s| BitBoard::from_square(s) | bb);

        rook_rays[square.to_index()] = ray;
    }
    rook_rays
});
pub static BISHOP_RAYS: LazyLock<[BitBoard; 64]> = LazyLock::new(|| {
    let mut bishop_rays = [BitBoard(0); 64];
    for square in Square::all_squares() {
        bishop_rays[square.to_index()] = Square::all_squares()
            .filter(|dest| {
                let src_rank = square.get_rank().to_index() as i8;
                let src_file = square.get_file().to_index() as i8;
                let dest_rank = dest.get_rank().to_index() as i8;
                let dest_file = dest.get_file().to_index() as i8;

                (src_rank - dest_rank).abs() == (src_file - dest_file).abs() && square != *dest
            })
            .fold(BitBoard(0), |b, s| b | BitBoard::from_square(s));
    }
    bishop_rays
});

pub fn write_rays(f: &mut File) {
    writeln!(f, "const ROOK_RAYS: [BitBoard; 64] = [").unwrap();
    for ray in ROOK_RAYS.iter() {
        writeln!(f, "    BitBoard({}),", ray.0).unwrap();
    }
    writeln!(f, "];").unwrap();

    writeln!(f, "const BISHOP_RAYS: [BitBoard; 64] = [").unwrap();
    for ray in BISHOP_RAYS.iter() {
        writeln!(f, "    BitBoard({}),", ray.0).unwrap();
    }
    writeln!(f, "];").unwrap();
}

pub fn get_rays(square: Square, piece: Piece) -> BitBoard {
    match piece {
        Piece::Rook => ROOK_RAYS[square.to_index()],
        Piece::Bishop => BISHOP_RAYS[square.to_index()],
        _ => panic!("Rays only for Rooks and Bishops"),
    }
}
