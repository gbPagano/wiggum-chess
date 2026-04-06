use crate::bitboard::BitBoard;
use crate::color::Color;
use crate::file::File;
use crate::rank::Rank;
use crate::square::Square;

include!(concat!(env!("OUT_DIR"), "/magic_file.rs"));

#[inline(always)]
pub fn get_rook_moves(square: Square, blockers: BitBoard) -> BitBoard {
    unsafe {
        let magic: Magic = *MAGIC_NUMBERS
            .get_unchecked(0) // rook index
            .get_unchecked(square.to_index());
        *MOVES.get_unchecked(
            (magic.offset as usize)
                + ((magic.magic_number * (blockers & magic.mask)) >> magic.rightshift).0 as usize,
        ) & get_rook_rays(square)
    }
}

#[inline(always)]
pub fn get_bishop_moves(square: Square, blockers: BitBoard) -> BitBoard {
    unsafe {
        let magic: Magic = *MAGIC_NUMBERS
            .get_unchecked(1) // bishop index
            .get_unchecked(square.to_index());
        *MOVES.get_unchecked(
            (magic.offset as usize)
                + ((magic.magic_number * (blockers & magic.mask)) >> magic.rightshift).0 as usize,
        ) & get_bishop_rays(square)
    }
}

#[inline(always)]
pub fn get_bishop_rays(square: Square) -> BitBoard {
    unsafe { *BISHOP_RAYS.get_unchecked(square.to_index()) }
}

#[inline(always)]
pub fn get_rook_rays(square: Square) -> BitBoard {
    unsafe { *ROOK_RAYS.get_unchecked(square.to_index()) }
}

#[inline(always)]
pub fn get_line(sq_1: Square, sq_2: Square) -> BitBoard {
    unsafe {
        *LINES
            .get_unchecked(sq_1.to_index())
            .get_unchecked(sq_2.to_index())
    }
}

#[inline(always)]
pub fn get_between(sq_1: Square, sq_2: Square) -> BitBoard {
    unsafe {
        *BETWEEN
            .get_unchecked(sq_1.to_index())
            .get_unchecked(sq_2.to_index())
    }
}

#[inline(always)]
pub fn get_knight_moves(square: Square) -> BitBoard {
    unsafe { *KNIGHT_MOVES.get_unchecked(square.to_index()) }
}

#[inline(always)]
pub fn get_pawn_attacks(square: Square, color: Color, blockers: BitBoard) -> BitBoard {
    unsafe {
        *PAWN_ATTACKS
            .get_unchecked(color.to_index())
            .get_unchecked(square.to_index())
            & blockers
    }
}

#[inline(always)]
fn get_pawn_forward_moves(sq: Square, color: Color, blockers: BitBoard) -> BitBoard {
    unsafe {
        if !(BitBoard::from_square(sq.forward(color).unwrap()) & blockers).is_empty() {
            BitBoard(0)
        } else {
            *PAWN_MOVES
                .get_unchecked(color.to_index())
                .get_unchecked(sq.to_index())
                & !blockers
        }
    }
}

#[inline(always)]
pub fn get_pawn_moves(sq: Square, color: Color, blockers: BitBoard) -> BitBoard {
    get_pawn_attacks(sq, color, blockers) ^ get_pawn_forward_moves(sq, color, blockers)
}

#[inline(always)]
pub fn get_pawn_source_double_moves() -> BitBoard {
    PAWN_SOURCE_DOUBLE_MOVES
}

#[inline(always)]
pub fn get_pawn_dest_double_moves() -> BitBoard {
    PAWN_DEST_DOUBLE_MOVES
}

#[inline(always)]
pub fn get_rank_bitboard(rank: Rank) -> BitBoard {
    unsafe { *RANKS.get_unchecked(rank.to_index()) }
}

#[inline(always)]
pub fn get_adjacent_files(file: File) -> BitBoard {
    unsafe { *ADJACENT_FILES.get_unchecked(file.to_index()) }
}

#[inline(always)]
pub fn get_king_moves(sq: Square) -> BitBoard {
    unsafe { *KING_MOVES.get_unchecked(sq.to_index()) }
}

#[inline(always)]
pub fn get_castle_squares() -> BitBoard {
    CASTLE_SQUARES
}
