use std::fs::File;
use std::io::Write;

fn xorshift64(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

/// Write Zobrist hash tables to the generated magic file.
///
/// Layout:
/// - `ZOBRIST_PIECES[piece * 128 + color * 64 + square]` — 6 pieces × 2 colors × 64 squares = 768 keys
/// - `ZOBRIST_CASTLING[0..4]` — white_kingside, white_queenside, black_kingside, black_queenside
/// - `ZOBRIST_EN_PASSANT[0..8]` — indexed by file (0=A .. 7=H)
/// - `ZOBRIST_SIDE: u64` — XOR when it is Black's turn to move
pub fn write_zobrist(f: &mut File) -> std::io::Result<()> {
    // Fixed seed for reproducibility across builds
    let mut state = 0x9E3779B97F4A7C15u64;

    // 768 piece/color/square keys
    writeln!(f, "const ZOBRIST_PIECES: [u64; 768] = [")?;
    for _ in 0..768 {
        writeln!(f, "    {},", xorshift64(&mut state))?;
    }
    writeln!(f, "];")?;

    // 4 castling rights
    writeln!(f, "const ZOBRIST_CASTLING: [u64; 4] = [")?;
    for _ in 0..4 {
        writeln!(f, "    {},", xorshift64(&mut state))?;
    }
    writeln!(f, "];")?;

    // 8 en-passant file keys
    writeln!(f, "const ZOBRIST_EN_PASSANT: [u64; 8] = [")?;
    for _ in 0..8 {
        writeln!(f, "    {},", xorshift64(&mut state))?;
    }
    writeln!(f, "];")?;

    // Side-to-move key (XOR when Black to move)
    writeln!(f, "const ZOBRIST_SIDE: u64 = {};", xorshift64(&mut state))?;

    Ok(())
}
