/// Color of a chess piece or side to move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    White,
    Black,
}

impl Color {
    pub fn opponent(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}

/// Kind of chess piece, independent of color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PieceKind {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

/// A colored chess piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece {
    pub kind: PieceKind,
    pub color: Color,
}

impl Piece {
    pub fn new(kind: PieceKind, color: Color) -> Self {
        Self { kind, color }
    }
}

/// Castling rights: which sides each color can still castle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CastlingRights {
    pub white_kingside: bool,
    pub white_queenside: bool,
    pub black_kingside: bool,
    pub black_queenside: bool,
}

impl CastlingRights {
    pub fn all() -> Self {
        Self {
            white_kingside: true,
            white_queenside: true,
            black_kingside: true,
            black_queenside: true,
        }
    }

    pub fn none() -> Self {
        Self {
            white_kingside: false,
            white_queenside: false,
            black_kingside: false,
            black_queenside: false,
        }
    }
}

/// A square on the board addressed as (rank, file) where both are 0-based.
/// rank 0 = rank 1 (white's back rank), rank 7 = rank 8 (black's back rank).
/// file 0 = file a, file 7 = file h.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Square {
    pub rank: u8,
    pub file: u8,
}

impl Square {
    pub fn new(rank: u8, file: u8) -> Self {
        debug_assert!(rank < 8 && file < 8, "Square out of bounds");
        Self { rank, file }
    }
}

/// A chess move from one square to another.
///
/// `promotion` is `Some(kind)` when a pawn reaches the back rank and promotes.
/// Normal moves, castling, and en passant have `promotion == None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move {
    pub from: Square,
    pub to: Square,
    pub promotion: Option<PieceKind>,
}

impl Move {
    pub fn new(from: Square, to: Square) -> Self {
        Self { from, to, promotion: None }
    }

    pub fn with_promotion(from: Square, to: Square, kind: PieceKind) -> Self {
        Self { from, to, promotion: Some(kind) }
    }
}

/// The full chess board state.
///
/// The board is stored as an 8×8 matrix of `Option<Piece>`.
/// Index: `squares[rank][file]`, rank 0 = rank 1 (white's home rank).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    pub squares: [[Option<Piece>; 8]; 8],
    pub side_to_move: Color,
    pub castling: CastlingRights,
    /// If `Some(file)`, the file on which an en passant capture is possible.
    /// When white is to move: black just double-pushed on this file; en passant
    /// target rank is 5. When black is to move: white just double-pushed; target rank is 2.
    pub en_passant_file: Option<u8>,
    pub halfmove_clock: u32,
    pub fullmove_number: u32,
}

impl Board {
    /// Returns an empty board (no pieces, white to move, no rights).
    pub fn empty() -> Self {
        Self {
            squares: [[None; 8]; 8],
            side_to_move: Color::White,
            castling: CastlingRights::none(),
            en_passant_file: None,
            halfmove_clock: 0,
            fullmove_number: 1,
        }
    }

    /// Returns the standard starting position.
    pub fn starting_position() -> Self {
        let mut b = Self::empty();
        b.castling = CastlingRights::all();

        use PieceKind::*;
        let back_rank = [Rook, Knight, Bishop, Queen, King, Bishop, Knight, Rook];

        for (file, &kind) in back_rank.iter().enumerate() {
            b.squares[0][file] = Some(Piece::new(kind, Color::White));
            b.squares[7][file] = Some(Piece::new(kind, Color::Black));
        }
        for file in 0..8 {
            b.squares[1][file] = Some(Piece::new(Pawn, Color::White));
            b.squares[6][file] = Some(Piece::new(Pawn, Color::Black));
        }

        b
    }

    /// Parse a board from a FEN string.
    ///
    /// Returns an error string if the FEN is malformed.
    pub fn from_fen(fen: &str) -> Result<Board, String> {
        let parts: Vec<&str> = fen.split_whitespace().collect();
        if parts.len() < 4 {
            return Err(format!("FEN needs at least 4 fields, got {}", parts.len()));
        }

        let mut board = Board::empty();

        // 1. Piece placement
        let mut rank: i8 = 7;
        let mut file: i8 = 0;
        for ch in parts[0].chars() {
            match ch {
                '/' => {
                    rank -= 1;
                    file = 0;
                }
                '1'..='8' => {
                    file += ch as i8 - '0' as i8;
                }
                _ => {
                    let color = if ch.is_uppercase() { Color::White } else { Color::Black };
                    let kind = match ch.to_ascii_lowercase() {
                        'p' => PieceKind::Pawn,
                        'n' => PieceKind::Knight,
                        'b' => PieceKind::Bishop,
                        'r' => PieceKind::Rook,
                        'q' => PieceKind::Queen,
                        'k' => PieceKind::King,
                        other => return Err(format!("Unknown piece char: {other}")),
                    };
                    if rank < 0 || rank > 7 || file < 0 || file > 7 {
                        return Err("Piece placement out of bounds".into());
                    }
                    board.squares[rank as usize][file as usize] = Some(Piece::new(kind, color));
                    file += 1;
                }
            }
        }

        // 2. Side to move
        board.side_to_move = match parts[1] {
            "w" => Color::White,
            "b" => Color::Black,
            other => return Err(format!("Unknown side to move: {other}")),
        };

        // 3. Castling rights
        board.castling = CastlingRights::none();
        if parts[2] != "-" {
            for ch in parts[2].chars() {
                match ch {
                    'K' => board.castling.white_kingside = true,
                    'Q' => board.castling.white_queenside = true,
                    'k' => board.castling.black_kingside = true,
                    'q' => board.castling.black_queenside = true,
                    _ => {}
                }
            }
        }

        // 4. En passant target square (e.g. "d6" or "-")
        board.en_passant_file = if parts[3] == "-" {
            None
        } else {
            let ep_chars: Vec<char> = parts[3].chars().collect();
            if ep_chars.len() < 2 {
                return Err(format!("Bad en passant square: {}", parts[3]));
            }
            let f = ep_chars[0] as i8 - 'a' as i8;
            if f < 0 || f > 7 {
                return Err(format!("Bad en passant file: {}", ep_chars[0]));
            }
            Some(f as u8)
        };

        // 5. Optional halfmove clock and fullmove number
        if parts.len() >= 5 {
            board.halfmove_clock = parts[4].parse().unwrap_or(0);
        }
        if parts.len() >= 6 {
            board.fullmove_number = parts[5].parse().unwrap_or(1);
        }

        Ok(board)
    }

    // -----------------------------------------------------------------------
    // Move application
    // -----------------------------------------------------------------------

