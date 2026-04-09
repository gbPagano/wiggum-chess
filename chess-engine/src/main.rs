use chess_engine::search::{search, search_timed, SearchContext};
use chess_engine::uci::{parse_go, GoParams};
use chess_engine::uci_engine_name;
use chesslib::board::Board;
use chesslib::chess_move::ChessMove;
use chesslib::movegen::MoveGen;
use clap::Parser;
use std::io::{self, BufRead};

#[derive(Parser)]
#[command(name = "chess-engine", about = "Wiggum Engine UCI chess engine")]
struct Args {
    /// Search depth (default 5)
    #[arg(long, default_value = "5")]
    depth: u8,
}

fn main() {
    let args = Args::parse();
    let depth = args.depth;

    let stdin = io::stdin();
    let mut current_board: Option<Board> = None;

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let line = line.trim().to_string();

        if line == "uci" {
            println!("id name {}", uci_engine_name());
            println!("id author chess-ic");
            println!("uciok");
        } else if line == "isready" {
            println!("readyok");
        } else if line == "ucinewgame" {
            current_board = None;
        } else if line == "quit" {
            break;
        } else if line.starts_with("position") {
            current_board = parse_position(&line);
        } else if line.starts_with("go") {
            let go_params: GoParams = parse_go(&line);
            if let Some(ref board) = current_board {
                let mv = if let Some(budget_ms) =
                    go_params.compute_budget_ms(board.side_to_move())
                {
                    // Timed search: build a context from the computed budget and
                    // dispatch through the timed entry point.  The iterative
                    // deepening loop (US-004) and timeout checks (US-006) will
                    // fill in this path; for now a single-depth call is made.
                    let ctx = SearchContext::from_budget_ms(budget_ms);
                    search_timed(board, &ctx).0
                } else {
                    // Depth-based search: existing non-timed behavior unchanged.
                    search(board, depth).0
                };
                let best = mv
                    .or_else(|| MoveGen::new_legal(board).next())
                    .map(|m| m.to_uci())
                    .unwrap_or_else(|| "0000".to_string());
                println!("bestmove {}", best);
            }
        }
        // Unknown commands are silently ignored (UCI spec allows this)
    }
}

/// Parse a `position` command and return the resulting Board.
///
/// Formats:
///   position startpos [moves <uci_move>...]
///   position fen <fen_str> [moves <uci_move>...]
fn parse_position(line: &str) -> Option<Board> {
    use std::str::FromStr;

    let tokens: Vec<&str> = line.split_whitespace().collect();
    let moves_idx = tokens.iter().position(|&t| t == "moves");

    let mut board = if tokens.get(1) == Some(&"startpos") {
        Board::default()
    } else if tokens.get(1) == Some(&"fen") {
        let fen_end = moves_idx.unwrap_or(tokens.len());
        if fen_end <= 2 {
            return None;
        }
        let fen = tokens[2..fen_end].join(" ");
        Board::from_str(&fen).ok()?
    } else {
        return None;
    };

    if let Some(idx) = moves_idx {
        for uci in &tokens[idx + 1..] {
            match ChessMove::from_uci(uci, &board) {
                Ok(mv) => board = board.make_move(mv),
                Err(_) => return None,
            }
        }
    }

    Some(board)
}
