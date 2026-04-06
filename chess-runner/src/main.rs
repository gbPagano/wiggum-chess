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
    /// Run a SPRT-based match to determine if engine1 is an improvement over engine2
    Sprt(SprtArgs),
    /// Generate a markdown version report for a specific engine version
    VersionReport(VersionReportArgs),
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

#[derive(Parser)]
struct SprtArgs {
    /// Path to the first engine executable (the one being tested)
    #[arg(long)]
    engine1: String,

    /// Path to the second engine executable (the baseline)
    #[arg(long)]
    engine2: String,

    /// Time per player in milliseconds
    #[arg(long, default_value = "10000")]
    time: u64,

    /// Increment per move in milliseconds
    #[arg(long, default_value = "100")]
    inc: u64,

    /// H0 Elo difference (null hypothesis — no improvement)
    #[arg(long, default_value = "0.0")]
    elo0: f64,

    /// H1 Elo difference (alternative hypothesis — target improvement)
    #[arg(long, default_value = "5.0")]
    elo1: f64,

    /// Type I error bound (false positive rate)
    #[arg(long, default_value = "0.05")]
    alpha: f64,

    /// Type II error bound (false negative rate)
    #[arg(long, default_value = "0.05")]
    beta: f64,

    /// Engine response timeout in milliseconds
    #[arg(long, default_value = "5000")]
    timeout: u64,

    /// Optional path to CSV file for appending SPRT results
    #[arg(long)]
    output: Option<String>,
}

#[derive(Parser)]
struct VersionReportArgs {
    /// Version label (e.g. 'v0.1'), used as fallback filter if --engine-name is not provided
    #[arg(long)]
    version: String,

    /// Engine name filter for matching CSV rows (case-insensitive substring, defaults to --version)
    #[arg(long)]
    engine_name: Option<String>,

    /// Path to match history CSV (from chess-runner match --output)
    #[arg(long)]
    matches_csv: String,

    /// Optional path to SPRT results CSV (from chess-runner sprt --output)
    #[arg(long)]
    sprt_csv: Option<String>,

