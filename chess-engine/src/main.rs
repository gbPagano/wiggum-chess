use chess_engine::search::search;
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
                // Timed search will be implemented in later stories.
                // For now, fall through to depth-based search regardless of time params.
                let _ = &go_params;
                let (mv, _) = search(board, depth);
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
