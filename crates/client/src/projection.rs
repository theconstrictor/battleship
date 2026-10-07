use bevy::prelude::Resource;
use game_core::{
    Board, Fact, ShipSpec,
    board::GRID_SIZE,
};
use rand::Rng;

use crate::screen::Screen;

pub const GRID: usize = GRID_SIZE;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnCellView {
    Water,
    Ship,
    Miss,
    Hit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyCellView {
    Unknown,
    Miss,
    Hit,
    Sunk,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyShot {
    Miss,
    Hit,
    Sunk,
}

/// What the projection asks the wiring to do after a fact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenAction {
    None,
    Enter(Screen),
    OpponentLeft,
}

/// Everything the client renders, derived purely from facts.
///
/// Facts mutate this; screens and cells project from it. The client's own
/// intents never write here (the submitted fleet is recorded only when the
/// `ShipsPlaced` intent is sent, and it is the authority's re-validation
/// that matters anyway).
#[derive(Resource, Debug, Default)]
pub struct Projection {
    pub seat: Option<u8>,
    pub opponent_name: Option<String>,
    pub your_turn: Option<bool>,
    pub status: String,
    pub own_fleet: Vec<ShipSpec>,
    pub own_damage: [[bool; GRID]; GRID],
    pub enemy_shots: [[Option<EnemyShot>; GRID]; GRID],
    pub winner: Option<u8>,
    pub rematch_rejected: bool,
}

impl Projection {
    pub fn apply_fact(&mut self, fact: &Fact) -> ScreenAction {
        match fact {
            Fact::Paired {
                your_index,
                opponent_name,
            } => {
                self.seat = Some(*your_index);
                self.opponent_name = Some(opponent_name.clone());
                self.reset_boards();
                ScreenAction::Enter(Screen::Placement)
            }
            Fact::GameStart => ScreenAction::Enter(Screen::Battle),
            Fact::ShotResult {
                x,
                y,
                hit,
                sunk,
                shooter,
                ..
            } => {
                let (x, y) = (*x as usize, *y as usize);
                if self.seat == Some(*shooter) {
                    self.enemy_shots[y][x] = Some(match (*hit, *sunk) {
                        (true, true) => EnemyShot::Sunk,
                        (true, false) => EnemyShot::Hit,
                        (false, _) => EnemyShot::Miss,
                    });
                    self.status = match (*hit, *sunk) {
                        (true, true) => "You sank an enemy ship!".into(),
                        (true, false) => "You hit an enemy ship!".into(),
                        (false, _) => "You missed.".into(),
                    };
                } else {
                    self.own_damage[y][x] = true;
                    self.status = match (*hit, *sunk) {
                        (true, true) => "The enemy sank your ship!".into(),
                        (true, false) => "The enemy hit your ship!".into(),
                        (false, _) => "The enemy missed.".into(),
                    };
                }
                ScreenAction::None
            }
            Fact::Turn { your_turn } => {
                self.your_turn = Some(*your_turn);
                self.status = if *your_turn { "Your turn" } else { "Opponent's turn" }.into();
                ScreenAction::None
            }
            Fact::GameOver { winner } => {
                self.winner = Some(*winner);
                self.status = if self.seat == Some(*winner) {
                    "You win!".into()
                } else {
                    "You lose.".into()
                };
                ScreenAction::Enter(Screen::GameOver)
            }
            Fact::Rematch => {
                self.reset_boards();
                ScreenAction::Enter(Screen::Placement)
            }
            Fact::Error { message } => {
                if message == "Opponent disconnected" {
                    ScreenAction::OpponentLeft
                } else {
                    self.status = message.clone();
                    if message == "No finished game to rematch" {
                        self.rematch_rejected = true;
                    }
                    ScreenAction::None
                }
            }
        }
    }

    fn reset_boards(&mut self) {
        self.own_fleet.clear();
        self.own_damage = [[false; GRID]; GRID];
        self.enemy_shots = [[None; GRID]; GRID];
        self.your_turn = None;
        self.winner = None;
        self.rematch_rejected = false;
        self.status.clear();
    }

    pub fn record_fleet(&mut self, ships: Vec<ShipSpec>) {
        self.own_fleet = ships;
    }

    pub fn own_cell(&self, x: usize, y: usize) -> OwnCellView {
        let has_ship = self
            .own_fleet
            .iter()
            .any(|s| s.cells().any(|(cx, cy)| cx as usize == x && cy as usize == y));
        match (has_ship, self.own_damage[y][x]) {
            (true, true) => OwnCellView::Hit,
            (true, false) => OwnCellView::Ship,
            (false, true) => OwnCellView::Miss,
            (false, false) => OwnCellView::Water,
        }
    }

    pub fn enemy_cell(&self, x: usize, y: usize) -> EnemyCellView {
        match self.enemy_shots[y][x] {
            None => EnemyCellView::Unknown,
            Some(EnemyShot::Miss) => EnemyCellView::Miss,
            Some(EnemyShot::Hit) => EnemyCellView::Hit,
            Some(EnemyShot::Sunk) => EnemyCellView::Sunk,
        }
    }
}

/// The placement screen's working fleet: which ships are placed, which one
/// the player is carrying, and whether the fleet has been submitted.
///
/// The convenience check lives here: a carried ship can only be dropped on
/// an in-bounds, non-overlapping anchor, and Ready is only offered once all
/// five ships are placed. The authority re-validates the fleet on receive.
#[derive(Resource, Debug, Default)]
pub struct FleetEditor {
    pub placed: [Option<ShipSpec>; 5],
    pub carried: Option<ShipSpec>,
    pub submitted: bool,
    pub hover: Option<(u8, u8)>,
}

impl FleetEditor {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn can_place(&self, spec: ShipSpec) -> bool {
        if !spec.in_bounds() {
            return false;
        }
        let cells: Vec<(u8, u8)> = spec.cells().collect();
        !self
            .placed
            .iter()
            .flatten()
            .filter(|p| p.ship != spec.ship)
            .any(|p| p.cells().any(|c| cells.contains(&c)))
    }

    pub fn pick_up_from_palette(&mut self, ship: u8) {
        if self.submitted || self.carried.is_some() {
            return;
        }
        if (ship as usize) < self.placed.len() && self.placed[ship as usize].is_none() {
            self.carried = Some(ShipSpec {
                ship,
                x: 0,
                y: 0,
                horizontal: true,
            });
        }
    }

    pub fn pick_up_at(&mut self, x: u8, y: u8) {
        if self.submitted || self.carried.is_some() {
            return;
        }
        if let Some(index) = self
            .placed
            .iter()
            .position(|s| s.is_some_and(|s| s.cells().any(|(cx, cy)| cx == x && cy == y)))
        {
            self.carried = self.placed[index].take();
        }
    }

    pub fn place_carried_at(&mut self, x: u8, y: u8) -> bool {
        if self.submitted {
            return false;
        }
        let Some(mut spec) = self.carried else {
            return false;
        };
        spec.x = x;
        spec.y = y;
        if !self.can_place(spec) {
            return false;
        }
        self.placed[spec.ship as usize] = Some(spec);
        self.carried = None;
        true
    }

    pub fn rotate_carried(&mut self) {
        if let Some(carried) = &mut self.carried {
            carried.horizontal = !carried.horizontal;
        }
    }

    pub fn return_carried(&mut self) {
        self.carried = None;
    }

    pub fn fleet(&self) -> Option<Vec<ShipSpec>> {
        if self.carried.is_none() && self.placed.iter().all(Option::is_some) {
            Some(self.placed.iter().flatten().copied().collect())
        } else {
            None
        }
    }

    pub fn ready_allowed(&self) -> bool {
        !self.submitted && self.fleet().is_some()
    }

    pub fn randomize(&mut self, rng: &mut impl Rng) {
        self.placed = Default::default();
        self.carried = None;
        for spec in Board::random(rng).ships() {
            self.placed[spec.ship as usize] = Some(spec);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(ship: u8, x: u8, y: u8, horizontal: bool) -> ShipSpec {
        ShipSpec {
            ship,
            x,
            y,
            horizontal,
        }
    }

    fn shot(x: u8, y: u8, hit: bool, sunk: bool, shooter: u8) -> Fact {
        Fact::ShotResult {
            x,
            y,
            hit,
            sunk,
            game_over: false,
            shooter,
        }
    }

    #[test]
    fn paired_sets_seat_and_enters_placement() {
        let mut p = Projection::default();
        assert_eq!(
            p.apply_fact(&Fact::Paired {
                your_index: 1,
                opponent_name: "alice".into(),
            }),
            ScreenAction::Enter(Screen::Placement)
        );
        assert_eq!(p.seat, Some(1));
        assert_eq!(p.opponent_name.as_deref(), Some("alice"));
    }

    #[test]
    fn game_start_enters_battle() {
        let mut p = Projection::default();
        assert_eq!(
            p.apply_fact(&Fact::GameStart),
            ScreenAction::Enter(Screen::Battle)
        );
    }

    #[test]
    fn own_shots_update_enemy_board_opponent_shots_update_own_damage() {
        let mut p = Projection {
            seat: Some(0),
            ..Default::default()
        };
        p.apply_fact(&shot(3, 4, true, false, 0));
        p.apply_fact(&shot(7, 8, false, false, 1));
        assert_eq!(p.enemy_cell(3, 4), EnemyCellView::Hit);
        assert_eq!(p.enemy_cell(7, 8), EnemyCellView::Unknown);
        assert_eq!(p.own_cell(7, 8), OwnCellView::Miss);
        assert_eq!(p.own_cell(3, 4), OwnCellView::Water);
        assert_eq!(p.status, "The enemy missed.");
    }

    #[test]
    fn sunk_shot_marks_enemy_cell_sunk() {
        let mut p = Projection {
            seat: Some(1),
            ..Default::default()
        };
        p.apply_fact(&shot(0, 0, true, true, 1));
        assert_eq!(p.enemy_cell(0, 0), EnemyCellView::Sunk);
        assert_eq!(p.status, "You sank an enemy ship!");
    }

    #[test]
    fn turn_sets_your_turn_and_status() {
        let mut p = Projection::default();
        p.apply_fact(&Fact::Turn { your_turn: false });
        assert_eq!(p.your_turn, Some(false));
        assert_eq!(p.status, "Opponent's turn");
        p.apply_fact(&Fact::Turn { your_turn: true });
        assert_eq!(p.your_turn, Some(true));
        assert_eq!(p.status, "Your turn");
    }

    #[test]
    fn game_over_records_winner_and_enters_game_over() {
        let mut p = Projection {
            seat: Some(0),
            ..Default::default()
        };
        assert_eq!(
            p.apply_fact(&Fact::GameOver { winner: 0 }),
            ScreenAction::Enter(Screen::GameOver)
        );
        assert_eq!(p.winner, Some(0));
        assert_eq!(p.status, "You win!");
    }

    #[test]
    fn rematch_resets_boards_and_enters_placement() {
        let mut p = Projection {
            seat: Some(0),
            ..Default::default()
        };
        p.record_fleet(vec![spec(0, 0, 0, true)]);
        p.apply_fact(&shot(1, 1, true, false, 0));
        p.apply_fact(&shot(2, 2, false, false, 1));
        p.apply_fact(&Fact::Turn { your_turn: true });
        assert_eq!(
            p.apply_fact(&Fact::Rematch),
            ScreenAction::Enter(Screen::Placement)
        );
        assert!(p.own_fleet.is_empty());
        assert_eq!(p.own_cell(2, 2), OwnCellView::Water);
        assert_eq!(p.enemy_cell(1, 1), EnemyCellView::Unknown);
        assert_eq!(p.your_turn, None);
        assert_eq!(p.winner, None);
    }

    #[test]
    fn error_sets_status_but_opponent_disconnect_asks_to_leave() {
        let mut p = Projection::default();
        assert_eq!(
            p.apply_fact(&Fact::Error {
                message: "Not your turn".into(),
            }),
            ScreenAction::None
        );
        assert_eq!(p.status, "Not your turn");
        assert_eq!(
            p.apply_fact(&Fact::Error {
                message: "Opponent disconnected".into(),
            }),
            ScreenAction::OpponentLeft
        );
    }

    #[test]
    fn rematch_race_error_hides_the_rematch_button() {
        let mut p = Projection::default();
        p.apply_fact(&Fact::Error {
            message: "No finished game to rematch".into(),
        });
        assert!(p.rematch_rejected);
    }

    #[test]
    fn own_cell_projects_fleet_and_damage() {
        let mut p = Projection::default();
        p.record_fleet(vec![spec(0, 2, 2, true)]);
        assert_eq!(p.own_cell(2, 2), OwnCellView::Ship);
        assert_eq!(p.own_cell(6, 2), OwnCellView::Ship);
        assert_eq!(p.own_cell(7, 2), OwnCellView::Water);
        p.own_damage[2][4] = true;
        assert_eq!(p.own_cell(4, 2), OwnCellView::Hit);
        assert_eq!(p.own_cell(9, 9), OwnCellView::Water);
    }

    #[test]
    fn editor_pick_up_place_and_rotate() {
        let mut e = FleetEditor::default();
        e.pick_up_from_palette(0);
        assert!(e.place_carried_at(0, 0));
        assert!(e.carried.is_none());
        e.pick_up_at(3, 0);
        assert_eq!(e.carried, Some(spec(0, 0, 0, true)));
        e.rotate_carried();
        assert!(e.place_carried_at(0, 0));
        assert_eq!(e.placed[0], Some(spec(0, 0, 0, false)));
    }

    #[test]
    fn editor_refuses_overlap_and_out_of_bounds() {
        let mut e = FleetEditor::default();
        e.pick_up_from_palette(0);
        assert!(e.place_carried_at(0, 0));
        e.pick_up_from_palette(1);
        assert!(!e.place_carried_at(2, 0), "overlapping a placed ship");
        assert!(e.carried.is_some(), "failed drop keeps the carried ship");
        assert!(!e.place_carried_at(7, 0), "battleship (size 4) at x=7 is out of bounds");
        assert!(e.place_carried_at(5, 0));
        assert_eq!(e.placed[1], Some(spec(1, 5, 0, true)));
    }

    #[test]
    fn editor_ready_gated_on_complete_fleet() {
        let mut e = FleetEditor::default();
        assert!(!e.ready_allowed());
        for ship in 0..5 {
            e.pick_up_from_palette(ship);
            assert!(e.place_carried_at(0, ship * 2), "ship {ship}");
        }
        assert!(e.ready_allowed());
        e.submitted = true;
        assert!(!e.ready_allowed());
    }

    #[test]
    fn editor_randomize_yields_a_ready_fleet() {
        let mut e = FleetEditor::default();
        e.randomize(&mut rand::rng());
        assert!(e.fleet().is_some());
        assert!(e.ready_allowed());
        let specs = e.fleet().unwrap();
        assert_eq!(specs.len(), 5);
        Board::from_specs(&specs).expect("randomized fleet must be valid");
    }
}
