use chesslib::board::Board;
use chesslib::color::Color;
use chesslib::pieces::Piece;

/// Centipawn value for each piece type (King has no value — it can't be captured).
const PIECE_VALUES: [i32; 6] = [
    100, // Pawn
    320, // Knight
    330, // Bishop
    500, // Rook
    900, // Queen
    0,   // King
];

/// Bit mask for each file (a through h).
const FILE_MASKS: [u64; 8] = [
    0x0101_0101_0101_0101,
    0x0202_0202_0202_0202,
    0x0404_0404_0404_0404,
    0x0808_0808_0808_0808,
    0x1010_1010_1010_1010,
    0x2020_2020_2020_2020,
    0x4040_4040_4040_4040,
    0x8080_8080_8080_8080,
];

/// Progressive passed-pawn bonus indexed by the pawn rank from White's perspective.
///
/// White uses the rank directly; Black uses the mirrored rank (`7 - rank`).
const PASSED_PAWN_BONUS_BY_WHITE_RANK: [i32; 8] = [
    0,   // rank 1
    0,   // rank 2 (starting rank, no bonus)
    10,  // rank 3
    20,  // rank 4
    35,  // rank 5
    60,  // rank 6
    100, // rank 7
    0,   // rank 8 (promotion would already have happened)
];

/// Evaluate the position from the side-to-move's perspective.
///
/// Returns:
/// - A positive score when the side-to-move has a material advantage.
/// - A negative score when the side-to-move is behind.
/// - `-30000` when the side-to-move is in checkmate (they are losing).
/// - `0` when the position is stalemate.
pub fn evaluate(board: &Board) -> i32 {
    if board.is_checkmate() {
        return -30000;
    }
    if board.is_stalemate() {
        return 0;
    }

    let stm = board.side_to_move();
    let opp = !stm;

    let mut score = 0i32;
    for (i, &value) in PIECE_VALUES.iter().enumerate() {
        let piece = match i {
            0 => Piece::Pawn,
            1 => Piece::Knight,
            2 => Piece::Bishop,
            3 => Piece::Rook,
            4 => Piece::Queen,
            5 => Piece::King,
            _ => unreachable!(),
        };

        let stm_count = (board.get_piece_bitboard(piece) & board.get_color_bitboard(stm))
            .0
            .count_ones() as i32;
        let opp_count = (board.get_piece_bitboard(piece) & board.get_color_bitboard(opp))
            .0
            .count_ones() as i32;

        score += value * (stm_count - opp_count);
    }

    score += passed_pawn_score(board, stm);
    score -= passed_pawn_score(board, opp);

    score
}

#[inline(always)]
fn passed_pawn_score(board: &Board, color: Color) -> i32 {
    let own_pawns = (board.get_piece_bitboard(Piece::Pawn) & board.get_color_bitboard(color)).0;
    let enemy_pawns = (board.get_piece_bitboard(Piece::Pawn) & board.get_color_bitboard(!color)).0;

    let mut total = 0;
    let mut pawns = own_pawns;

    while pawns != 0 {
        let square_idx = pawns.trailing_zeros() as usize;
        pawns &= pawns - 1;

        if is_passed_pawn(square_idx, enemy_pawns, color) {
            total += passed_pawn_bonus(square_idx, color);
        }
    }

    total
}

#[inline(always)]
fn is_passed_pawn(square_idx: usize, enemy_pawns: u64, color: Color) -> bool {
    let file = square_idx & 7;
    let rank = square_idx >> 3;

    let mut relevant_files = FILE_MASKS[file];
    if file > 0 {
        relevant_files |= FILE_MASKS[file - 1];
    }
    if file < 7 {
        relevant_files |= FILE_MASKS[file + 1];
    }

    let forward_ranks = match color {
        Color::White => {
            if rank == 7 {
                0
            } else {
                !0u64 << ((rank + 1) * 8)
            }
        }
        Color::Black => {
            if rank == 0 {
                0
            } else {
                (1u64 << (rank * 8)) - 1
            }
        }
    };

    (enemy_pawns & relevant_files & forward_ranks) == 0
}

