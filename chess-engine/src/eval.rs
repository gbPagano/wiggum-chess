use chesslib::board::Board;
use chesslib::color::Color;
use chesslib::pieces::Piece;

/// Centipawn value for each piece type (King has no material value — it can't be captured).
const PIECE_VALUES: [i32; 6] = [
    100, // Pawn
    320, // Knight
    330, // Bishop
    500, // Rook
    900, // Queen
    0,   // King
];

const PIECES: [Piece; 6] = [
    Piece::Pawn,
    Piece::Knight,
    Piece::Bishop,
    Piece::Rook,
    Piece::Queen,
    Piece::King,
];

/// Piece-Square Tables (white perspective, indexed from a1 = 0 to h8 = 63).
/// Black pieces reuse the same tables with vertical mirroring.
///
/// These are simple opening/middlegame PSTs intended to add basic positional sense
/// without changing the current engine architecture.
const PAWN_PST: [i32; 64] = [
     0,   0,   0,   0,   0,   0,   0,   0,
     5,  10,  10, -20, -20,  10,  10,   5,
     5,  -5, -10,   0,   0, -10,  -5,   5,
     0,   0,   0,  20,  20,   0,   0,   0,
     5,   5,  10,  25,  25,  10,   5,   5,
    10,  10,  20,  30,  30,  20,  10,  10,
    50,  50,  50,  50,  50,  50,  50,  50,
     0,   0,   0,   0,   0,   0,   0,   0,
];

const KNIGHT_PST: [i32; 64] = [
   -50, -40, -30, -30, -30, -30, -40, -50,
   -40, -20,   0,   5,   5,   0, -20, -40,
   -30,   5,  10,  15,  15,  10,   5, -30,
   -30,   0,  15,  20,  20,  15,   0, -30,
   -30,   5,  15,  20,  20,  15,   5, -30,
   -30,   0,  10,  15,  15,  10,   0, -30,
   -40, -20,   0,   0,   0,   0, -20, -40,
   -50, -40, -30, -30, -30, -30, -40, -50,
];

const BISHOP_PST: [i32; 64] = [
   -20, -10, -10, -10, -10, -10, -10, -20,
   -10,   5,   0,   0,   0,   0,   5, -10,
   -10,  10,  10,  10,  10,  10,  10, -10,
   -10,   0,  10,  10,  10,  10,   0, -10,
   -10,   5,   5,  10,  10,   5,   5, -10,
   -10,   0,   5,  10,  10,   5,   0, -10,
   -10,   0,   0,   0,   0,   0,   0, -10,
   -20, -10, -10, -10, -10, -10, -10, -20,
];

const ROOK_PST: [i32; 64] = [
     0,   0,   0,   5,   5,   0,   0,   0,
    -5,   0,   0,   0,   0,   0,   0,  -5,
    -5,   0,   0,   0,   0,   0,   0,  -5,
    -5,   0,   0,   0,   0,   0,   0,  -5,
    -5,   0,   0,   0,   0,   0,   0,  -5,
    -5,   0,   0,   0,   0,   0,   0,  -5,
     5,  10,  10,  10,  10,  10,  10,   5,
     0,   0,   0,   0,   0,   0,   0,   0,
];

const QUEEN_PST: [i32; 64] = [
   -20, -10, -10,  -5,  -5, -10, -10, -20,
   -10,   0,   5,   0,   0,   0,   0, -10,
   -10,   5,   5,   5,   5,   5,   0, -10,
     0,   0,   5,   5,   5,   5,   0,  -5,
    -5,   0,   5,   5,   5,   5,   0,  -5,
   -10,   0,   5,   5,   5,   5,   0, -10,
   -10,   0,   0,   0,   0,   0,   0, -10,
   -20, -10, -10,  -5,  -5, -10, -10, -20,
];

const KING_PST: [i32; 64] = [
    20,  30,  10,   0,   0,  10,  30,  20,
    20,  20,   0,   0,   0,   0,  20,  20,
   -10, -20, -20, -20, -20, -20, -20, -10,
   -20, -30, -30, -40, -40, -30, -30, -20,
   -30, -40, -40, -50, -50, -40, -40, -30,
   -30, -40, -40, -50, -50, -40, -40, -30,
   -30, -40, -40, -50, -50, -40, -40, -30,
   -30, -40, -40, -50, -50, -40, -40, -30,
];

