use chesslib::board::Board;
use chesslib::movegen::MoveGen;
use std::collections::HashMap;
use std::env;
use std::time::Instant;

fn main() {
    let depth = env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(6);

    let board = Board::default();

    let start = Instant::now();
    let _ = MoveGen::perft_test(&board, depth);
    let duration = start.elapsed();
    println!("Perft {depth} in: {:?}", duration);

    // let movements: HashMap<String, Vec<String>> =
    //     MoveGen::new_legal(&board).fold(HashMap::new(), |mut acc, m| {
    //         acc.entry(m.source.to_string())
    //             .or_default()
    //             .push(m.dest.to_string());
    //         acc
    //     });
    //
    // println!("{:?}", movements);
}
