# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`chesslib` is a Rust chess move generator library focused on performance. It uses bitboard representation with magic bitboard sliding piece attack generation. The main binary runs a perft (performance test) to validate correctness and measure move generation speed.

## Build & Test Commands

```bash
cargo build                          # debug build (still opt-level=3)
cargo build --release                # release build (LTO, single codegen unit)
cargo test                           # run all tests
cargo test movegen_perft_5           # run a single test by name
cargo run -- 6                       # run perft to depth 6 (default)
cargo bench --bench perft            # criterion perft benchmarks (depths 1-7)
cargo bench --bench alloc            # allocation benchmarks (divan)
```

## Architecture

### Build-time Code Generation (`src/build.rs`, `src/gen_files/`)

The build script generates lookup tables into `$OUT_DIR/magic_file.rs`, which is `include!()`'d by `src/magic.rs`. Generated tables include: magic numbers for sliding pieces, rays, between/line bitboards, knight/king/pawn move tables, and chessboard utility constants. The `src/gen_files/` modules contain the generation logic and are compiled separately for the build script (they re-declare shared types like `bitboard`, `square`, etc.).

### Core Types

- **`BitBoard`** (`src/bitboard.rs`) — `u64` wrapper representing a set of squares. All piece positions and move sets are bitboards.
- **`Board`** (`src/board.rs`) — Full board state: piece/color/combined bitboards, side to move, castling rights, en passant, and precomputed pinned/checkers bitboards. Immutable move application via `make_move()` (returns new Board). Parses/displays FEN strings.
- **`Square`**, **`Rank`**, **`File`**, **`Color`**, **`Piece`** — Small enum/newtype wrappers with index conversions.

### Move Generation (`src/piece_moves.rs`, `src/movegen.rs`)

Legal move generation uses a trait-based design:
- `PieceMoves` trait with `pseudo_legals()` and `legals()` methods, implemented per piece type (`RookMoves`, `BishopMoves`, etc.).
- `CheckStatus` trait (`InCheck`/`NotInCheck`) used as a const generic parameter to specialize check-aware move generation at compile time.
- Pinned pieces are restricted to movement along their pin line. In double check, only king moves are generated.
- `MoveGen` is an iterator over `ChessMove` that lazily expands `BitBoardMove` entries (bitboard of destinations per source square).

### Magic Bitboards (`src/magic.rs`)

Sliding piece attacks (rook/bishop) use magic bitboard lookup. `get_rook_moves()`/`get_bishop_moves()` use unsafe unchecked indexing for performance. All other piece move lookups are also unsafe table reads.

## Testing

Correctness is validated through perft tests (`src/movegen.rs` tests) — comparing node counts at various depths against known-correct values for many positions. These are the primary correctness tests.
