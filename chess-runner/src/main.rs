use anyhow::Result;
use chesslib::chess_move::ChessMove;
use chesslib::clock::Clock;
use chesslib::engine::Engine;
use chesslib::game::{DrawReason, Game, GameResult};
use chesslib::match_runner::{Match, MatchObserver};
use chesslib::uci_engine::UciEngine;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "chess-runner", about = "Run engine vs engine chess matches")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run an engine vs engine match
    Match(MatchArgs),
    /// Show a report of match history from a CSV file
    Report(ReportArgs),
}

#[derive(Parser)]
struct MatchArgs {
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

    /// Print board diagram and move details for every move (default: quiet mode)
    #[arg(long)]
    verbose: bool,
}

#[derive(Parser)]
struct ReportArgs {
    /// Path to CSV file with match history
    #[arg(long)]
    input: String,
}

/// Observer that prints moves and game-over events to stdout.
/// In verbose mode, prints board diagram and move details on every move.
/// In quiet mode, suppresses per-move output (caller prints the one-line summary).
struct PrintObserver {
    game_number: usize,
    verbose: bool,
}

impl MatchObserver for PrintObserver {
    fn on_move(&mut self, game: &Game, chess_move: ChessMove, engine_name: &str) {
        if self.verbose {
            println!(
                "  Game {} move {}: {} plays {}",
                self.game_number,
                game.moves().len(),
                engine_name,
                chess_move.to_uci()
            );
            println!("{:?}", game.board());
        }
    }