    /// Apply a move to this board, returning the resulting board state.
    ///
    /// Handles all move types: normal moves, captures, en passant, castling, and promotion.
    /// Also updates castling rights, en passant state, halfmove clock, and fullmove number.
    ///
    /// Caller is responsible for passing a legal move.
    pub fn apply_move(&self, mv: Move) -> Board {
        let mut next = self.clone();

        let piece = next.squares[mv.from.rank as usize][mv.from.file as usize]
            .take()
            .expect("apply_move: no piece on from-square");

        // Detect en passant: pawn moves diagonally to an empty square
        let is_en_passant = piece.kind == PieceKind::Pawn
            && mv.from.file != mv.to.file
            && next.squares[mv.to.rank as usize][mv.to.file as usize].is_none();

        // Detect castling: king moves exactly 2 files
        let is_castling = piece.kind == PieceKind::King
            && (mv.from.file as i8 - mv.to.file as i8).abs() == 2;

        // Place piece on destination (captures whatever was there)
        let captured = next.squares[mv.to.rank as usize][mv.to.file as usize].replace(piece);

        // Halfmove clock: reset on pawn move or capture (en passant is also a pawn move)
        if piece.kind == PieceKind::Pawn || captured.is_some() || is_en_passant {
            next.halfmove_clock = 0;
        } else {
            next.halfmove_clock += 1;
        }

        // Fullmove number increments after black's move
        if self.side_to_move == Color::Black {
            next.fullmove_number += 1;
        }

        // En passant: remove the captured pawn (on same rank as from-square, file of to-square)
        if is_en_passant {
            next.squares[mv.from.rank as usize][mv.to.file as usize] = None;
        }

        // Castling: move the rook to the other side of the king
        if is_castling {
            let r = mv.from.rank as usize;
            if mv.to.file == 6 {
                // Kingside: rook from h-file (7) to f-file (5)
                next.squares[r][5] = next.squares[r][7].take();
            } else {
                // Queenside: rook from a-file (0) to d-file (3)
                next.squares[r][3] = next.squares[r][0].take();
            }
        }

        // Promotion: replace the pawn with the chosen piece
        if let Some(promo_kind) = mv.promotion {
            next.squares[mv.to.rank as usize][mv.to.file as usize] =
                Some(Piece::new(promo_kind, self.side_to_move));
        }

        // Update castling rights: king move loses both rights for that color
        if piece.kind == PieceKind::King {
            match self.side_to_move {
                Color::White => {
                    next.castling.white_kingside = false;
                    next.castling.white_queenside = false;
                }
                Color::Black => {
                    next.castling.black_kingside = false;
                    next.castling.black_queenside = false;
                }
            }
        }

        // Rook move loses its specific right
        if piece.kind == PieceKind::Rook {
            match (mv.from.rank, mv.from.file) {
                (0, 0) => next.castling.white_queenside = false,
                (0, 7) => next.castling.white_kingside = false,
                (7, 0) => next.castling.black_queenside = false,
                (7, 7) => next.castling.black_kingside = false,
                _ => {}
            }
        }

        // Rook captured on its home square also loses castling right
        if let Some(cap) = captured {
            if cap.kind == PieceKind::Rook {
                match (mv.to.rank, mv.to.file) {
                    (0, 0) => next.castling.white_queenside = false,
                    (0, 7) => next.castling.white_kingside = false,
                    (7, 0) => next.castling.black_queenside = false,
                    (7, 7) => next.castling.black_kingside = false,
                    _ => {}
                }
            }
        }

        // Set en passant file for double pawn pushes; clear otherwise
        next.en_passant_file = if piece.kind == PieceKind::Pawn {
            match self.side_to_move {
                Color::White if mv.from.rank == 1 && mv.to.rank == 3 => Some(mv.from.file),
                Color::Black if mv.from.rank == 6 && mv.to.rank == 4 => Some(mv.from.file),
                _ => None,
            }
        } else {
            None
        };

        next.side_to_move = self.side_to_move.opponent();
        next
    }

    // -----------------------------------------------------------------------
    // Check and attack detection
    // -----------------------------------------------------------------------

    /// Returns true if `sq` is attacked by any piece of `by_color`.
    pub fn is_square_attacked(&self, sq: Square, by_color: Color) -> bool {
        // Pawn attacks: a pawn of `by_color` attacks `sq` diagonally forward.
        // From `sq`'s perspective, look in the direction opposite to the pawn's advance.
        let pawn_dir: i8 = if by_color == Color::White { 1 } else { -1 };
        let pawn_rank = sq.rank as i8 - pawn_dir;
        if (0..8).contains(&pawn_rank) {
            for df in [-1i8, 1i8] {
                let pf = sq.file as i8 + df;
                if (0..8).contains(&pf) {
                    if let Some(p) = self.get(Square::new(pawn_rank as u8, pf as u8)) {
                        if p.kind == PieceKind::Pawn && p.color == by_color {
                            return true;
                        }
                    }
                }
            }
        }

        // Knight attacks
        for (dr, df) in [(-2i8,-1i8),(-2,1),(-1,-2),(-1,2),(1,-2),(1,2),(2,-1),(2,1)] {
            let r = sq.rank as i8 + dr;
            let f = sq.file as i8 + df;
            if (0..8).contains(&r) && (0..8).contains(&f) {
                if let Some(p) = self.get(Square::new(r as u8, f as u8)) {
                    if p.kind == PieceKind::Knight && p.color == by_color {
                        return true;
                    }
                }
            }
        }

        // King attacks
        for (dr, df) in [(-1i8,-1i8),(-1,0),(-1,1),(0,-1),(0,1),(1,-1),(1,0),(1,1)] {
            let r = sq.rank as i8 + dr;
            let f = sq.file as i8 + df;
            if (0..8).contains(&r) && (0..8).contains(&f) {
                if let Some(p) = self.get(Square::new(r as u8, f as u8)) {
                    if p.kind == PieceKind::King && p.color == by_color {
                        return true;
                    }
                }
            }
        }

        // Bishop / Queen diagonal attacks
        for (dr, df) in [(-1i8,-1i8),(-1,1),(1,-1),(1,1)] {
            let mut r = sq.rank as i8 + dr;
            let mut f = sq.file as i8 + df;
            while (0..8).contains(&r) && (0..8).contains(&f) {
                match self.get(Square::new(r as u8, f as u8)) {
                    Some(p) if p.color == by_color
                        && (p.kind == PieceKind::Bishop || p.kind == PieceKind::Queen) =>
                    {
                        return true;
                    }
                    Some(_) => break,
                    None => {}
                }
                r += dr;
                f += df;
            }
        }

        // Rook / Queen rank and file attacks
        for (dr, df) in [(-1i8,0i8),(1,0),(0,-1),(0,1)] {
            let mut r = sq.rank as i8 + dr;
            let mut f = sq.file as i8 + df;
            while (0..8).contains(&r) && (0..8).contains(&f) {
                match self.get(Square::new(r as u8, f as u8)) {
                    Some(p) if p.color == by_color
                        && (p.kind == PieceKind::Rook || p.kind == PieceKind::Queen) =>
                    {
                        return true;
                    }
                    Some(_) => break,
                    None => {}
                }
                r += dr;
                f += df;
            }
        }

        false
    }

    /// Find the king square for the given color. Returns `None` if no king found.
    pub fn king_square(&self, color: Color) -> Option<Square> {
        for rank in 0..8u8 {
            for file in 0..8u8 {
                let sq = Square::new(rank, file);
                if let Some(p) = self.get(sq) {
                    if p.kind == PieceKind::King && p.color == color {
                        return Some(sq);
                    }
                }
            }
        }
        None
    }

