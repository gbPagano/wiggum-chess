use anyhow::Result;
use clap::Parser;
use chesslib::chess_move::ChessMove;
use chesslib::clock::Clock;
use chesslib::engine::Engine;
use chesslib::game::{DrawReason, Game, GameResult};
use chesslib::match_runner::{Match, MatchObserver};
use chesslib::uci_engine::UciEngine;

#[derive(Parser)]
#[command(name = "chess-runner", about = "Run engine vs engine chess matches")]
struct Args {
    /// Path to the first engine executable
    #[arg(long)]
    engine1: String,

    /// Path to the second engine executable
    #[arg(long)]
    engine2: String,

    /// Time per player in milliseconds
    #[arg(long, default_value = "60000")]
    time: u64,

    /// Increment per move in milliseconds
    #[arg(long, default_value = "0")]
    inc: u64,

    /// Number of games to play
    #[arg(long, default_value = "1")]
    games: usize,

    /// Optional starting FEN position (applied to all games)
    #[arg(long)]
    start_fen: Option<String>,

    /// Engine response timeout in milliseconds
    #[arg(long, default_value = "5000")]
    timeout: u64,

    /// Optional path to CSV file for appending match results
    #[arg(long)]
    output: Option<String>,
}

/// Observer that prints moves and game-over events to stdout.
struct PrintObserver {
    game_number: usize,
}

impl MatchObserver for PrintObserver {
    fn on_move(&mut self, game: &Game, chess_move: ChessMove, engine_name: &str) {
        println!(
            "  Game {} move {}: {} plays {}",
            self.game_number,
            game.moves().len(),
            engine_name,
            chess_move.to_uci()
        );
        println!("{:?}", game.board());
    }

    fn on_game_over(&mut self, result: &GameResult) {
        println!("  Game {} result: {}", self.game_number, format_result(result));
    }
}

fn format_result(result: &GameResult) -> String {
    match result {
        GameResult::WhiteWins => "White wins (checkmate)".to_string(),
        GameResult::BlackWins => "Black wins (checkmate or flag)".to_string(),
        GameResult::Draw(reason) => format!("Draw ({})", format_draw_reason(reason)),
        GameResult::Ongoing => "Ongoing (error)".to_string(),
    }
}

fn format_draw_reason(reason: &DrawReason) -> &'static str {
    match reason {
        DrawReason::Stalemate => "stalemate",
        DrawReason::InsufficientMaterial => "insufficient material",
        DrawReason::ThreefoldRepetition => "threefold repetition",
        DrawReason::FivefoldRepetition => "fivefold repetition",
        DrawReason::FiftyMoveRule => "50-move rule",
        DrawReason::SeventyFiveMoveRule => "75-move rule",
    }
}

/// Write a match result row to a CSV file, creating it with headers if needed.
fn write_csv(
    path: &str,
    engine1_name: &str,
    engine2_name: &str,
    games_played: usize,
    engine1_wins: usize,
    engine2_wins: usize,
    draws: usize,
) -> Result<()> {
    let file_exists = std::path::Path::new(path).exists()
        && std::fs::metadata(path).map(|m| m.len() > 0).unwrap_or(false);

    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;

    let mut wtr = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(file);

    if !file_exists {
        wtr.write_record(&[
            "timestamp",
            "engine1_name",
            "engine2_name",
            "games_played",
            "engine1_wins",
            "engine2_wins",
            "draws",
            "engine1_win_rate",
        ])?;
    }

    let win_rate = if games_played > 0 {
        engine1_wins as f64 / games_played as f64
    } else {
        0.0
    };

    let timestamp = chrono::Utc::now().to_rfc3339();

    wtr.write_record(&[
        timestamp.as_str(),
        engine1_name,
        engine2_name,
        &games_played.to_string(),
        &engine1_wins.to_string(),
        &engine2_wins.to_string(),
        &draws.to_string(),
        &format!("{:.4}", win_rate),
    ])?;

    wtr.flush()?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    println!("Engine 1: {}", args.engine1);
    println!("Engine 2: {}", args.engine2);
    println!(
        "Time control: {}ms + {}ms increment, {} game(s)",
        args.time, args.inc, args.games
    );
    if let Some(ref fen) = args.start_fen {
        println!("Starting FEN: {}", fen);
    }
    println!();

    // Query engine names via UCI handshake before the match loop.
    let engine1_name = {
        let mut e = UciEngine::new(&args.engine1, args.timeout)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to query engine1 name: {}", e))?;
        let n = e.name().await;
        e.quit().await;
        n
    };
    let engine2_name = {
        let mut e = UciEngine::new(&args.engine2, args.timeout)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to query engine2 name: {}", e))?;
        let n = e.name().await;
        e.quit().await;
        n
    };

    println!("Engine 1 UCI name: {}", engine1_name);
    println!("Engine 2 UCI name: {}", engine2_name);
    println!();

    // Score: [engine1_wins, engine2_wins, draws]
    let mut engine1_wins = 0usize;
    let mut engine2_wins = 0usize;
    let mut draws = 0usize;

    for game_idx in 0..args.games {
        let game_number = game_idx + 1;

        // Alternate colors each game: even games engine1=white, odd games engine1=black
        let engine1_is_white = game_idx % 2 == 0;

        let (white_path, black_path) = if engine1_is_white {
            (args.engine1.as_str(), args.engine2.as_str())
        } else {
            (args.engine2.as_str(), args.engine1.as_str())
        };

        println!(
            "=== Game {}/{}: {} (white) vs {} (black) ===",
            game_number, args.games, white_path, black_path
        );

        let white_engine = Box::new(
            UciEngine::new(white_path, args.timeout)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to start white engine: {}", e))?,
        );
        let black_engine = Box::new(
            UciEngine::new(black_path, args.timeout)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to start black engine: {}", e))?,
        );

        let game = match &args.start_fen {
            Some(fen) => Game::from_fen(fen)?,
            None => Game::new(),
        };

        let clock = Clock::new(args.time, args.inc);
        let observer = Box::new(PrintObserver { game_number });

        let mut chess_match = Match::new(white_engine, black_engine)
            .with_game(game)
            .with_clock(clock)
            .with_observer(observer);

        let result = chess_match.run().await;

        // Record score from engine1's perspective
        match &result {
            GameResult::WhiteWins => {
                if engine1_is_white {
                    engine1_wins += 1;
                } else {
                    engine2_wins += 1;
                }
            }
            GameResult::BlackWins => {
                if engine1_is_white {
                    engine2_wins += 1;
                } else {
                    engine1_wins += 1;
                }
            }
            GameResult::Draw(_) => draws += 1,
            GameResult::Ongoing => {
                eprintln!("Warning: game {} ended in unexpected Ongoing state", game_number);
                draws += 1;
            }
        }

        println!();
    }

    println!("=== Match Complete ===");
    println!(
        "{}: {} win(s)",
        args.engine1, engine1_wins
    );
    println!(
        "{}: {} win(s)",
        args.engine2, engine2_wins
    );
    println!("Draws: {}", draws);

    // Write CSV output if requested.
    if let Some(ref output_path) = args.output {
        write_csv(
            output_path,
            &engine1_name,
            &engine2_name,
            args.games,
            engine1_wins,
            engine2_wins,
            draws,
        )?;
        println!("Match result appended to: {}", output_path);
    }

    Ok(())
}
