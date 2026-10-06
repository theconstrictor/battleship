use game_core::{
    GameBoard, ShipSpec, ShotOutcome,
    board::{SHIP_SIZES, Board, PlaceError},
    protocol::{Intent, Fact},
};
use renet::ClientId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Lobby,
    Placement,
    Battle,
    GameOver,
}

#[derive(Debug, Clone, Copy)]
pub enum Target {
    All,
    One(ClientId),
}

#[derive(Debug)]
pub struct OutMsg {
    pub target: Target,
    pub fact: Fact,
}

#[derive(Debug, Default)]
struct Player {
    name: String,
    board: Option<GameBoard>,
}

#[derive(Debug, Default)]
pub struct Game {
    players: Vec<Player>,
    ids: Vec<ClientId>,
    phase: Phase,
    turn: usize,
}

impl Default for Phase {
    fn default() -> Self {
        Phase::Lobby
    }
}

impl Game {
    pub fn on_connect(&mut self, id: ClientId) -> bool {
        if self.players.len() >= 2 {
            return false;
        }
        self.players.push(Player::default());
        self.ids.push(id);
        true
    }

    pub fn on_disconnect(&mut self, id: ClientId) -> Vec<OutMsg> {
        let Some(index) = self.index_of(id) else {
            return Vec::new();
        };
        self.players.remove(index);
        self.ids.remove(index);
        let mut out = Vec::new();
        if self.phase != Phase::Lobby {
            self.phase = Phase::Lobby;
            self.turn = 0;
            for player in &mut self.players {
                player.board = None;
            }
            for remaining in &self.ids {
                out.push(OutMsg {
                    target: Target::One(*remaining),
                    fact: Fact::Error {
                        message: "Opponent disconnected".to_string(),
                    },
                });
            }
        }
        out
    }

    pub fn apply(&mut self, from: ClientId, intent: Intent) -> Vec<OutMsg> {
        let mut out = Vec::new();
        let Some(index) = self.index_of(from) else {
            return out;
        };
        match intent {
            Intent::Join { name } => {
                self.players[index].name = name;
                if self.phase == Phase::Lobby && self.players.iter().all(|p| !p.name.is_empty()) {
                    self.phase = Phase::Placement;
                    out.extend(self.pairing_msgs());
                }
            }
            Intent::ShipsPlaced { ships } => {
                if self.phase != Phase::Placement {
                    return self.error_to(from, "Ships can only be placed during placement");
                }
                match validate_ships(&ships) {
                    Ok(board) => {
                        self.players[index].board = Some(GameBoard::new(board));
                        if self.players.iter().all(|p| p.board.is_some()) {
                            self.phase = Phase::Battle;
                            self.turn = 0;
                            out.push(OutMsg {
                                target: Target::All,
                                fact: Fact::GameStart,
                            });
                            out.extend(self.turn_msgs());
                        }
                    }
                    Err(message) => {
                        return self.error_to(from, &message);
                    }
                }
            }
            Intent::Shot { x, y } => {
                if self.phase != Phase::Battle {
                    return self.error_to(from, "No battle in progress");
                }
                if index != self.turn {
                    return self.error_to(from, "Not your turn");
                }
                if x as usize >= 10 || y as usize >= 10 {
                    return self.error_to(from, "Shot out of bounds");
                }
                let opponent = index ^ 1;
                let outcome = match self.players[opponent].board.as_mut() {
                    Some(board) => board.fire(x, y),
                    None => None,
                };
                let Some(outcome) = outcome else {
                    return self.error_to(from, "Cell already shot");
                };
                let (hit, sunk) = match outcome {
                    ShotOutcome::Miss => (false, false),
                    ShotOutcome::Hit { sunk } => (true, sunk),
                };
                let game_over = sunk && self.players[opponent].board.as_ref().is_some_and(|b| b.board.all_sunk(&b.shots));
                out.push(OutMsg {
                    target: Target::All,
                    fact: Fact::ShotResult {
                        x,
                        y,
                        hit,
                        sunk,
                        game_over,
                        shooter: index as u8,
                    },
                });
                if game_over {
                    self.phase = Phase::GameOver;
                    out.push(OutMsg {
                        target: Target::All,
                        fact: Fact::GameOver { winner: index as u8 },
                    });
                } else {
                    self.turn ^= 1;
                    out.extend(self.turn_msgs());
                }
            }
            Intent::Rematch => {
                if self.phase != Phase::GameOver {
                    return self.error_to(from, "No finished game to rematch");
                }
                for player in &mut self.players {
                    player.board = None;
                }
                self.phase = Phase::Placement;
                self.turn = 0;
                out.push(OutMsg {
                    target: Target::All,
                    fact: Fact::Rematch,
                });
            }
        }
        out
    }