    /// Returns true if the given color's king is currently in check.
    pub fn is_in_check(&self, color: Color) -> bool {
        self.king_square(color)
            .map(|sq| self.is_square_attacked(sq, color.opponent()))
            .unwrap_or(false)
    }

    // -----------------------------------------------------------------------
    // Legal move generation
    // -----------------------------------------------------------------------

    /// Generate all legal moves for the side to move.
    ///
    /// Filters out pseudo-legal moves that leave the moving side's king in check.
    /// Includes castling, en passant, and all promotion choices.
    pub fn legal_moves(&self) -> Vec<Move> {
        let color = self.side_to_move;
        let mut pseudo = Vec::new();
        pseudo.extend(self.pseudo_legal_pawn_moves());
        pseudo.extend(self.pseudo_legal_knight_moves());
        pseudo.extend(self.pseudo_legal_bishop_moves());
        pseudo.extend(self.pseudo_legal_rook_moves());
        pseudo.extend(self.pseudo_legal_queen_moves());
        pseudo.extend(self.pseudo_legal_king_moves());
        pseudo.extend(self.castling_moves());

        pseudo
            .into_iter()
            .filter(|&mv| {
                let after = self.apply_move(mv);
                !after.is_in_check(color)
            })
            .collect()
    }

    // -----------------------------------------------------------------------
    // Pseudo-legal move generators
    // -----------------------------------------------------------------------

    /// Generate pseudo-legal pawn moves for the side to move.
    ///
    /// Includes single pushes, double pushes, diagonal captures, en passant,
    /// and all four promotion choices.
    pub fn pseudo_legal_pawn_moves(&self) -> Vec<Move> {
        let mut moves = Vec::new();
        let color = self.side_to_move;

        let (start_rank, advance_dir, promote_rank): (u8, i8, u8) = match color {
            Color::White => (1, 1, 7),
            Color::Black => (6, -1, 0),
        };

        // En passant: where the capturing pawn starts, and where it lands
        let ep_source_rank: u8 = match color {
            Color::White => 4,
            Color::Black => 3,
        };
        let ep_target_rank: u8 = match color {
            Color::White => 5,
            Color::Black => 2,
        };

        for rank in 0..8u8 {
            for file in 0..8u8 {
                let sq = Square::new(rank, file);
                let Some(piece) = self.get(sq) else { continue };
                if piece.kind != PieceKind::Pawn || piece.color != color {
                    continue;
                }

                let to_rank = rank as i8 + advance_dir;
                if !(0..8).contains(&to_rank) {
                    continue;
                }
                let to_rank = to_rank as u8;
                let will_promote = to_rank == promote_rank;

                // Single push
                if self.squares[to_rank as usize][file as usize].is_none() {
                    if will_promote {
                        for promo in [
                            PieceKind::Queen,
                            PieceKind::Rook,
                            PieceKind::Bishop,
                            PieceKind::Knight,
                        ] {
                            moves.push(Move::with_promotion(sq, Square::new(to_rank, file), promo));
                        }
                    } else {
                        moves.push(Move::new(sq, Square::new(to_rank, file)));

                        // Double push from starting rank
                        if rank == start_rank {
                            let to_rank2 = (to_rank as i8 + advance_dir) as u8;
                            if self.squares[to_rank2 as usize][file as usize].is_none() {
                                moves.push(Move::new(sq, Square::new(to_rank2, file)));
                            }
                        }
                    }
                }

                // Diagonal captures (normal and promotion captures)
                for &df in &[-1i8, 1i8] {
                    let cap_file = file as i8 + df;
                    if !(0..8).contains(&cap_file) {
                        continue;
                    }
                    let cap_file = cap_file as u8;
                    let cap_sq = Square::new(to_rank, cap_file);

                    // Normal capture
                    if let Some(target) = self.get(cap_sq) {
                        if target.color != color {
                            if will_promote {
                                for promo in [
                                    PieceKind::Queen,
                                    PieceKind::Rook,
                                    PieceKind::Bishop,
                                    PieceKind::Knight,
                                ] {
                                    moves.push(Move::with_promotion(sq, cap_sq, promo));
                                }
                            } else {
                                moves.push(Move::new(sq, cap_sq));
                            }
                        }
                    }

                    // En passant capture
                    if rank == ep_source_rank {
                        if let Some(ep_file) = self.en_passant_file {
                            if cap_file == ep_file && to_rank == ep_target_rank {
                                moves.push(Move::new(sq, Square::new(to_rank, cap_file)));
                            }
                        }
                    }
                }
            }
        }

        moves
    }

    /// Generate pseudo-legal knight moves for the side to move.
    pub fn pseudo_legal_knight_moves(&self) -> Vec<Move> {
        let color = self.side_to_move;
        let offsets: [(i8, i8); 8] = [
            (-2, -1), (-2, 1),
            (-1, -2), (-1, 2),
            ( 1, -2), ( 1, 2),
            ( 2, -1), ( 2, 1),
        ];
        self.short_range_moves(color, PieceKind::Knight, &offsets)
    }

    /// Generate pseudo-legal king moves for the side to move (no castling).
    pub fn pseudo_legal_king_moves(&self) -> Vec<Move> {
        let color = self.side_to_move;
        let offsets: [(i8, i8); 8] = [
            (-1, -1), (-1, 0), (-1, 1),
            ( 0, -1),           ( 0, 1),
            ( 1, -1), ( 1, 0), ( 1, 1),
        ];
        self.short_range_moves(color, PieceKind::King, &offsets)
    }

    /// Generate pseudo-legal bishop moves for the side to move.
    pub fn pseudo_legal_bishop_moves(&self) -> Vec<Move> {
        let directions = [(-1i8, -1i8), (-1, 1), (1, -1), (1, 1)];
        self.sliding_moves(PieceKind::Bishop, &directions)
    }

    /// Generate pseudo-legal rook moves for the side to move.
    pub fn pseudo_legal_rook_moves(&self) -> Vec<Move> {
        let directions = [(-1i8, 0i8), (1, 0), (0, -1), (0, 1)];
        self.sliding_moves(PieceKind::Rook, &directions)
    }

    /// Generate pseudo-legal queen moves for the side to move.
    pub fn pseudo_legal_queen_moves(&self) -> Vec<Move> {
        let directions = [
            (-1i8, -1i8), (-1, 1), (1, -1), (1, 1),
            (-1, 0), (1, 0), (0, -1), (0, 1),
        ];
        self.sliding_moves(PieceKind::Queen, &directions)
    }

