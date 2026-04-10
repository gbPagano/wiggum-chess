use clap::{Parser, ValueEnum};
use shakmaty::{CastlingMode, Chess as ShakmatyChess, fen::Fen};

#[derive(Parser)]
#[command(name = "perft-bench", about = "Perft benchmark for chess move generator libraries")]
struct Args {
    /// Which library to benchmark
    #[arg(long, short)]
    engine: Engine,

    /// Perft depth
    #[arg(long, short)]
    depth: usize,

    /// FEN position (defaults to starting position)
    #[arg(long, default_value = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")]
    fen: String,
}

#[derive(ValueEnum, Clone)]
enum Engine {
    /// Bitboard-based chesslib (magic bitboards)
    Chesslib,
    /// Simple mailbox-based chesslib
    ChesslibSimple,
    /// The `chess` crate by jordanbray
    Chess,
    /// The `shakmaty` crate by niklasf
    Shakmaty,
}

fn perft_chesslib(fen: &str, depth: usize) -> usize {
    let board: chesslib::board::Board = fen.parse().expect("invalid FEN");
    chesslib::movegen::MoveGen::perft_test(&board, depth)
}

fn perft_simple(fen: &str, depth: usize) -> u64 {
    let board = chesslib_simple::Board::from_fen(fen).expect("invalid FEN");
    board.perft(depth)
}

fn perft_chess_crate(fen: &str, depth: usize) -> usize {
    let board: chess::Board = fen.parse().expect("invalid FEN");
    chess::MoveGen::movegen_perft_test(&board, depth)
}

fn perft_shakmaty(fen: &str, depth: usize) -> u64 {
    let fen: Fen = fen.parse().expect("invalid FEN");
    let position: ShakmatyChess = fen
        .into_position(CastlingMode::Standard)
        .expect("invalid FEN position");
    shakmaty::perft(&position, depth as u32)
}

fn main() {
    let args = Args::parse();

    let nodes: u64 = match args.engine {
        Engine::Chesslib => perft_chesslib(&args.fen, args.depth) as u64,
        Engine::ChesslibSimple => perft_simple(&args.fen, args.depth),
        Engine::Chess => perft_chess_crate(&args.fen, args.depth) as u64,
        Engine::Shakmaty => perft_shakmaty(&args.fen, args.depth),
    };

    println!("{nodes}");
}
