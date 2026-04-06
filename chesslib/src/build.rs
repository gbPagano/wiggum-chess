#![allow(dead_code)]

use anyhow::Result;
use std::env;
use std::fs::File;
use std::path::Path;

mod bitboard;
mod color;
mod file;
mod gen_files;
mod pieces;
mod rank;
mod square;

use gen_files::*;

fn main() -> Result<()> {
    gen_all_magic();

    let out_dir = env::var("OUT_DIR").unwrap();
    let magic_path = Path::new(&out_dir).join("magic_file.rs");
    let mut f = File::create(&magic_path).unwrap();

    write_rays(&mut f);
    write_magics(&mut f);
    write_between(&mut f)?;
    write_lines(&mut f)?;
    write_knight_moves(&mut f)?;
    write_king_moves(&mut f)?;
    write_pawn_moves(&mut f)?;
    write_pawn_attacks(&mut f)?;
    write_chessboard_utils(&mut f)?;

    Ok(())
}
