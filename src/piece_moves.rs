use crate::bitboard::BitBoard;
use crate::board::Board;
use crate::color::Color;
use crate::pieces::Piece;
use crate::square::Square;

use super::movegen::{BitBoardMove, MoveList};

use crate::magic;

pub trait AsPiece {
    const PIECE: Piece;
}

pub struct InCheck;
pub struct NotInCheck;

pub trait CheckStatus {
    const IN_CHECK: bool;
}

impl CheckStatus for InCheck {
    const IN_CHECK: bool = true;
}

impl CheckStatus for NotInCheck {
    const IN_CHECK: bool = false;
}

pub trait PieceMoves: AsPiece {
    fn pseudo_legals(square: Square, color: Color, combined: BitBoard, mask: BitBoard) -> BitBoard;

    #[inline(always)]
    fn legals<T: CheckStatus>(movelist: &mut MoveList, board: &Board, mask: BitBoard) {
        let combined = board.get_combined_bitboard();
        let color = board.side_to_move();
        let my_pieces = board.get_color_bitboard(color);
        let king_square = board.get_king_square(color);

        let pieces = board.get_piece_bitboard(Self::PIECE) & my_pieces;
        let pinned = board.get_pinned_bitboard();
        let checkers = board.get_checkers_bitboard();

        let check_mask = if T::IN_CHECK {
            magic::get_between(checkers.to_square(), king_square) ^ checkers
        } else {
            !BitBoard(0) // full bitboard
        };

        for square in (pieces & !pinned).get_squares() {
            let moves = Self::pseudo_legals(square, color, combined, mask) & check_mask;
            if !moves.is_empty() {
                unsafe {
                    movelist.push(BitBoardMove::new(square, moves, false));
                }
            }
        }

        if !T::IN_CHECK {
            for square in (pieces & pinned).get_squares() {
                let moves = Self::pseudo_legals(square, color, combined, mask)
                    & magic::get_line(square, king_square);
                if !moves.is_empty() {
                    unsafe {
                        movelist.push(BitBoardMove::new(square, moves, false));
                    }
                }
            }
        }
    }
}

pub struct RookMoves;
impl PieceMoves for RookMoves {
    #[inline(always)]
    fn pseudo_legals(sq: Square, _: Color, combined: BitBoard, mask: BitBoard) -> BitBoard {
        magic::get_rook_moves(sq, combined) & mask
    }
}
impl AsPiece for RookMoves {
    const PIECE: Piece = Piece::Rook;
}

pub struct BishopMoves;
impl PieceMoves for BishopMoves {
    #[inline(always)]
    fn pseudo_legals(sq: Square, _: Color, combined: BitBoard, mask: BitBoard) -> BitBoard {
        magic::get_bishop_moves(sq, combined) & mask
    }
}
impl AsPiece for BishopMoves {
    const PIECE: Piece = Piece::Bishop;
}

pub struct QueenMoves;
impl PieceMoves for QueenMoves {
    #[inline(always)]
    fn pseudo_legals(sq: Square, _: Color, combined: BitBoard, mask: BitBoard) -> BitBoard {
        (magic::get_rook_moves(sq, combined) ^ magic::get_bishop_moves(sq, combined)) & mask
    }
}
impl AsPiece for QueenMoves {
    const PIECE: Piece = Piece::Queen;
}

pub struct KnightMoves;
impl PieceMoves for KnightMoves {
    #[inline(always)]
    fn pseudo_legals(sq: Square, _: Color, _combined: BitBoard, mask: BitBoard) -> BitBoard {
        magic::get_knight_moves(sq) & mask
    }
}
impl AsPiece for KnightMoves {
    const PIECE: Piece = Piece::Knight;
}

pub struct PawnMoves;
impl PawnMoves {
    #[inline(always)]
    pub fn legal_ep_move(board: &Board, source: Square, dest: Square) -> bool {
        let captured_pawn = board
            .en_passant()
            .unwrap()
            .forward(!board.side_to_move())
            .unwrap();

        let combined = board.get_combined_bitboard()
            ^ BitBoard::from_square(captured_pawn)
            ^ BitBoard::from_square(source)
            ^ BitBoard::from_square(dest);

        let king_square = board.get_king_square(board.side_to_move());

        let enemy_rooks = (board.get_piece_bitboard(Piece::Rook)
            | board.get_piece_bitboard(Piece::Queen))
            & board.get_color_bitboard(!board.side_to_move());

        if !(magic::get_rook_rays(king_square) & enemy_rooks).is_empty()
            && !(magic::get_rook_moves(king_square, combined) & enemy_rooks).is_empty()
        {
            return false;
        }

        let enemy_bishops = (board.get_piece_bitboard(Piece::Bishop)
            | board.get_piece_bitboard(Piece::Queen))
            & board.get_color_bitboard(!board.side_to_move());

        if !(magic::get_bishop_rays(king_square) & enemy_bishops).is_empty()
            && !(magic::get_bishop_moves(king_square, combined) & enemy_bishops).is_empty()
        {
            return false;
        }

        true
    }
}
impl PieceMoves for PawnMoves {
    #[inline(always)]
    fn pseudo_legals(sq: Square, color: Color, combined: BitBoard, mask: BitBoard) -> BitBoard {
        magic::get_pawn_moves(sq, color, combined) & mask
    }