    /// Generate castling moves for the side to move.
    ///
    /// Returns at most two moves (kingside and queenside).
    /// All legality conditions are checked: rights, empty squares, and not
    /// castling through or out of check.
    pub fn castling_moves(&self) -> Vec<Move> {
        let mut moves = Vec::new();
        let color = self.side_to_move;
        let opponent = color.opponent();

        let back_rank: u8 = match color {
            Color::White => 0,
            Color::Black => 7,
        };

        let king_sq = Square::new(back_rank, 4);

        // Verify the king is actually on e1/e8
        if self.get(king_sq) != Some(Piece::new(PieceKind::King, color)) {
            return moves;
        }

        // King must not be in check to castle
        if self.is_square_attacked(king_sq, opponent) {
            return moves;
        }

        // Kingside castling (king to g1/g8)
        let can_ks = match color {
            Color::White => self.castling.white_kingside,
            Color::Black => self.castling.black_kingside,
        };
        if can_ks {
            let f_sq = Square::new(back_rank, 5);
            let g_sq = Square::new(back_rank, 6);
            if self.get(f_sq).is_none()
                && self.get(g_sq).is_none()
                && !self.is_square_attacked(f_sq, opponent)
                && !self.is_square_attacked(g_sq, opponent)
            {
                moves.push(Move::new(king_sq, g_sq));
            }
        }

        // Queenside castling (king to c1/c8)
        let can_qs = match color {
            Color::White => self.castling.white_queenside,
            Color::Black => self.castling.black_queenside,
        };
        if can_qs {
            let d_sq = Square::new(back_rank, 3);
            let c_sq = Square::new(back_rank, 2);
            let b_sq = Square::new(back_rank, 1);
            // b-file must be empty too, though the king doesn't pass through it
            if self.get(d_sq).is_none()
                && self.get(c_sq).is_none()
                && self.get(b_sq).is_none()
                // King passes through d and lands on c — neither may be attacked
                && !self.is_square_attacked(d_sq, opponent)
                && !self.is_square_attacked(c_sq, opponent)
            {
                moves.push(Move::new(king_sq, c_sq));
            }
        }

        moves
    }

    // -----------------------------------------------------------------------
    // Perft
    // -----------------------------------------------------------------------

    /// Count leaf nodes at the given depth using the legal move generator.
    ///
    /// `perft(0)` returns 1 (the current node).
    /// `perft(1)` returns the number of legal moves.
    pub fn perft(&self, depth: usize) -> u64 {
        if depth == 0 {
            return 1;
        }
        let moves = self.legal_moves();
        if depth == 1 {
            return moves.len() as u64;
        }
        moves.iter().map(|&mv| self.apply_move(mv).perft(depth - 1)).sum()
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    fn sliding_moves(&self, kind: PieceKind, directions: &[(i8, i8)]) -> Vec<Move> {
        let color = self.side_to_move;
        let mut moves = Vec::new();
        for rank in 0..8u8 {
            for file in 0..8u8 {
                let sq = Square::new(rank, file);
                let Some(piece) = self.get(sq) else { continue };
                if piece.kind != kind || piece.color != color {
                    continue;
                }
                for &(dr, df) in directions {
                    let mut r = rank as i8 + dr;
                    let mut f = file as i8 + df;
                    while (0..8).contains(&r) && (0..8).contains(&f) {
                        let to_sq = Square::new(r as u8, f as u8);
                        match self.get(to_sq) {
                            None => {
                                moves.push(Move::new(sq, to_sq));
                            }
                            Some(blocker) if blocker.color != color => {
                                moves.push(Move::new(sq, to_sq));
                                break;
                            }
                            Some(_) => break,
                        }
                        r += dr;
                        f += df;
                    }
                }
            }
        }
        moves
    }

    fn short_range_moves(&self, color: Color, kind: PieceKind, offsets: &[(i8, i8)]) -> Vec<Move> {
        let mut moves = Vec::new();
        for rank in 0..8u8 {
            for file in 0..8u8 {
                let sq = Square::new(rank, file);
                let Some(piece) = self.get(sq) else { continue };
                if piece.kind != kind || piece.color != color {
                    continue;
                }
                for &(dr, df) in offsets {
                    let to_rank = rank as i8 + dr;
                    let to_file = file as i8 + df;
                    if !(0..8).contains(&to_rank) || !(0..8).contains(&to_file) {
                        continue;
                    }
                    let to_sq = Square::new(to_rank as u8, to_file as u8);
                    if let Some(target) = self.get(to_sq) {
                        if target.color == color {
                            continue;
                        }
                    }
                    moves.push(Move::new(sq, to_sq));
                }
            }
        }
        moves
    }

    /// Get the piece at a square (if any).
    pub fn get(&self, sq: Square) -> Option<Piece> {
        self.squares[sq.rank as usize][sq.file as usize]
    }

    /// Set the piece at a square.
    pub fn set(&mut self, sq: Square, piece: Option<Piece>) {
        self.squares[sq.rank as usize][sq.file as usize] = piece;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Board construction ---

    #[test]
    fn empty_board_has_no_pieces() {
        let board = Board::empty();
        for rank in 0..8u8 {
            for file in 0..8u8 {
                assert!(board.get(Square::new(rank, file)).is_none());
            }
        }
    }

    #[test]
    fn starting_position_has_correct_pieces() {
        let board = Board::starting_position();

        assert_eq!(
            board.get(Square::new(0, 0)),
            Some(Piece::new(PieceKind::Rook, Color::White))
        );
        assert_eq!(
            board.get(Square::new(0, 4)),
            Some(Piece::new(PieceKind::King, Color::White))
        );
        assert_eq!(
            board.get(Square::new(7, 4)),
            Some(Piece::new(PieceKind::King, Color::Black))
        );
        assert_eq!(
            board.get(Square::new(1, 3)),
            Some(Piece::new(PieceKind::Pawn, Color::White))
        );
        assert_eq!(
            board.get(Square::new(6, 3)),
            Some(Piece::new(PieceKind::Pawn, Color::Black))
        );
        assert!(board.get(Square::new(3, 3)).is_none());
    }

    #[test]
    fn starting_position_metadata() {
        let board = Board::starting_position();
        assert_eq!(board.side_to_move, Color::White);
        assert!(board.castling.white_kingside);
        assert!(board.castling.white_queenside);
        assert!(board.castling.black_kingside);
        assert!(board.castling.black_queenside);
        assert!(board.en_passant_file.is_none());
    }

    #[test]
    fn color_opponent() {
        assert_eq!(Color::White.opponent(), Color::Black);
        assert_eq!(Color::Black.opponent(), Color::White);
    }

    // --- FEN parsing ---

    #[test]
    fn fen_starting_position() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let board = Board::from_fen(fen).unwrap();
        assert_eq!(board.side_to_move, Color::White);
        assert!(board.castling.white_kingside);
        assert!(board.castling.black_queenside);
        assert_eq!(
            board.get(Square::new(0, 4)),
            Some(Piece::new(PieceKind::King, Color::White))
        );
        assert_eq!(
            board.get(Square::new(7, 4)),
            Some(Piece::new(PieceKind::King, Color::Black))
        );
    }

    #[test]
    fn fen_en_passant_field() {
        let fen = "8/5bk1/8/2Pp4/8/1K6/8/8 w - d6 0 1";
        let board = Board::from_fen(fen).unwrap();
        // d6 = file d = file index 3
        assert_eq!(board.en_passant_file, Some(3));
    }

    #[test]
    fn fen_no_castling() {
        let fen = "4k3/8/8/8/8/8/8/4K3 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        assert!(!board.castling.white_kingside);
        assert!(!board.castling.white_queenside);
        assert!(!board.castling.black_kingside);
        assert!(!board.castling.black_queenside);
    }

    // --- apply_move ---

    #[test]
    fn normal_move_updates_squares_and_side_to_move() {
        let board = Board::starting_position();
        let from = Square::new(1, 4);
        let to = Square::new(2, 4);
        let after = board.apply_move(Move::new(from, to));

        assert!(after.get(from).is_none(), "from-square should be empty");
        assert_eq!(
            after.get(to),
            Some(Piece::new(PieceKind::Pawn, Color::White))
        );
        assert_eq!(after.side_to_move, Color::Black);
    }

    #[test]
    fn capture_removes_captured_piece() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        let attacker_sq = Square::new(3, 4);
        let target_sq = Square::new(4, 4);
        board.set(attacker_sq, Some(Piece::new(PieceKind::Rook, Color::White)));
        board.set(target_sq, Some(Piece::new(PieceKind::Pawn, Color::Black)));

        let after = board.apply_move(Move::new(attacker_sq, target_sq));

        assert!(after.get(attacker_sq).is_none());
        assert_eq!(
            after.get(target_sq),
            Some(Piece::new(PieceKind::Rook, Color::White))
        );
    }