    fn on_game_over(&mut self, result: &GameResult) {
        if self.verbose {
            println!(
                "  Game {} result: {}",
                self.game_number,
                format_result(result)
            );
        }
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
        && std::fs::metadata(path)
            .map(|m| m.len() > 0)
            .unwrap_or(false);

    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;

    let mut wtr = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(file);

    if !file_exists {
        wtr.write_record([
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

    wtr.write_record([
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

/// A parsed row from the match history CSV.
#[derive(Debug)]
struct MatchRow {
    timestamp: String,
    engine1_name: String,
    engine2_name: String,
    games_played: usize,
    engine1_wins: usize,
    engine2_wins: usize,
    draws: usize,
    engine1_win_rate: f64,
}

fn run_report(args: &ReportArgs) -> Result<()> {
    let path = &args.input;

    if !std::path::Path::new(path).exists() {
        println!("No match history found at {}", path);
        return Ok(());
    }

    let mut rdr = csv::Reader::from_path(path)?;
    let mut rows: Vec<MatchRow> = Vec::new();

    for result in rdr.records() {
        let record = result?;
        rows.push(MatchRow {
            timestamp: record[0].to_string(),
            engine1_name: record[1].to_string(),
            engine2_name: record[2].to_string(),
            games_played: record[3].parse().unwrap_or(0),
            engine1_wins: record[4].parse().unwrap_or(0),
            engine2_wins: record[5].parse().unwrap_or(0),
            draws: record[6].parse().unwrap_or(0),
            engine1_win_rate: record[7].parse().unwrap_or(0.0),
        });
    }

    if rows.is_empty() {
        println!("No match results recorded yet");
        return Ok(());
    }

    // Compute trend arrows: compare each row's win_rate with the previous row
    // for the same (engine1, engine2) pair.
    let mut last_win_rate: std::collections::HashMap<(String, String), f64> =
        std::collections::HashMap::new();
    let mut trends: Vec<&'static str> = Vec::with_capacity(rows.len());

    for row in &rows {
        let key = (row.engine1_name.clone(), row.engine2_name.clone());
        let trend = if let Some(&prev) = last_win_rate.get(&key) {
            let diff = row.engine1_win_rate - prev;
            if diff > 0.001 {
                "↑"
            } else if diff < -0.001 {
                "↓"
            } else {
                "→"
            }
        } else {
            "-"
        };
        trends.push(trend);
        last_win_rate.insert(key, row.engine1_win_rate);
    }

    // Determine column widths.
    let date_w = rows
        .iter()
        .map(|r| {
            // Use only the date portion of the ISO timestamp for display
            r.timestamp.get(..10).unwrap_or(&r.timestamp).len()
        })
        .max()
        .unwrap_or(10)
        .max(4); // "Date"
    let e1_w = rows
        .iter()
        .map(|r| r.engine1_name.len())
        .max()
        .unwrap_or(6)
        .max(7); // "Engine1"
    let e2_w = rows
        .iter()
        .map(|r| r.engine2_name.len())
        .max()
        .unwrap_or(6)
        .max(7); // "Engine2"

    // Print header
    println!(
        "{:<date_w$}  {:<e1_w$}  {:<e2_w$}  {:>5}  {:>4}  {:>6}  {:>5}  {:>6}  Trend",
        "Date",
        "Engine1",
        "Engine2",
        "Games",
        "Wins",
        "Losses",
        "Draws",
        "Win%",
        date_w = date_w,
        e1_w = e1_w,
        e2_w = e2_w,
    );
    println!("{}", "-".repeat(date_w + e1_w + e2_w + 45));

    for (row, trend) in rows.iter().zip(trends.iter()) {
        let date = row.timestamp.get(..10).unwrap_or(&row.timestamp);
        println!(
            "{:<date_w$}  {:<e1_w$}  {:<e2_w$}  {:>5}  {:>4}  {:>6}  {:>5}  {:>5.1}%  {}",
            date,
            row.engine1_name,
            row.engine2_name,
            row.games_played,
            row.engine1_wins,
            row.engine2_wins,
            row.draws,
            row.engine1_win_rate * 100.0,
            trend,
            date_w = date_w,
            e1_w = e1_w,
            e2_w = e2_w,
        );
    }

    println!();

    // Overall summary
    let total_games: usize = rows.iter().map(|r| r.games_played).sum();
    let total_wins: usize = rows.iter().map(|r| r.engine1_wins).sum();
    let overall_win_rate = if total_games > 0 {
        total_wins as f64 / total_games as f64
    } else {
        0.0
    };

    // Best and worst matchup by win_rate
    let best = rows
        .iter()
        .max_by(|a, b| a.engine1_win_rate.partial_cmp(&b.engine1_win_rate).unwrap());
    let worst = rows
        .iter()
        .min_by(|a, b| a.engine1_win_rate.partial_cmp(&b.engine1_win_rate).unwrap());

    println!("=== Summary ===");
    println!("Total games played: {}", total_games);
    println!(
        "Overall win rate (engine1): {:.1}%",
        overall_win_rate * 100.0
    );
    if let Some(b) = best {
        println!(
            "Best matchup:  {} vs {} ({:.1}%)",
            b.engine1_name,
            b.engine2_name,
            b.engine1_win_rate * 100.0
        );
    }
    if let Some(w) = worst {
        println!(
            "Worst matchup: {} vs {} ({:.1}%)",
            w.engine1_name,
            w.engine2_name,
            w.engine1_win_rate * 100.0
        );
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Report(ref args) => {
            run_report(args)?;
        }
        Commands::Match(args) => {
            run_match(args).await?;
        }
    }

    Ok(())
}

async fn run_match(args: MatchArgs) -> Result<()> {
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

        if args.verbose {
            println!(
                "=== Game {}/{}: {} (white) vs {} (black) ===",
                game_number, args.games, white_path, black_path
            );
        }

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
        let observer = Box::new(PrintObserver {
            game_number,
            verbose: args.verbose,
        });

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
                eprintln!(
                    "Warning: game {} ended in unexpected Ongoing state",
                    game_number
                );
                draws += 1;
            }
        }

        if !args.verbose {
            println!(
                "Game {}/{}: {} | Score: {}-{}-{}",
                game_number,
                args.games,
                format_result(&result),
                engine1_wins,
                engine2_wins,
                draws
            );
        } else {
            println!();
        }
    }

    println!("=== Match Complete ===");
    println!("{}: {} win(s)", args.engine1, engine1_wins);
    println!("{}: {} win(s)", args.engine2, engine2_wins);
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
