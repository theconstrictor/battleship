use serde::{Deserialize, Serialize};

use crate::board::ShipSpec;

/// A message sent by a client to the server.
///
/// Intents are requests and claims, not statements of fact:
/// the server is free to accept or reject any of them.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Intent {
    Join { name: String },
    ShipsPlaced { ships: Vec<ShipSpec> },
    Shot { x: u8, y: u8 },
    Rematch,
}

/// A message sent by the server to clients.
///
/// Facts are statements about the authoritative game state:
/// clients render them and cannot reject them.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Fact {
    Paired {
        your_index: u8,
        opponent_name: String,
    },
    GameStart,
    ShotResult {
        x: u8,
        y: u8,
        hit: bool,
        sunk: bool,
        game_over: bool,
        shooter: u8,
    },
    Turn {
        your_turn: bool,
    },
    GameOver {
        winner: u8,
    },
    Rematch,
    Error {
        message: String,
    },
}
