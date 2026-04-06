use super::magics::magic_mask;
use crate::bitboard::BitBoard;
use crate::pieces::Piece;
use crate::square::Square;

pub fn gen_blocker_combinations(mask: BitBoard) -> Vec<BitBoard> {
    let mut result = vec![];
    let squares: Vec<_> = mask.get_squares().collect();

    for i in 0..(1u64 << squares.len()) {
        let mut current = BitBoard(0);
        for (j, sq) in squares.iter().enumerate() {
            if (i & (1u64 << j)) == (1u64 << j) {
                current |= BitBoard::from_square(*sq);
            }
        }
        result.push(current);
    }

    result
}

pub fn gen_magic_attack_map(square: Square, piece: Piece) -> (Vec<BitBoard>, Vec<BitBoard>) {
    let occupancy_mask = magic_mask(square, piece);
    let blockers_combinations = gen_blocker_combinations(occupancy_mask);
    let mut attacks = Vec::new();

    let directions: Vec<fn(Square) -> Option<_>> = match piece {
        Piece::Rook => vec![|s| s.left(), |s| s.right(), |s| s.up(), |s| s.down()],
        Piece::Bishop => vec![
            |s| s.left().and_then(|s| s.up()),
            |s| s.right().and_then(|s| s.up()),
            |s| s.left().and_then(|s| s.down()),
            |s| s.right().and_then(|s| s.down()),
        ],
        _ => panic!("Magic only for Rooks and Bishops"),
    };

    for blockers in blockers_combinations.iter() {
        let mut attack_mask = BitBoard(0);
        for dir in directions.iter() {
            let mut next_square = dir(square);
            while let Some(curr_sq) = next_square {
                attack_mask ^= BitBoard::from_square(curr_sq);
                if (BitBoard::from_square(curr_sq) & *blockers) != BitBoard(0) {
                    break;
                }
                next_square = dir(curr_sq);
            }
        }
        attacks.push(attack_mask);
    }

    (blockers_combinations, attacks)
}
