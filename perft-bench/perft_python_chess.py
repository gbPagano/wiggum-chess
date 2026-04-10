#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.11"
# dependencies = [
#   "chess",
# ]
# ///

import argparse
import chess

STARTING_FEN = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"


def perft(board: chess.Board, depth: int) -> int:
    if depth == 1:
        return board.legal_moves.count()
    count = 0
    for move in board.legal_moves:
        board.push(move)
        count += perft(board, depth - 1)
        board.pop()
    return count


def main() -> None:
    parser = argparse.ArgumentParser(description="Perft benchmark using python-chess")
    parser.add_argument("--depth", "-d", type=int, required=True, help="Perft depth")
    parser.add_argument("--fen", default=STARTING_FEN, help="FEN position")
    args = parser.parse_args()

    board = chess.Board(args.fen)
    nodes = perft(board, args.depth)
    print(nodes)


if __name__ == "__main__":
    main()