    fn index_of(&self, id: ClientId) -> Option<usize> {
        self.ids.iter().position(|candidate| *candidate == id)
    }

    fn pairing_msgs(&self) -> Vec<OutMsg> {
        self.ids
            .iter()
            .enumerate()
            .map(|(index, id)| OutMsg {
                target: Target::One(*id),
                fact: Fact::Paired {
                    your_index: index as u8,
                    opponent_name: self.players[index ^ 1].name.clone(),
                },
            })
            .collect()
    }

    fn turn_msgs(&self) -> Vec<OutMsg> {
        self.ids
            .iter()
            .enumerate()
            .map(|(index, id)| OutMsg {
                target: Target::One(*id),
                fact: Fact::Turn {
                    your_turn: index == self.turn,
                },
            })
            .collect()
    }

    fn error_to(&self, to: ClientId, message: &str) -> Vec<OutMsg> {
        vec![OutMsg {
            target: Target::One(to),
            fact: Fact::Error {
                message: message.to_string(),
            },
        }]
    }
}

fn validate_ships(ships: &[ShipSpec]) -> Result<Board, String> {
    if ships.len() != SHIP_SIZES.len() {
        return Err(format!(
            "Expected {} ships, got {}",
            SHIP_SIZES.len(),
            ships.len()
        ));
    }
    let mut seen = [false; 5];
    for ship in ships {
        if ship.ship as usize >= SHIP_SIZES.len() || seen[ship.ship as usize] {
            return Err("Each ship type must appear exactly once".to_string());
        }
        seen[ship.ship as usize] = true;
    }
    Board::from_specs(ships).map_err(|error| match error {
        PlaceError::OutOfBounds => "Ship out of bounds".to_string(),
        PlaceError::Overlap => "Ships overlap".to_string(),
        PlaceError::UnknownShip => "Unknown ship".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: ClientId = 1;
    const B: ClientId = 2;

    fn placed() -> Game {
        let mut game = Game::default();
        assert!(game.on_connect(A));
        assert!(game.on_connect(B));
        game.apply(A, Intent::Join { name: "alice".into() });
        game.apply(B, Intent::Join { name: "bob".into() });
        game.apply(A, Intent::ShipsPlaced { ships: test_ships() });
        game.apply(B, Intent::ShipsPlaced { ships: test_ships() });
        game
    }

    fn test_ships() -> Vec<ShipSpec> {
        vec![
            ShipSpec { ship: 0, x: 0, y: 0, horizontal: true },
            ShipSpec { ship: 1, x: 0, y: 2, horizontal: true },
            ShipSpec { ship: 2, x: 0, y: 4, horizontal: true },
            ShipSpec { ship: 3, x: 0, y: 6, horizontal: true },
            ShipSpec { ship: 4, x: 0, y: 8, horizontal: true },
        ]
    }

    #[test]
    fn third_client_is_rejected() {
        let mut game = Game::default();
        assert!(game.on_connect(A));
        assert!(game.on_connect(B));
        assert!(!game.on_connect(3));
    }

    #[test]
    fn pairing_happens_once_both_have_names() {
        let mut game = Game::default();
        game.on_connect(A);
        game.on_connect(B);
        assert!(game.apply(A, Intent::Join { name: "alice".into() }).is_empty());
        let msgs = game.apply(B, Intent::Join { name: "bob".into() });
        assert_eq!(msgs.len(), 2);
        assert!(matches!(msgs[0].fact, Fact::Paired { .. }));
        assert_eq!(game.phase, Phase::Placement);
    }

    #[test]
    fn game_starts_when_both_place_valid_ships() {
        let mut game = Game::default();
        game.on_connect(A);
        game.on_connect(B);
        game.apply(A, Intent::Join { name: "alice".into() });
        game.apply(B, Intent::Join { name: "bob".into() });
        let msgs = game.apply(A, Intent::ShipsPlaced { ships: test_ships() });
        assert!(msgs.is_empty());
        let msgs = game.apply(B, Intent::ShipsPlaced { ships: test_ships() });
        assert_eq!(game.phase, Phase::Battle);
        assert!(matches!(msgs[0].fact, Fact::GameStart));
        assert!(matches!(msgs[1].fact, Fact::Turn { your_turn: true }));
        assert!(matches!(msgs[2].fact, Fact::Turn { your_turn: false }));
    }

    #[test]
    fn out_of_turn_shot_is_rejected() {
        let mut game = placed();
        let msgs = game.apply(B, Intent::Shot { x: 0, y: 0 });
        assert!(matches!(&msgs[0].fact, Fact::Error { .. }));
    }

    #[test]
    fn shot_reports_hit_then_miss_and_repeat_is_rejected() {
        let mut game = placed();
        let msgs = game.apply(A, Intent::Shot { x: 0, y: 0 });
        match &msgs[0].fact {
            Fact::ShotResult { hit, sunk, .. } => assert!(*hit && !*sunk),
            other => panic!("expected shot result, got {other:?}"),
        }
        let msgs = game.apply(B, Intent::Shot { x: 5, y: 5 });
        match &msgs[0].fact {
            Fact::ShotResult { hit, .. } => assert!(!*hit),
            other => panic!("expected shot result, got {other:?}"),
        }
        let msgs = game.apply(A, Intent::Shot { x: 0, y: 0 });
        assert!(matches!(&msgs[0].fact, Fact::Error { .. }));
    }

    #[test]
    fn out_of_bounds_shot_is_rejected() {
        let mut game = placed();
        let msgs = game.apply(A, Intent::Shot { x: 10, y: 0 });
        assert!(matches!(&msgs[0].fact, Fact::Error { .. }));
    }

    fn play_to_completion(game: &mut Game) -> u8 {
        let targets = [
            (0, 0), (1, 0), (2, 0), (3, 0), (4, 0),
            (0, 2), (1, 2), (2, 2), (3, 2),
            (0, 4), (1, 4), (2, 4),
            (0, 6), (1, 6), (2, 6),
            (0, 8), (1, 8),
        ];
        let misses: Vec<(u8, u8)> = (0..10).map(|y| (9, y)).chain((0..7).map(|y| (8, y))).collect();
        let mut winner = None;
        for (i, &(x, y)) in targets.iter().enumerate() {
            let msgs = game.apply(A, Intent::Shot { x, y });
            assert!(msgs.iter().all(|m| !matches!(m.fact, Fact::Error { .. })));
            if let Some(OutMsg { fact: Fact::GameOver { winner: w }, .. }) =
                msgs.iter().find(|m| matches!(m.fact, Fact::GameOver { .. }))
            {
                winner = Some(*w);
                break;
            }
            let (x, y) = misses[i];
            game.apply(B, Intent::Shot { x, y });
        }
        winner.expect("game should have ended")
    }

    #[test]
    fn sinking_everything_ends_the_game() {
        let mut game = placed();
        let winner = play_to_completion(&mut game);
        assert_eq!(winner, 0);
        assert_eq!(game.phase, Phase::GameOver);
    }

    #[test]
    fn invalid_ships_are_rejected() {
        let mut game = Game::default();
        game.on_connect(A);
        game.on_connect(B);
        game.apply(A, Intent::Join { name: "alice".into() });
        game.apply(B, Intent::Join { name: "bob".into() });
        let mut overlapping = test_ships();
        overlapping[1] = ShipSpec { ship: 1, x: 0, y: 0, horizontal: true };
        let msgs = game.apply(A, Intent::ShipsPlaced { ships: overlapping });
        assert!(matches!(&msgs[0].fact, Fact::Error { .. }));
        let mut missing = test_ships();
        missing.pop();
        let msgs = game.apply(A, Intent::ShipsPlaced { ships: missing });
        assert!(matches!(&msgs[0].fact, Fact::Error { .. }));
    }

    #[test]
    fn rematch_resets_to_placement() {
        let mut game = placed();
        play_to_completion(&mut game);
        assert_eq!(game.phase, Phase::GameOver);
        let msgs = game.apply(A, Intent::Rematch);
        assert!(matches!(&msgs[0].fact, Fact::Rematch));
        assert_eq!(game.phase, Phase::Placement);
    }

    #[test]
    fn disconnect_mid_game_notifies_and_resets() {
        let mut game = placed();
        let msgs = game.on_disconnect(B);
        assert_eq!(msgs.len(), 1);
        assert!(matches!(&msgs[0].fact, Fact::Error { .. }));
        assert_eq!(game.phase, Phase::Lobby);
        assert!(game.on_connect(B));
        game.apply(B, Intent::Join { name: "bob".into() });
        assert_eq!(game.phase, Phase::Placement);
    }
}
