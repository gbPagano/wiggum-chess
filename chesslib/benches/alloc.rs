use divan::{AllocProfiler, Bencher, black_box};
use chesslib::board::Board;
use chesslib::movegen::MoveGen;

#[global_allocator]
static ALLOC: AllocProfiler = AllocProfiler::system();

fn main() {
    divan::main();
}

#[divan::bench(sample_size = 1, sample_count = 1)]
fn perft_6_alloc(bencher: Bencher) {
    let board = Board::default();

    bencher.bench_local(move || {
        MoveGen::perft_test(black_box(&board), black_box(6));
    })
}