    #[test]
    fn fullmove_increments_after_black_move() {
        let mut board = Board::empty();
        board.side_to_move = Color::Black;
        board.fullmove_number = 3;
        board.set(Square::new(6, 0), Some(Piece::new(PieceKind::Rook, Color::Black)));
        board.set(Square::new(7, 4), Some(Piece::new(PieceKind::King, Color::Black)));

        let after = board.apply_move(Move::new(Square::new(6, 0), Square::new(5, 0)));
        assert_eq!(after.fullmove_number, 4);
    }

    #[test]
    fn halfmove_clock_resets_on_capture() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.halfmove_clock = 10;
        board.set(Square::new(0, 0), Some(Piece::new(PieceKind::Rook, Color::White)));
        board.set(Square::new(4, 0), Some(Piece::new(PieceKind::Pawn, Color::Black)));

        let after = board.apply_move(Move::new(Square::new(0, 0), Square::new(4, 0)));
        assert_eq!(after.halfmove_clock, 0);
    }

    #[test]
    fn halfmove_clock_increments_on_quiet_non_pawn_move() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.halfmove_clock = 5;
        board.set(Square::new(0, 0), Some(Piece::new(PieceKind::Rook, Color::White)));
        board.set(Square::new(0, 4), Some(Piece::new(PieceKind::King, Color::White)));

        let after = board.apply_move(Move::new(Square::new(0, 0), Square::new(4, 0)));
        assert_eq!(after.halfmove_clock, 6);
    }

    #[test]
    fn apply_move_sets_en_passant_file_on_double_push() {
        let board = Board::starting_position();
        let after = board.apply_move(Move::new(Square::new(1, 4), Square::new(3, 4)));
        assert_eq!(after.en_passant_file, Some(4));
    }

    #[test]
    fn apply_move_clears_en_passant_on_single_push() {
        let board = Board::starting_position();
        let after = board.apply_move(Move::new(Square::new(1, 4), Square::new(2, 4)));
        assert_eq!(after.en_passant_file, None);
    }

    #[test]
    fn apply_move_en_passant_removes_captured_pawn() {
        // White pawn on e5 (rank 4, file 4), black pawn on d5 (rank 4, file 3)
        // En passant: white captures d6 (rank 5, file 3)
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.en_passant_file = Some(3); // d-file
        board.set(Square::new(4, 4), Some(Piece::new(PieceKind::Pawn, Color::White)));
        board.set(Square::new(4, 3), Some(Piece::new(PieceKind::Pawn, Color::Black)));
        // Need kings to avoid issues in legal_moves checks
        board.set(Square::new(0, 4), Some(Piece::new(PieceKind::King, Color::White)));
        board.set(Square::new(7, 4), Some(Piece::new(PieceKind::King, Color::Black)));

        let after = board.apply_move(Move::new(Square::new(4, 4), Square::new(5, 3)));

        // White pawn should be on d6
        assert_eq!(
            after.get(Square::new(5, 3)),
            Some(Piece::new(PieceKind::Pawn, Color::White))
        );
        // Original square empty
        assert!(after.get(Square::new(4, 4)).is_none());
        // Captured black pawn removed from d5
        assert!(after.get(Square::new(4, 3)).is_none());
    }

    #[test]
    fn apply_move_castling_kingside_white() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.castling.white_kingside = true;
        board.set(Square::new(0, 4), Some(Piece::new(PieceKind::King, Color::White)));
        board.set(Square::new(0, 7), Some(Piece::new(PieceKind::Rook, Color::White)));
        board.set(Square::new(7, 4), Some(Piece::new(PieceKind::King, Color::Black)));

        // Castle kingside: king e1→g1
        let after = board.apply_move(Move::new(Square::new(0, 4), Square::new(0, 6)));

        assert_eq!(
            after.get(Square::new(0, 6)),
            Some(Piece::new(PieceKind::King, Color::White))
        );
        assert_eq!(
            after.get(Square::new(0, 5)),
            Some(Piece::new(PieceKind::Rook, Color::White))
        );
        assert!(after.get(Square::new(0, 4)).is_none());
        assert!(after.get(Square::new(0, 7)).is_none());
        assert!(!after.castling.white_kingside);
        assert!(!after.castling.white_queenside);
    }

    #[test]
    fn apply_move_castling_queenside_white() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.castling.white_queenside = true;
        board.set(Square::new(0, 4), Some(Piece::new(PieceKind::King, Color::White)));
        board.set(Square::new(0, 0), Some(Piece::new(PieceKind::Rook, Color::White)));
        board.set(Square::new(7, 4), Some(Piece::new(PieceKind::King, Color::Black)));

        // Castle queenside: king e1→c1
        let after = board.apply_move(Move::new(Square::new(0, 4), Square::new(0, 2)));

        assert_eq!(
            after.get(Square::new(0, 2)),
            Some(Piece::new(PieceKind::King, Color::White))
        );
        assert_eq!(
            after.get(Square::new(0, 3)),
            Some(Piece::new(PieceKind::Rook, Color::White))
        );
        assert!(after.get(Square::new(0, 4)).is_none());
        assert!(after.get(Square::new(0, 0)).is_none());
    }

    #[test]
    fn apply_move_promotion() {
        // White pawn on e7 promotes to queen
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.set(Square::new(6, 4), Some(Piece::new(PieceKind::Pawn, Color::White)));
        board.set(Square::new(0, 4), Some(Piece::new(PieceKind::King, Color::White)));
        board.set(Square::new(7, 0), Some(Piece::new(PieceKind::King, Color::Black)));

        let after = board.apply_move(Move::with_promotion(
            Square::new(6, 4),
            Square::new(7, 4),
            PieceKind::Queen,
        ));

        assert_eq!(
            after.get(Square::new(7, 4)),
            Some(Piece::new(PieceKind::Queen, Color::White))
        );
        assert!(after.get(Square::new(6, 4)).is_none());
    }

    #[test]
    fn castling_rights_lost_after_king_move() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.castling = CastlingRights::all();
        board.set(Square::new(0, 4), Some(Piece::new(PieceKind::King, Color::White)));
        board.set(Square::new(7, 4), Some(Piece::new(PieceKind::King, Color::Black)));

        let after = board.apply_move(Move::new(Square::new(0, 4), Square::new(0, 3)));
        assert!(!after.castling.white_kingside);
        assert!(!after.castling.white_queenside);
        // Black castling rights unchanged
        assert!(after.castling.black_kingside);
        assert!(after.castling.black_queenside);
    }

    #[test]
    fn castling_rights_lost_after_rook_move() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.castling = CastlingRights::all();
        board.set(Square::new(0, 4), Some(Piece::new(PieceKind::King, Color::White)));
        board.set(Square::new(0, 7), Some(Piece::new(PieceKind::Rook, Color::White)));
        board.set(Square::new(7, 4), Some(Piece::new(PieceKind::King, Color::Black)));

        let after = board.apply_move(Move::new(Square::new(0, 7), Square::new(0, 6)));
        assert!(!after.castling.white_kingside);
        assert!(after.castling.white_queenside); // unaffected
    }

    // --- Pawn pseudo-legal moves ---

    #[test]
    fn white_pawn_single_push() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.set(Square::new(3, 4), Some(Piece::new(PieceKind::Pawn, Color::White)));
        let moves = board.pseudo_legal_pawn_moves();
        assert!(moves.contains(&Move::new(Square::new(3, 4), Square::new(4, 4))));
        assert!(!moves.contains(&Move::new(Square::new(3, 4), Square::new(5, 4))));
    }

    #[test]
    fn white_pawn_double_push_from_start_rank() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.set(Square::new(1, 4), Some(Piece::new(PieceKind::Pawn, Color::White)));
        let moves = board.pseudo_legal_pawn_moves();
        assert!(moves.contains(&Move::new(Square::new(1, 4), Square::new(2, 4))));
        assert!(moves.contains(&Move::new(Square::new(1, 4), Square::new(3, 4))));
    }

    #[test]
    fn white_pawn_double_push_blocked_by_piece() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.set(Square::new(1, 4), Some(Piece::new(PieceKind::Pawn, Color::White)));
        board.set(Square::new(2, 4), Some(Piece::new(PieceKind::Rook, Color::Black)));
        let moves = board.pseudo_legal_pawn_moves();
        assert!(!moves.contains(&Move::new(Square::new(1, 4), Square::new(2, 4))));
        assert!(!moves.contains(&Move::new(Square::new(1, 4), Square::new(3, 4))));
    }

    #[test]
    fn white_pawn_double_push_blocked_by_second_piece() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.set(Square::new(1, 4), Some(Piece::new(PieceKind::Pawn, Color::White)));
        board.set(Square::new(3, 4), Some(Piece::new(PieceKind::Rook, Color::Black)));
        let moves = board.pseudo_legal_pawn_moves();
        assert!(moves.contains(&Move::new(Square::new(1, 4), Square::new(2, 4))));
        assert!(!moves.contains(&Move::new(Square::new(1, 4), Square::new(3, 4))));
    }

    #[test]
    fn white_pawn_captures_diagonally() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.set(Square::new(3, 3), Some(Piece::new(PieceKind::Pawn, Color::White)));
        board.set(Square::new(4, 2), Some(Piece::new(PieceKind::Pawn, Color::Black)));
        board.set(Square::new(4, 4), Some(Piece::new(PieceKind::Knight, Color::Black)));
        let moves = board.pseudo_legal_pawn_moves();
        assert!(moves.contains(&Move::new(Square::new(3, 3), Square::new(4, 2))));
        assert!(moves.contains(&Move::new(Square::new(3, 3), Square::new(4, 4))));
    }

    #[test]
    fn white_pawn_cannot_capture_friendly() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.set(Square::new(3, 3), Some(Piece::new(PieceKind::Pawn, Color::White)));
        board.set(Square::new(4, 2), Some(Piece::new(PieceKind::Rook, Color::White)));
        board.set(Square::new(4, 4), Some(Piece::new(PieceKind::Rook, Color::White)));
        let moves = board.pseudo_legal_pawn_moves();
        assert!(!moves.contains(&Move::new(Square::new(3, 3), Square::new(4, 2))));
        assert!(!moves.contains(&Move::new(Square::new(3, 3), Square::new(4, 4))));
    }

    #[test]
    fn black_pawn_single_push() {
        let mut board = Board::empty();
        board.side_to_move = Color::Black;
        board.set(Square::new(4, 4), Some(Piece::new(PieceKind::Pawn, Color::Black)));
        let moves = board.pseudo_legal_pawn_moves();
        assert!(moves.contains(&Move::new(Square::new(4, 4), Square::new(3, 4))));
    }

    #[test]
    fn black_pawn_double_push_from_start_rank() {
        let mut board = Board::empty();
        board.side_to_move = Color::Black;
        board.set(Square::new(6, 4), Some(Piece::new(PieceKind::Pawn, Color::Black)));
        let moves = board.pseudo_legal_pawn_moves();
        assert!(moves.contains(&Move::new(Square::new(6, 4), Square::new(5, 4))));
        assert!(moves.contains(&Move::new(Square::new(6, 4), Square::new(4, 4))));
    }

    #[test]
    fn black_pawn_captures_diagonally() {
        let mut board = Board::empty();
        board.side_to_move = Color::Black;
        board.set(Square::new(4, 3), Some(Piece::new(PieceKind::Pawn, Color::Black)));
        board.set(Square::new(3, 2), Some(Piece::new(PieceKind::Pawn, Color::White)));
        board.set(Square::new(3, 4), Some(Piece::new(PieceKind::Knight, Color::White)));
        let moves = board.pseudo_legal_pawn_moves();
        assert!(moves.contains(&Move::new(Square::new(4, 3), Square::new(3, 2))));
        assert!(moves.contains(&Move::new(Square::new(4, 3), Square::new(3, 4))));
    }

    #[test]
    fn pawn_promotion_generates_four_moves() {
        // White pawn on e7 ready to promote
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.set(Square::new(6, 4), Some(Piece::new(PieceKind::Pawn, Color::White)));
        let moves = board.pseudo_legal_pawn_moves();

        // Should generate 4 promotions (Q, R, B, N)
        let promo_moves: Vec<_> = moves.iter()
            .filter(|m| m.from == Square::new(6, 4) && m.to == Square::new(7, 4))
            .collect();
        assert_eq!(promo_moves.len(), 4);
        assert!(promo_moves.iter().any(|m| m.promotion == Some(PieceKind::Queen)));
        assert!(promo_moves.iter().any(|m| m.promotion == Some(PieceKind::Rook)));
        assert!(promo_moves.iter().any(|m| m.promotion == Some(PieceKind::Bishop)));
        assert!(promo_moves.iter().any(|m| m.promotion == Some(PieceKind::Knight)));
    }

    #[test]
    fn en_passant_move_generated() {
        // White pawn on e5 (rank 4, file 4), black just double-pushed d7→d5 (en passant on file 3)
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.en_passant_file = Some(3); // d-file
        board.set(Square::new(4, 4), Some(Piece::new(PieceKind::Pawn, Color::White)));
        board.set(Square::new(4, 3), Some(Piece::new(PieceKind::Pawn, Color::Black)));

        let moves = board.pseudo_legal_pawn_moves();
        // En passant capture: e5→d6 (rank 5, file 3)
        assert!(moves.contains(&Move::new(Square::new(4, 4), Square::new(5, 3))));
    }

    // --- Knight and king pseudo-legal moves (unchanged from before) ---

    #[test]
    fn knight_in_center_has_eight_moves() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.set(Square::new(3, 4), Some(Piece::new(PieceKind::Knight, Color::White)));
        let moves = board.pseudo_legal_knight_moves();
        assert_eq!(moves.len(), 8);
    }

    #[test]
    fn knight_in_corner_has_two_moves() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.set(Square::new(0, 0), Some(Piece::new(PieceKind::Knight, Color::White)));
        let moves = board.pseudo_legal_knight_moves();
        assert_eq!(moves.len(), 2);
    }

    #[test]
    fn knight_cannot_capture_friendly() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.set(Square::new(3, 4), Some(Piece::new(PieceKind::Knight, Color::White)));
        board.set(Square::new(5, 5), Some(Piece::new(PieceKind::Pawn, Color::White)));
        let moves = board.pseudo_legal_knight_moves();
        assert!(!moves.contains(&Move::new(Square::new(3, 4), Square::new(5, 5))));
        assert_eq!(moves.len(), 7);
    }

    #[test]
    fn king_in_center_has_eight_moves() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.set(Square::new(3, 4), Some(Piece::new(PieceKind::King, Color::White)));
        let moves = board.pseudo_legal_king_moves();
        assert_eq!(moves.len(), 8);
    }

    #[test]
    fn king_in_corner_has_three_moves() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.set(Square::new(0, 0), Some(Piece::new(PieceKind::King, Color::White)));
        let moves = board.pseudo_legal_king_moves();
        assert_eq!(moves.len(), 3);
    }

    // --- Sliding piece tests ---

    #[test]
    fn bishop_in_center_open_board_has_13_moves() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.set(Square::new(3, 3), Some(Piece::new(PieceKind::Bishop, Color::White)));
        let moves = board.pseudo_legal_bishop_moves();
        assert_eq!(moves.len(), 13);
    }

    #[test]
    fn rook_in_center_open_board_has_14_moves() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.set(Square::new(3, 3), Some(Piece::new(PieceKind::Rook, Color::White)));
        let moves = board.pseudo_legal_rook_moves();
        assert_eq!(moves.len(), 14);
    }

    // --- Check detection ---

    #[test]
    fn king_in_check_by_rook() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.set(Square::new(0, 4), Some(Piece::new(PieceKind::King, Color::White)));
        board.set(Square::new(7, 4), Some(Piece::new(PieceKind::Rook, Color::Black)));

        assert!(board.is_in_check(Color::White));
        assert!(!board.is_in_check(Color::Black));
    }

    #[test]
    fn king_in_check_by_bishop() {
        let mut board = Board::empty();
        board.set(Square::new(0, 4), Some(Piece::new(PieceKind::King, Color::White)));
        board.set(Square::new(4, 0), Some(Piece::new(PieceKind::Bishop, Color::Black)));

        assert!(board.is_in_check(Color::White));
    }

    #[test]
    fn king_in_check_by_knight() {
        let mut board = Board::empty();
        board.set(Square::new(0, 4), Some(Piece::new(PieceKind::King, Color::White)));
        // Knight on b3 (rank 2, file 1) — attacks d1 but not e1
        // Knight on d2 (rank 1, file 3) — attacks e4, c4, b1, f1, b3, f3... not e1
        // Knight on f2 (rank 1, file 5) — attacks e4, g4, d1, h1, d3, h3... not e1
        // Knight on c2 (rank 1, file 2) — attacks a1, a3, b4, d4, e1, e3 — YES attacks e1!
        board.set(Square::new(1, 2), Some(Piece::new(PieceKind::Knight, Color::Black)));

        assert!(board.is_in_check(Color::White));
    }

    #[test]
    fn king_in_check_by_pawn() {
        let mut board = Board::empty();
        // White king on e4 (rank 3, file 4), black pawn on d5 (rank 4, file 3)
        // Black pawn attacks e4 diagonally? No — black pawn advances downward (decreasing rank),
        // so a black pawn on d5 attacks c4 and e4. Yes!
        board.set(Square::new(3, 4), Some(Piece::new(PieceKind::King, Color::White)));
        board.set(Square::new(4, 3), Some(Piece::new(PieceKind::Pawn, Color::Black)));

        assert!(board.is_in_check(Color::White));
    }

    #[test]
    fn king_not_in_check_when_blocked() {
        let mut board = Board::empty();
        board.set(Square::new(0, 4), Some(Piece::new(PieceKind::King, Color::White)));
        board.set(Square::new(7, 4), Some(Piece::new(PieceKind::Rook, Color::Black)));
        // Friendly piece blocks the rook's line
        board.set(Square::new(3, 4), Some(Piece::new(PieceKind::Pawn, Color::White)));

        assert!(!board.is_in_check(Color::White));
    }

    // --- Legal moves ---

    #[test]
    fn pinned_piece_cannot_move_off_pin_line() {
        // White king on e1 (0,4), white rook on e4 (3,4), black rook on e8 (7,4)
        // The white rook is pinned along the e-file.
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.set(Square::new(0, 4), Some(Piece::new(PieceKind::King, Color::White)));
        board.set(Square::new(3, 4), Some(Piece::new(PieceKind::Rook, Color::White)));
        board.set(Square::new(7, 4), Some(Piece::new(PieceKind::Rook, Color::Black)));
        board.set(Square::new(7, 0), Some(Piece::new(PieceKind::King, Color::Black)));

        let legal = board.legal_moves();

        // Pinned rook should only be able to move along the e-file
        for mv in &legal {
            if mv.from == Square::new(3, 4) {
                // Must stay on file 4
                assert_eq!(mv.to.file, 4, "pinned rook moved off pin line: {:?}", mv);
            }
        }
    }

    #[test]
    fn king_cannot_move_into_check() {
        // White king on e1, black queen on e8 covering the entire e-file and also d8, f8...
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.set(Square::new(0, 4), Some(Piece::new(PieceKind::King, Color::White)));
        board.set(Square::new(7, 4), Some(Piece::new(PieceKind::Queen, Color::Black)));
        board.set(Square::new(7, 0), Some(Piece::new(PieceKind::King, Color::Black)));

        let legal = board.legal_moves();

        // King must not move to any square covered by the queen
        for mv in &legal {
            if mv.from == Square::new(0, 4) {
                let after = board.apply_move(*mv);
                assert!(
                    !after.is_in_check(Color::White),
                    "king moved into check: {:?}", mv
                );
            }
        }
    }

    #[test]
    fn only_king_moves_in_double_check() {
        // Construct a double check: white king on e1, attacked by both black rook on e8
        // and black knight on d3 (rank 2, file 3). Only king moves should be legal.
        // Knight on d3 (rank 2, file 3) attacks e1 (rank 0, file 4): dr=2, df=1 — not valid knight jump.
        // Let's use: king e1 (0,4), black rook on e8 (7,4), black bishop on h4 (3,7) checking diagonally?
        // Bishop on h4 (3,7) vs king e1 (0,4): dr=-3, df=-3 — yes, diagonal.
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.set(Square::new(0, 4), Some(Piece::new(PieceKind::King, Color::White)));
        board.set(Square::new(0, 0), Some(Piece::new(PieceKind::Rook, Color::White)));
        board.set(Square::new(7, 4), Some(Piece::new(PieceKind::Rook, Color::Black)));
        board.set(Square::new(3, 7), Some(Piece::new(PieceKind::Bishop, Color::Black)));
        board.set(Square::new(7, 0), Some(Piece::new(PieceKind::King, Color::Black)));

        let legal = board.legal_moves();

        // All legal moves must be king moves
        for mv in &legal {
            assert_eq!(
                mv.from, Square::new(0, 4),
                "non-king move generated under double check: {:?}", mv
            );
        }
    }

    #[test]
    fn castling_kingside_generated_when_legal() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.castling.white_kingside = true;
        board.set(Square::new(0, 4), Some(Piece::new(PieceKind::King, Color::White)));
        board.set(Square::new(0, 7), Some(Piece::new(PieceKind::Rook, Color::White)));
        board.set(Square::new(7, 0), Some(Piece::new(PieceKind::King, Color::Black)));

        let legal = board.legal_moves();
        assert!(
            legal.contains(&Move::new(Square::new(0, 4), Square::new(0, 6))),
            "kingside castling not generated"
        );
    }

    #[test]
    fn castling_not_generated_when_in_check() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.castling.white_kingside = true;
        board.set(Square::new(0, 4), Some(Piece::new(PieceKind::King, Color::White)));
        board.set(Square::new(0, 7), Some(Piece::new(PieceKind::Rook, Color::White)));
        board.set(Square::new(7, 4), Some(Piece::new(PieceKind::Rook, Color::Black)));
        board.set(Square::new(7, 0), Some(Piece::new(PieceKind::King, Color::Black)));

        let legal = board.legal_moves();
        assert!(
            !legal.contains(&Move::new(Square::new(0, 4), Square::new(0, 6))),
            "castling generated while in check"
        );
    }

    #[test]
    fn castling_not_generated_through_attacked_square() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.castling.white_kingside = true;
        board.set(Square::new(0, 4), Some(Piece::new(PieceKind::King, Color::White)));
        board.set(Square::new(0, 7), Some(Piece::new(PieceKind::Rook, Color::White)));
        // Black rook controls f1 (rank 0, file 5)
        board.set(Square::new(7, 5), Some(Piece::new(PieceKind::Rook, Color::Black)));
        board.set(Square::new(7, 0), Some(Piece::new(PieceKind::King, Color::Black)));

        let legal = board.legal_moves();
        assert!(
            !legal.contains(&Move::new(Square::new(0, 4), Square::new(0, 6))),
            "castling generated through attacked square"
        );
    }

    #[test]
    fn en_passant_legal_move() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.en_passant_file = Some(3);
        board.set(Square::new(4, 4), Some(Piece::new(PieceKind::Pawn, Color::White)));
        board.set(Square::new(4, 3), Some(Piece::new(PieceKind::Pawn, Color::Black)));
        board.set(Square::new(0, 4), Some(Piece::new(PieceKind::King, Color::White)));
        board.set(Square::new(7, 4), Some(Piece::new(PieceKind::King, Color::Black)));

        let legal = board.legal_moves();
        assert!(
            legal.contains(&Move::new(Square::new(4, 4), Square::new(5, 3))),
            "en passant move not generated"
        );
    }

    #[test]
    fn promotion_in_legal_moves() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.set(Square::new(6, 0), Some(Piece::new(PieceKind::Pawn, Color::White)));
        board.set(Square::new(0, 4), Some(Piece::new(PieceKind::King, Color::White)));
        board.set(Square::new(7, 4), Some(Piece::new(PieceKind::King, Color::Black)));

        let legal = board.legal_moves();
        let promos: Vec<_> = legal.iter()
            .filter(|m| m.from == Square::new(6, 0) && m.to == Square::new(7, 0))
            .collect();
        assert_eq!(promos.len(), 4, "expected 4 promotion moves, got {}", promos.len());
    }

    // --- Perft tests ---

    #[test]
    fn perft_starting_position_depth_1() {
        let board = Board::starting_position();
        assert_eq!(board.perft(1), 20);
    }

    #[test]
    fn perft_starting_position_depth_2() {
        let board = Board::starting_position();
        assert_eq!(board.perft(2), 400);
    }

    #[test]
    fn perft_starting_position_depth_3() {
        let board = Board::starting_position();
        assert_eq!(board.perft(3), 8902);
    }

    #[test]
    fn perft_starting_position_depth_4() {
        let board = Board::starting_position();
        assert_eq!(board.perft(4), 197281);
    }

    /// Position with castling rights (kingside only for white).
    /// FEN: "5k2/8/8/8/8/8/8/4K2R w K - 0 1"
    /// Depth 4 verified against chesslib reference.
    #[test]
    fn perft_castling_position_depth_4() {
        let board = Board::from_fen("5k2/8/8/8/8/8/8/4K2R w K - 0 1").unwrap();
        assert_eq!(board.perft(4), 6399);
    }

    /// Position with en passant available.
    /// FEN: "8/5bk1/8/2Pp4/8/1K6/8/8 w - d6 0 1"
    /// Depth 4 verified against chesslib reference.
    #[test]
    fn perft_en_passant_position_depth_4() {
        let board = Board::from_fen("8/5bk1/8/2Pp4/8/1K6/8/8 w - d6 0 1").unwrap();
        assert_eq!(board.perft(4), 9287);
    }

    /// Position with promotion opportunities.
    /// FEN: "2K2r2/4P3/8/8/8/8/8/3k4 w - - 0 1"
    /// Depth 4 verified against chesslib reference.
    #[test]
    fn perft_promotion_position_depth_4() {
        let board = Board::from_fen("2K2r2/4P3/8/8/8/8/8/3k4 w - - 0 1").unwrap();
        assert_eq!(board.perft(4), 19174);
    }
}
