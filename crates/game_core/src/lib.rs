pub mod board;
pub mod protocol;

pub use board::{Board, GameBoard, ShotOutcome, ShipSpec};
pub use protocol::{Fact, Intent};

pub const PROTOCOL_ID: u64 = 0x4241_5454_4c_45;
pub const DEFAULT_PORT: u16 = 5000;
