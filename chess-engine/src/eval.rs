use chesslib::board::Board;
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
    score
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn starting_position_is_zero() {
        let board = Board::from_str("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
            .unwrap();
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
}
