use chesslib::board::Board;
use chesslib::movegen::MoveGen;
use std::env;
use std::time::Instant;

fn main() {
    let depth = env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(6);

    let board = Board::default();

    let start = Instant::now();
    let _ = MoveGen::perft_test(&board, depth);
    let duration = start.elapsed();
    println!("Perft {depth} in: {:?}", duration);
}
