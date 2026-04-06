use criterion::{Criterion, SamplingMode, criterion_group, criterion_main};
use chesslib::board::Board;
use chesslib::movegen::MoveGen;
use std::time::Duration;

fn perft_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("Perft");
    let board = Board::default();

    // short perfts
    group
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(5));
    for depth in 1..=5 {
        group.bench_function(format!("{depth}"), |b| {
            b.iter(|| {
                MoveGen::perft_test(&board, depth);
            })
        });
    }

    // long perfts
    group
        .sample_size(10)
        .sampling_mode(SamplingMode::Flat)
        .measurement_time(Duration::from_secs(30));
    for depth in 6..=7 {
        group.bench_function(format!("{depth}"), |b| {
            b.iter(|| {
                MoveGen::perft_test(&board, depth);
            })
        });
    }
    group.finish();
}

criterion_group!(benches, perft_bench);
criterion_main!(benches);