    /// Output path for the generated markdown report
    #[arg(long)]
    output: String,
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

/// Compute expected score from Elo difference using the logistic model.
fn elo_to_score(elo_diff: f64) -> f64 {
    1.0 / (1.0 + 10.0_f64.powf(-elo_diff / 400.0))
}

/// Compute LLR bounds from alpha and beta.
pub fn sprt_bounds(alpha: f64, beta: f64) -> (f64, f64) {
    let lower = (beta / (1.0 - alpha)).ln();
    let upper = ((1.0 - beta) / alpha).ln();
    (lower, upper)
}

/// Compute SPRT LLR using the trinomial (W/D/L) model.
/// draw_ratio is estimated from actual results: D / total.
pub fn compute_llr(wins: usize, draws: usize, losses: usize, elo0: f64, elo1: f64) -> f64 {
    let total = wins + draws + losses;
    if total == 0 {
        return 0.0;
    }

    let draw_ratio = draws as f64 / total as f64;
    const EPS: f64 = 1e-9;

    let s0 = elo_to_score(elo0);
    let s1 = elo_to_score(elo1);

    // Trinomial probabilities under each hypothesis.
    let p0_w = (s0 - draw_ratio / 2.0).max(EPS);
    let p0_d = draw_ratio.max(EPS);
    let p0_l = (1.0 - s0 - draw_ratio / 2.0).max(EPS);

    let p1_w = (s1 - draw_ratio / 2.0).max(EPS);
    let p1_d = draw_ratio.max(EPS);
    let p1_l = (1.0 - s1 - draw_ratio / 2.0).max(EPS);

    let w = wins as f64;
    let d = draws as f64;
    let l = losses as f64;

    w * (p1_w / p0_w).ln() + d * (p1_d / p0_d).ln() + l * (p1_l / p0_l).ln()
}

/// Write a SPRT result row to a CSV file, creating it with headers if needed.
fn write_sprt_csv(
    path: &str,
    engine1_name: &str,
    engine2_name: &str,
    games_played: usize,
    wins: usize,
    draws: usize,
    losses: usize,
    elo0: f64,
    elo1: f64,
    sprt_result: &str,
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
            "wins",
            "draws",
            "losses",
            "elo0",
            "elo1",
            "sprt_result",
        ])?;
    }

    let timestamp = chrono::Utc::now().to_rfc3339();

    wtr.write_record([
        timestamp.as_str(),
        engine1_name,
        engine2_name,
        &games_played.to_string(),
        &wins.to_string(),
        &draws.to_string(),
        &losses.to_string(),
        &format!("{:.2}", elo0),
        &format!("{:.2}", elo1),
        sprt_result,
    ])?;

    wtr.flush()?;
    Ok(())
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
        Commands::Sprt(args) => {
            run_sprt(args).await?;
        }
        Commands::VersionReport(args) => {
            run_version_report(&args)?;
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

async fn run_sprt(args: SprtArgs) -> Result<()> {
    let (lower_bound, upper_bound) = sprt_bounds(args.alpha, args.beta);

    println!("Engine 1: {}", args.engine1);
    println!("Engine 2: {}", args.engine2);
    println!(
        "SPRT: elo0={}, elo1={}, alpha={}, beta={}",
        args.elo0, args.elo1, args.alpha, args.beta
    );
    println!(
        "LLR bounds: [{:.3}, {:.3}]",
        lower_bound, upper_bound
    );
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

    let mut wins = 0usize;
    let mut draws = 0usize;
    let mut losses = 0usize;
    let mut game_number = 0usize;
    let mut sprt_result = "inconclusive";

    loop {
        game_number += 1;
        let game_idx = game_number - 1;
        let engine1_is_white = game_idx % 2 == 0;

        let (white_path, black_path) = if engine1_is_white {
            (args.engine1.as_str(), args.engine2.as_str())
        } else {
            (args.engine2.as_str(), args.engine1.as_str())
        };

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

        let clock = Clock::new(args.time, args.inc);
        let observer = Box::new(PrintObserver {
            game_number,
            verbose: false,
        });

        let mut chess_match = Match::new(white_engine, black_engine)
            .with_clock(clock)
            .with_observer(observer);

        let result = chess_match.run().await;

        match &result {
            GameResult::WhiteWins => {
                if engine1_is_white {
                    wins += 1;
                } else {
                    losses += 1;
                }
            }
            GameResult::BlackWins => {
                if engine1_is_white {
                    losses += 1;
                } else {
                    wins += 1;
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

        let llr = compute_llr(wins, draws, losses, args.elo0, args.elo1);
        println!(
            "[Game {}] LLR: {:.2} / [{:.3}, {:.3}] | W:{} D:{} L:{}",
            game_number, llr, lower_bound, upper_bound, wins, draws, losses
        );

        if llr >= upper_bound {
            sprt_result = "H1 accepted";
            break;
        } else if llr <= lower_bound {
            sprt_result = "H0 accepted";
            break;
        }
    }

    println!();
    if sprt_result == "H1 accepted" {
        println!("SPRT Result: H1 accepted (improvement confirmed)");
    } else {
        println!("SPRT Result: H0 accepted (no improvement detected)");
    }

    if let Some(ref output_path) = args.output {
        write_sprt_csv(
            output_path,
            &engine1_name,
            &engine2_name,
            game_number,
            wins,
            draws,
            losses,
            args.elo0,
            args.elo1,
            sprt_result,
        )?;
        println!("SPRT result appended to: {}", output_path);
    }

    Ok(())
}

/// Per-opponent aggregated stats from the perspective of the target engine.
struct OpponentStats {
    opponent: String,
    games: usize,
    wins: usize,
    draws: usize,
    losses: usize,
}

impl OpponentStats {
    fn win_pct(&self) -> f64 {
        if self.games == 0 {
            0.0
        } else {
            self.wins as f64 / self.games as f64 * 100.0
        }
    }
}

fn run_version_report(args: &VersionReportArgs) -> Result<()> {
    let filter = args
        .engine_name
        .as_deref()
        .unwrap_or(args.version.as_str())
        .to_lowercase();

    // ---- Parse matches CSV ----
    let mut opponent_map: std::collections::HashMap<String, OpponentStats> =
        std::collections::HashMap::new();
    let matches_note;

    if !std::path::Path::new(&args.matches_csv).exists() {
        matches_note = format!(
            "_No matches CSV found at `{}`._",
            args.matches_csv
        );
    } else {
        let mut rdr = csv::Reader::from_path(&args.matches_csv)?;
        let mut found = 0usize;

        for record in rdr.records() {
            let record = record?;
            if record.len() < 7 {
                continue;
            }
            let engine1 = record[1].to_string();
            let engine2 = record[2].to_string();
            let games: usize = record[3].parse().unwrap_or(0);
            let e1_wins: usize = record[4].parse().unwrap_or(0);
            let e2_wins: usize = record[5].parse().unwrap_or(0);
            let draws: usize = record[6].parse().unwrap_or(0);

            let target_is_e1 = engine1.to_lowercase().contains(&filter);
            let target_is_e2 = engine2.to_lowercase().contains(&filter);

            if !target_is_e1 && !target_is_e2 {
                continue;
            }
            found += 1;

            let (opponent, wins, losses) = if target_is_e1 {
                (engine2.clone(), e1_wins, e2_wins)
            } else {
                (engine1.clone(), e2_wins, e1_wins)
            };

            let entry = opponent_map.entry(opponent.clone()).or_insert(OpponentStats {
                opponent,
                games: 0,
                wins: 0,
                draws: 0,
                losses: 0,
            });
            entry.games += games;
            entry.wins += wins;
            entry.draws += draws;
            entry.losses += losses;
        }

        if found == 0 {
            matches_note = format!(
                "_No rows matching `{}` found in `{}`._",
                filter, args.matches_csv
            );
        } else {
            matches_note = String::new();
        }
    }

    // ---- Parse SPRT CSV (optional) ----
    struct SprtRow {
        opponent: String,
        games: usize,
        wins: usize,
        draws: usize,
        losses: usize,
        result: String,
    }
    let mut sprt_rows: Vec<SprtRow> = Vec::new();
    let sprt_note;

    if let Some(ref sprt_path) = args.sprt_csv {
        if !std::path::Path::new(sprt_path).exists() {
            sprt_note = format!("_No SPRT CSV found at `{}`._", sprt_path);
        } else {
            let mut rdr = csv::Reader::from_path(sprt_path)?;
            let mut found = 0usize;

            for record in rdr.records() {
                let record = record?;
                if record.len() < 10 {
                    continue;
                }
                let engine1 = record[1].to_string();
                let engine2 = record[2].to_string();
                let games: usize = record[3].parse().unwrap_or(0);
                let wins: usize = record[4].parse().unwrap_or(0);
                let draws: usize = record[5].parse().unwrap_or(0);
                let losses: usize = record[6].parse().unwrap_or(0);
                let result = record[9].to_string();

                let target_is_e1 = engine1.to_lowercase().contains(&filter);
                let target_is_e2 = engine2.to_lowercase().contains(&filter);

                if !target_is_e1 && !target_is_e2 {
                    continue;
                }
                found += 1;

                let (opponent, w, l) = if target_is_e1 {
                    (engine2.clone(), wins, losses)
                } else {
                    (engine1.clone(), losses, wins)
                };

                sprt_rows.push(SprtRow {
                    opponent,
                    games,
                    wins: w,
                    draws,
                    losses: l,
                    result,
                });
            }

            if found == 0 {
                sprt_note = format!(
                    "_No SPRT rows matching `{}` found in `{}`._",
                    filter,
                    sprt_path
                );
            } else {
                sprt_note = String::new();
            }
        }
    } else {
        sprt_note = String::new();
    }

    // ---- Build markdown ----
    let mut md = String::new();
    let generation_date = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let report_title = args
        .engine_name
        .as_deref()
        .unwrap_or(args.version.as_str());

    md.push_str(&format!("# {} — Version Report\n\n", report_title));
    md.push_str(&format!("_Generated: {}_\n\n", generation_date));

    // ---- Match Results section ----
    md.push_str("## Match Results\n\n");

    if !matches_note.is_empty() {
        md.push_str(&matches_note);
        md.push('\n');
    } else {
        // Summary table
        md.push_str("| Opponent | Games | Wins | Draws | Losses | Win% |\n");
        md.push_str("|----------|-------|------|-------|--------|------|\n");

        let mut all_stats: Vec<&OpponentStats> = opponent_map.values().collect();
        all_stats.sort_by(|a, b| a.opponent.cmp(&b.opponent));

        let mut total_games = 0usize;
        let mut total_wins = 0usize;
        let mut total_draws = 0usize;
        let mut total_losses = 0usize;

        for s in &all_stats {
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} | {:.1}% |\n",
                s.opponent,
                s.games,
                s.wins,
                s.draws,
                s.losses,
                s.win_pct()
            ));
            total_games += s.games;
            total_wins += s.wins;
            total_draws += s.draws;
            total_losses += s.losses;
        }

        md.push('\n');

        let overall_win_pct = if total_games > 0 {
            total_wins as f64 / total_games as f64 * 100.0
        } else {
            0.0
        };

        md.push_str(&format!(
            "**Overall:** {} games, {} wins, {} draws, {} losses — **{:.1}% win rate**\n\n",
            total_games, total_wins, total_draws, total_losses, overall_win_pct
        ));

        let best = all_stats
            .iter()
            .max_by(|a, b| a.win_pct().partial_cmp(&b.win_pct()).unwrap());
        let worst = all_stats
            .iter()
            .min_by(|a, b| a.win_pct().partial_cmp(&b.win_pct()).unwrap());

        if let Some(b) = best {
            md.push_str(&format!(
                "**Best matchup:** {} ({:.1}%)\n\n",
                b.opponent,
                b.win_pct()
            ));
        }
        if let Some(w) = worst {
            md.push_str(&format!(
                "**Worst matchup:** {} ({:.1}%)\n\n",
                w.opponent,
                w.win_pct()
            ));
        }
    }

    // ---- SPRT Results section (if --sprt-csv provided) ----
    if args.sprt_csv.is_some() {
        md.push_str("## SPRT Results\n\n");

        if !sprt_note.is_empty() {
            md.push_str(&sprt_note);
            md.push('\n');
        } else {
            md.push_str("| Opponent | Games | W | D | L | Result |\n");
            md.push_str("|----------|-------|---|---|---|--------|\n");

            for row in &sprt_rows {
                md.push_str(&format!(
                    "| {} | {} | {} | {} | {} | {} |\n",
                    row.opponent, row.games, row.wins, row.draws, row.losses, row.result
                ));
            }
            md.push('\n');
        }
    }

    // ---- Notes section ----
    md.push_str("## Notes\n\n");

    // ---- Write output ----
    if let Some(parent) = std::path::Path::new(&args.output).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(&args.output, &md)?;
    println!("Version report written to: {}", args.output);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sprt_bounds_alpha05_beta05() {
        let (lower, upper) = sprt_bounds(0.05, 0.05);
        // Expected: ln(0.05/0.95) ≈ -2.944, ln(0.95/0.05) ≈ 2.944
        assert!((lower - (-2.944)).abs() < 0.001, "lower={}", lower);
        assert!((upper - 2.944).abs() < 0.001, "upper={}", upper);
    }

    #[test]
    fn test_compute_llr_known_values() {
        // W=60, D=20, L=20, elo0=0, elo1=5
        // draw_ratio=0.2, s0=0.5, s1≈0.50718
        // p0_w=0.4, p0_d=0.2, p0_l=0.4
        // p1_w≈0.40718, p1_d=0.2, p1_l≈0.39282
        // LLR should be positive since engine1 wins more than H0 predicts
        let llr = compute_llr(60, 20, 20, 0.0, 5.0);
        assert!(llr > 0.0, "LLR should be positive, got {}", llr);
        assert!(llr < 10.0, "LLR unexpectedly large: {}", llr);
    }

    #[test]
    fn test_compute_llr_equal_score_near_zero() {
        // W=50, D=0, L=50 with elo0=0 — actual score matches H0
        let llr = compute_llr(50, 0, 50, 0.0, 5.0);
        assert!(llr.abs() < 1.0, "LLR should be near 0, got {}", llr);
    }
}
