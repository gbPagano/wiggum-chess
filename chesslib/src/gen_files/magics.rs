use super::attacks::gen_magic_attack_map;
use super::rays::*;
use crate::bitboard::BitBoard;
use crate::pieces::Piece;
use crate::square::Square;
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::SmallRng;
use std::fs::File;
use std::io::Write;
use std::sync::Mutex;

pub fn magic_mask(square: Square, piece: Piece) -> BitBoard {
    get_rays(square, piece)
        & !Square::all_squares()
            .filter(|other| match piece {
                Piece::Bishop => other.is_edge(),
                Piece::Rook => {
                    (square.get_rank() == other.get_rank() && (other.get_file().is_edge()))
                        || (square.get_file() == other.get_file() && (other.get_rank().is_edge()))
                }
                _ => panic!("Magic only for Rooks and Bishops"),
            })
            .fold(BitBoard(0), |b, s| b | BitBoard::from_square(s))
}

#[derive(Copy, Clone)]
struct Magic {
    magic_number: BitBoard,
    mask: BitBoard,
    offset: u32,
    rightshift: u8,
}

static MAGIC_NUMBERS: Mutex<[[Magic; 64]; 2]> = Mutex::new(
    [[Magic {
        magic_number: BitBoard(0),
        mask: BitBoard(0),
        offset: 0,
        rightshift: 0,
    }; 64]; 2],
); // for rooks and bishops

const NUM_MOVES: usize = 64 * (1<<12) /* Rook Moves */ +
                         64 * (1<<9) /* Bishop Moves */;
static MOVES_MAX_IDX: Mutex<usize> = Mutex::new(0);
static MOVES: Mutex<[BitBoard; NUM_MOVES]> = Mutex::new([BitBoard(0); NUM_MOVES]);
static MOVE_RAYS: Mutex<[BitBoard; NUM_MOVES]> = Mutex::new([BitBoard(0); NUM_MOVES]);

fn generate_magic(square: Square, piece: Piece, curr_offset: usize) -> usize {
    let (blockers, attacks) = gen_magic_attack_map(square, piece);
    let mask = magic_mask(square, piece);

    let mut move_rays = MOVE_RAYS.lock().unwrap();
    let mut moves = MOVES.lock().unwrap();
    let mut magic_numbers = MAGIC_NUMBERS.lock().unwrap();

    let mut new_offset = curr_offset;
    for i in 0..curr_offset {
        let mut found = true;
        for j in 0..attacks.len() {
            if move_rays[i + j] & get_rays(square, piece) != BitBoard(0) {
                found = false;
                break;
            }
        }
        if found {
            new_offset = i;
            break;
        }
    }

    let mut magic = Magic {
        magic_number: BitBoard(0),
        mask,
        offset: new_offset as u32,
        rightshift: ((blockers.len() as u64).leading_zeros() + 1) as u8,
    };

    // TODO: tranform this into unittest
    assert_eq!(blockers.len().count_ones(), 1);
    assert_eq!(blockers.len(), attacks.len());

    assert_eq!(blockers.iter().fold(BitBoard(0), |b, n| b | *n), mask);
    assert_eq!(
        attacks.iter().fold(BitBoard(0), |b, n| b | *n),
        get_rays(square, piece)
    );

    let mut rng = SmallRng::from_os_rng();

    let mut done = false;
    while !done {
        let magic_number =
            BitBoard::new(rng.random::<u64>() & rng.random::<u64>() & rng.random::<u64>());

        if (mask * magic_number).0.count_ones() < 6 {
            continue;
        }
        done = true;

        let mut new_attacks = vec![BitBoard(0); blockers.len()];
        for (i, &blocker) in blockers.iter().enumerate() {
            let j = ((magic_number * blocker) >> magic.rightshift).0 as usize;
            if new_attacks[j] == BitBoard(0) || new_attacks[j] == attacks[i] {
                new_attacks[j] = attacks[i];
            } else {
                done = false;
                break;
            }
        }

        if done {
            magic.magic_number = magic_number;
        }
    }

    magic_numbers[if piece == Piece::Rook { 0 } else { 1 }][square.to_index()] = magic;

    for (i, &blocker) in blockers.iter().enumerate() {
        let j = ((magic.magic_number * blocker) >> magic.rightshift).0 as usize;
        moves[magic.offset as usize + j] |= attacks[i];
        move_rays[magic.offset as usize + j] |= get_rays(square, piece);
    }

    if new_offset + attacks.len() < curr_offset {
        curr_offset
    } else {
        new_offset + attacks.len()
    }
}

pub fn gen_all_magic() {
    let mut offset = 0;
    for piece in [Piece::Rook, Piece::Bishop].iter() {
        for square in Square::all_squares() {
            offset = generate_magic(square, *piece, offset);
        }
    }
    *MOVES_MAX_IDX.lock().unwrap() = offset;
    dbg!(&MOVES_MAX_IDX);
}

pub fn write_magics(f: &mut File) {
    let magic_struct = r#"#[derive(Copy, Clone)]
struct Magic {
    magic_number: BitBoard,
    mask: BitBoard,
    offset: u32,
    rightshift: u8
}
"#
    .to_string();
    writeln!(f, "{}", magic_struct).unwrap();

    let magic_numbers = MAGIC_NUMBERS.lock().unwrap();
    writeln!(f, "const MAGIC_NUMBERS: [[Magic; 64]; 2] = [[").unwrap();
    for rook_magic in magic_numbers[0].iter() {
        writeln!(f, "    Magic {{ magic_number: BitBoard({}), mask: BitBoard({}), offset: {}, rightshift: {} }},",
            rook_magic.magic_number.0,
            rook_magic.mask.0,
            rook_magic.offset,
            rook_magic.rightshift
        ).unwrap();
    }

    writeln!(f, "], [").unwrap();
    for bishop_magic in magic_numbers[1].iter() {
        writeln!(f, "    Magic {{ magic_number: BitBoard({}), mask: BitBoard({}), offset: {}, rightshift: {} }},",
            bishop_magic.magic_number.0,
            bishop_magic.mask.0,
            bishop_magic.offset,
            bishop_magic.rightshift
        ).unwrap();
    }
    writeln!(f, "]];").unwrap();

    writeln!(
        f,
        "const MOVES: [BitBoard; {}] = [",
        MOVES_MAX_IDX.lock().unwrap()
    )
    .unwrap();
    let moves = MOVES.lock().unwrap();
    for move_bb in moves.iter().take(*MOVES_MAX_IDX.lock().unwrap()) {
        writeln!(f, "    BitBoard({}),", move_bb.0).unwrap();
    }
    writeln!(f, "];").unwrap();
}

#[test]
fn name() {
    //find_magic("c3".parse().unwrap(), Piece::Rook, 0);

    gen_all_magic();

    assert!(false);
}