#[inline(always)]
fn passed_pawn_bonus(square_idx: usize, color: Color) -> i32 {
    let rank = square_idx >> 3;
    let white_pov_rank = match color {
        Color::White => rank,
        Color::Black => 7 - rank,
    };

    PASSED_PAWN_BONUS_BY_WHITE_RANK[white_pov_rank]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn starting_position_is_zero() {
        let board =
            Board::from_str("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
        assert_eq!(evaluate(&board), 0);
    }

    #[test]
    fn missing_white_queen_is_negative_for_white() {
        // White is missing a queen — white to move should see a negative score
        let board =
            Board::from_str("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNB1KBNR w KQkq - 0 1").unwrap();
        let score = evaluate(&board);
        assert!(score < 0, "Expected negative score, got {}", score);
        assert_eq!(score, -900);
    }

    #[test]
    fn missing_black_queen_is_positive_for_white() {
        // Black is missing a queen — white to move should see a positive score
        let board =
            Board::from_str("rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
        let score = evaluate(&board);
        assert!(score > 0, "Expected positive score, got {}", score);
        assert_eq!(score, 900);
    }

    #[test]
    fn checkmate_returns_extreme_negative() {
        // Fool's mate — Black has delivered checkmate. White (side to move) is mated.
        // After: 1. f3 e5 2. g4 Qh4#
        let board =
            Board::from_str("rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3")
                .unwrap();
        assert_eq!(evaluate(&board), -30000);
    }

    #[test]
    fn stalemate_returns_zero() {
        // Classic queen stalemate: Black king on a8, White king on c7, White queen on b6
        // Black to move — no legal moves, not in check
        let board = Board::from_str("k7/2K5/1Q6/8/8/8/8/8 b - - 0 1").unwrap();
        assert_eq!(evaluate(&board), 0);
    }

    fn pawn_square(board: &Board, color: Color) -> usize {
        let pawns = (board.get_piece_bitboard(Piece::Pawn) & board.get_color_bitboard(color)).0;
        assert_eq!(pawns.count_ones(), 1, "expected exactly one pawn");
        pawns.trailing_zeros() as usize
    }

    #[test]
    fn identifies_white_passed_pawn() {
        let board = Board::from_str("4k3/8/8/4P3/8/8/8/4K3 w - - 0 1").unwrap();
        let white_pawn = pawn_square(&board, Color::White);
        assert!(is_passed_pawn(white_pawn, 0, Color::White));
    }

    #[test]
    fn identifies_black_passed_pawn() {
        let board = Board::from_str("4k3/8/8/8/3p4/8/8/4K3 b - - 0 1").unwrap();
        let black_pawn = pawn_square(&board, Color::Black);
        assert!(is_passed_pawn(black_pawn, 0, Color::Black));
    }

    #[test]
    fn no_passed_pawn_when_enemy_pawn_is_ahead_on_same_file() {
        let board = Board::from_str("4k3/8/4p3/4P3/8/8/8/4K3 w - - 0 1").unwrap();
        let white_pawn = pawn_square(&board, Color::White);
        let black_pawns =
            (board.get_piece_bitboard(Piece::Pawn) & board.get_color_bitboard(Color::Black)).0;

        assert!(!is_passed_pawn(white_pawn, black_pawns, Color::White));
        assert_eq!(passed_pawn_score(&board, Color::White), 0);
    }

    #[test]
    fn no_passed_pawn_when_enemy_pawn_is_ahead_on_adjacent_file() {
        let board = Board::from_str("4k3/8/3p4/4P3/8/8/8/4K3 w - - 0 1").unwrap();
        let white_pawn = pawn_square(&board, Color::White);
        let black_pawns =
            (board.get_piece_bitboard(Piece::Pawn) & board.get_color_bitboard(Color::Black)).0;

        assert!(!is_passed_pawn(white_pawn, black_pawns, Color::White));
        assert_eq!(passed_pawn_score(&board, Color::White), 0);
    }

    #[test]
    fn more_advanced_passed_pawn_gets_larger_bonus() {
        let less_advanced = Board::from_str("4k3/8/8/8/4P3/8/8/4K3 w - - 0 1").unwrap();
        let more_advanced = Board::from_str("4k3/8/4P3/8/8/8/8/4K3 w - - 0 1").unwrap();

        assert!(
            evaluate(&more_advanced) > evaluate(&less_advanced),
            "more advanced passed pawn should score higher"
        );
    }

    #[test]
    fn evaluation_preserves_side_to_move_perspective() {
        let white_to_move = Board::from_str("4k3/8/4P3/8/8/8/8/4K3 w - - 0 1").unwrap();
        let black_to_move = Board::from_str("4k3/8/4P3/8/8/8/8/4K3 b - - 0 1").unwrap();

        let white_score = evaluate(&white_to_move);
        let black_score = evaluate(&black_to_move);

        assert!(white_score > 0, "white to move should prefer this position");
        assert!(black_score < 0, "black to move should dislike this position");
        assert_eq!(white_score, -black_score);
    }
}