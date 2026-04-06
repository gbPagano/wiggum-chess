use std::fs;
use std::io::Write;
use std::sync::LazyLock;

use crate::bitboard::BitBoard;
use crate::file::File;
use crate::rank::Rank;
use crate::square::Square;

static RANKS: LazyLock<[BitBoard; 8]> = LazyLock::new(|| {
    let mut ranks = [BitBoard(0); 8];
    for (idx, rank) in ranks.iter_mut().enumerate() {
        *rank = Square::all_squares()
            .filter(|x| x.get_rank().to_index() == idx)
            .fold(BitBoard(0), |v, s| v | BitBoard::from_square(s));
    }
    ranks
});

//static FILES: LazyLock<[BitBoard; 8]> = LazyLock::new(|| {
//    let mut files = [BitBoard(0); 8];
//    for i in 0..8 {
//        files[i] = Square::all_squares()
//            .filter(|x| x.get_file().to_index() == i)
//            .fold(BitBoard(0), |v, s| v | BitBoard::from_square(s));
//    }
//    files
//});

static ADJACENT_FILES: LazyLock<[BitBoard; 8]> = LazyLock::new(|| {
    let mut adjacent_files = [BitBoard(0); 8];
    for (idx, file) in adjacent_files.iter_mut().enumerate() {
        *file = Square::all_squares()
            .filter(|y| {
                ((y.get_file().to_index() as i8) == (idx as i8) - 1)
                    || ((y.get_file().to_index() as i8) == (idx as i8) + 1)
            })
            .fold(BitBoard(0), |v, s| v | BitBoard::from_square(s));
    }
    adjacent_files
});

static EDGES: LazyLock<BitBoard> = LazyLock::new(|| {
    Square::all_squares()
        .filter(|x| {
            x.get_rank() == Rank::First
                || x.get_rank() == Rank::Eighth
                || x.get_file() == File::A
                || x.get_file() == File::H
        })
        .fold(BitBoard(0), |v, s| v | BitBoard::from_square(s))
});

pub fn write_chessboard_utils(f: &mut fs::File) -> std::io::Result<()> {
    //writeln!(f, "const FILES: [BitBoard; 8] = [")?;
    //for i in 0..8 {
    //    writeln!(f, "    BitBoard({}),", FILES[i].0)?;
    //}
    //writeln!(f, "];\n")?;
    writeln!(f, "const ADJACENT_FILES: [BitBoard; 8] = [")?;
    for file in ADJACENT_FILES.iter() {
        writeln!(f, "    BitBoard({}),", file.0)?;
    }
    writeln!(f, "];\n")?;
    writeln!(f, "const RANKS: [BitBoard; 8] = [")?;
    for rank in RANKS.iter() {
        writeln!(f, "    BitBoard({}),", rank.0)?;
    }
    writeln!(f, "];")?;
    writeln!(f, "pub const EDGES: BitBoard = BitBoard({});", EDGES.0)?;
    Ok(())
}