    #[inline(always)]
    fn legals<T: CheckStatus>(movelist: &mut MoveList, board: &Board, mask: BitBoard) {
        let combined = board.get_combined_bitboard();
        let color = board.side_to_move();
        let my_pieces = board.get_color_bitboard(color);
        let king_square = board.get_king_square(color);

        let pieces = board.get_piece_bitboard(Self::PIECE) & my_pieces;
        let pinned = board.get_pinned_bitboard();
        let checkers = board.get_checkers_bitboard();

        let check_mask = if T::IN_CHECK {
            magic::get_between(checkers.to_square(), king_square) ^ checkers
        } else {
            !BitBoard(0) // full bitboard
        };

        for square in (pieces & !pinned).get_squares() {
            let moves = Self::pseudo_legals(square, color, combined, mask) & check_mask;
            if !moves.is_empty() {
                unsafe {
                    movelist.push(BitBoardMove::new(
                        square,
                        moves,
                        square.get_rank() == color.pre_promotion_rank(),
                    ));
                }
            }
        }

        if !T::IN_CHECK {
            for square in (pieces & pinned).get_squares() {
                let moves = Self::pseudo_legals(square, color, combined, mask)
                    & magic::get_line(king_square, square);
                if !moves.is_empty() {
                    unsafe {
                        movelist.push(BitBoardMove::new(
                            square,
                            moves,
                            square.get_rank() == color.pre_promotion_rank(),
                        ));
                    }
                }
            }
        }

        if let Some(ep_square) = board.en_passant() {
            let rank = magic::get_rank_bitboard(ep_square.get_rank().forward(!color));
            let files = magic::get_adjacent_files(ep_square.get_file());
            for square in (rank & files & pieces).get_squares() {
                if PawnMoves::legal_ep_move(board, square, ep_square) {
                    unsafe {
                        movelist.push(BitBoardMove::new(
                            square,
                            BitBoard::from_square(ep_square),
                            false,
                        ));
                    }
                }
            }
        }
    }
}
impl AsPiece for PawnMoves {
    const PIECE: Piece = Piece::Pawn;
}

pub struct KingMoves;
impl KingMoves {
    #[inline(always)]
    pub fn legal_move(board: &Board, dest: Square) -> bool {
        let combined = board.get_combined_bitboard()
            ^ (board.get_piece_bitboard(Piece::King)
                & board.get_color_bitboard(board.side_to_move()))
            | BitBoard::from_square(dest);

        let mut attackers = BitBoard(0);

        let enemy_rooks = (board.get_piece_bitboard(Piece::Rook)
            | board.get_piece_bitboard(Piece::Queen))
            & board.get_color_bitboard(!board.side_to_move());
        attackers |= magic::get_rook_moves(dest, combined) & enemy_rooks;

        let enemy_bishops = (board.get_piece_bitboard(Piece::Bishop)
            | board.get_piece_bitboard(Piece::Queen))
            & board.get_color_bitboard(!board.side_to_move());
        attackers |= magic::get_bishop_moves(dest, combined) & enemy_bishops;

        let knight_rays = magic::get_knight_moves(dest);
        attackers |= knight_rays
            & board.get_piece_bitboard(Piece::Knight)
            & board.get_color_bitboard(!board.side_to_move());

        let king_rays = magic::get_king_moves(dest);
        attackers |= king_rays
            & board.get_piece_bitboard(Piece::King)
            & board.get_color_bitboard(!board.side_to_move());

        attackers |= magic::get_pawn_attacks(
            dest,
            board.side_to_move(),
            board.get_piece_bitboard(Piece::Pawn) & board.get_color_bitboard(!board.side_to_move()),
        );

        attackers.is_empty()
    }
}
impl PieceMoves for KingMoves {
    #[inline(always)]
    fn pseudo_legals(sq: Square, _: Color, _combined: BitBoard, mask: BitBoard) -> BitBoard {
        magic::get_king_moves(sq) & mask
    }

    #[inline(always)]
    fn legals<T: CheckStatus>(movelist: &mut MoveList, board: &Board, mask: BitBoard) {
        let combined = board.get_combined_bitboard();
        let color = board.side_to_move();
        let king_square = board.get_king_square(color);

        let mut moves = Self::pseudo_legals(king_square, color, combined, mask);
        for dest in moves.get_squares() {
            if !KingMoves::legal_move(board, dest) {
                moves ^= BitBoard::from_square(dest);
            }
        }

        if !T::IN_CHECK {
            if board.castle_rights().has_kingside(color)
                && (combined & board.castle_rights().kingside_squares(color)).is_empty()
            {
                let first = king_square.right().unwrap();
                let second = first.right().unwrap();
                if KingMoves::legal_move(board, first) && KingMoves::legal_move(board, second) {
                    moves ^= BitBoard::from_square(second);
                }
            }

            if board.castle_rights().has_queenside(color)
                && (combined & board.castle_rights().queenside_squares(color)).is_empty()
            {
                let first = king_square.left().unwrap();
                let second = first.left().unwrap();
                if KingMoves::legal_move(board, first) && KingMoves::legal_move(board, second) {
                    moves ^= BitBoard::from_square(second);
                }
            }
        }

        if !moves.is_empty() {
            unsafe {
                movelist.push(BitBoardMove::new(king_square, moves, false));
            }
        }
    }
}
impl AsPiece for KingMoves {
    const PIECE: Piece = Piece::King;
}