#[inline]
fn pst_for(piece: Piece) -> &'static [i32; 64] {
    match piece {
        Piece::Pawn => &PAWN_PST,
        Piece::Knight => &KNIGHT_PST,
        Piece::Bishop => &BISHOP_PST,
        Piece::Rook => &ROOK_PST,
        Piece::Queen => &QUEEN_PST,
        Piece::King => &KING_PST,
    }
}

#[inline]
fn mirror_square(square_index: usize) -> usize {
    square_index ^ 56
}

#[inline]
fn pst_value(piece: Piece, color: Color, square_index: usize) -> i32 {
    let index = match color {
        Color::White => square_index,
        Color::Black => mirror_square(square_index),
    };
    pst_for(piece)[index]
}

fn score_side(board: &Board, color: Color) -> i32 {
    let mut score = 0i32;

    for (&piece, &material_value) in PIECES.iter().zip(PIECE_VALUES.iter()) {
        let mut bb = (board.get_piece_bitboard(piece) & board.get_color_bitboard(color)).0;

        while bb != 0 {
            let square_index = bb.trailing_zeros() as usize;
            bb &= bb - 1;

            score += material_value;
            score += pst_value(piece, color, square_index);
        }
    }

    score
}

/// Evaluate the position from the side-to-move's perspective.
///
/// Returns:
/// - A positive score when the side-to-move has the better position.
/// - A negative score when the side-to-move is worse.
/// - `-30000` when the side-to-move is in checkmate (they are losing).
/// - `0` when the position is stalemate.
pub fn evaluate(board: &Board) -> i32 {
    if board.is_checkmate() {
        return -30000;
    }
    if board.is_stalemate() {
        return 0;
    }

    let white_score = score_side(board, Color::White);
    let black_score = score_side(board, Color::Black);
    let white_pov_score = white_score - black_score;

    match board.side_to_move() {
        Color::White => white_pov_score,
        Color::Black => -white_pov_score,
    }
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
        let board =
            Board::from_str("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNB1KBNR w KQkq - 0 1").unwrap();
        let score = evaluate(&board);
        assert!(score < 0, "Expected negative score, got {}", score);
        assert!(score < -800, "Expected material deficit to remain dominant, got {}", score);
    }

    #[test]
    fn missing_black_queen_is_positive_for_white() {
        let board =
            Board::from_str("rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
        let score = evaluate(&board);
        assert!(score > 0, "Expected positive score, got {}", score);
        assert!(score > 800, "Expected material edge to remain dominant, got {}", score);
    }

    #[test]
    fn centralized_white_knight_scores_better_than_corner_white_knight() {
        let center =
            Board::from_str("4k3/8/8/8/3N4/8/8/4K3 w - - 0 1").unwrap();
        let corner =
            Board::from_str("4k3/8/8/8/8/8/8/N3K3 w - - 0 1").unwrap();

        assert!(
            evaluate(&center) > evaluate(&corner),
            "Expected centralized knight to score better"
        );
    }

    #[test]
    fn centralized_black_knight_is_worse_for_white_than_corner_black_knight() {
        let center =
            Board::from_str("4k3/8/8/3n4/8/8/8/4K3 w - - 0 1").unwrap();
        let corner =
            Board::from_str("n3k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();

        assert!(
            evaluate(&center) < evaluate(&corner),
            "Expected centralized black knight to be evaluated as better for black"
        );
    }

    #[test]
    fn side_to_move_perspective_is_preserved_with_pst() {
        let white_to_move =
            Board::from_str("n3k3/8/8/8/3N4/8/8/4K3 w - - 0 1").unwrap();
        let black_to_move =
            Board::from_str("n3k3/8/8/8/3N4/8/8/4K3 b - - 0 1").unwrap();

        assert_eq!(evaluate(&white_to_move), -evaluate(&black_to_move));
    }

    #[test]
    fn checkmate_returns_extreme_negative() {
        let board =
            Board::from_str("rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3")
                .unwrap();
        assert_eq!(evaluate(&board), -30000);
    }

    #[test]
    fn stalemate_returns_zero() {
        let board = Board::from_str("k7/2K5/1Q6/8/8/8/8/8 b - - 0 1").unwrap();
        assert_eq!(evaluate(&board), 0);
    }
}