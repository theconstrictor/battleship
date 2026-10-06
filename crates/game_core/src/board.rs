use rand::Rng;
use serde::{Deserialize, Serialize};

pub const GRID_SIZE: usize = 10;

pub const SHIP_SIZES: [u8; 5] = [5, 4, 3, 3, 2];

pub const SHIP_NAMES: [&str; 5] = ["Carrier", "Battleship", "Cruiser", "Submarine", "Destroyer"];

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShipSpec {
    pub ship: u8,
    pub x: u8,
    pub y: u8,
    pub horizontal: bool,
}

impl ShipSpec {
    pub fn size(&self) -> u8 {
        SHIP_SIZES[self.ship as usize]
    }

    pub fn cells(&self) -> impl Iterator<Item = (u8, u8)> + '_ {
        (0..self.size()).map(move |i| {
            if self.horizontal {
                (self.x + i, self.y)
            } else {
                (self.x, self.y + i)
            }
        })
    }

    pub fn in_bounds(&self) -> bool {
        self.cells()
            .all(|(x, y)| (x as usize) < GRID_SIZE && (y as usize) < GRID_SIZE)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Board {
    cells: [[Option<u8>; GRID_SIZE]; GRID_SIZE],
}

impl Board {
    pub fn new() -> Self {
        Self {
            cells: [[None; GRID_SIZE]; GRID_SIZE],
        }
    }

    pub fn from_specs(specs: &[ShipSpec]) -> Result<Self, PlaceError> {
        let mut board = Board::new();
        for spec in specs {
            board.place(*spec)?;
        }
        Ok(board)
    }

    pub fn place(&mut self, spec: ShipSpec) -> Result<(), PlaceError> {
        if spec.ship as usize >= SHIP_SIZES.len() {
            return Err(PlaceError::UnknownShip);
        }
        if !spec.in_bounds() {
            return Err(PlaceError::OutOfBounds);
        }
        let cells: Vec<(u8, u8)> = spec.cells().collect();
        if cells
            .iter()
            .any(|&(x, y)| self.cells[y as usize][x as usize].is_some())
        {
            return Err(PlaceError::Overlap);
        }
        for (x, y) in cells {
            self.cells[y as usize][x as usize] = Some(spec.ship);
        }
        Ok(())
    }

    pub fn ship_at(&self, x: u8, y: u8) -> Option<u8> {
        self.cells[y as usize][x as usize]
    }

    pub fn ships(&self) -> Vec<ShipSpec> {
        let mut specs: Vec<ShipSpec> = Vec::new();
        for y in 0..GRID_SIZE {
            for x in 0..GRID_SIZE {
                let Some(ship) = self.cells[y][x] else {
                    continue;
                };
                let horizontal = self.cells[y].get(x + 1) == Some(&Some(ship))
                    || x > 0 && self.cells[y][x - 1] == Some(ship);
                let vertical = y + 1 < GRID_SIZE && self.cells[y + 1][x] == Some(ship)
                    || y > 0 && self.cells[y - 1][x] == Some(ship);
                if horizontal && vertical {
                    continue;
                }
                let (ox, oy) = if horizontal {
                    let ox = (0..x)
                        .rev()
                        .take_while(|&px| self.cells[y][px] == Some(ship))
                        .last()
                        .map_or_else(|| x, |px| px);
                    (ox, y)
                } else {
                    let oy = (0..y)
                        .rev()
                        .take_while(|&py| self.cells[py][x] == Some(ship))
                        .last()
                        .map_or_else(|| y, |py| py);
                    (x, oy)
                };
                if ox == x && oy == y {
                    specs.push(ShipSpec {
                        ship,
                        x: ox as u8,
                        y: oy as u8,
                        horizontal,
                    });
                }
            }
        }
        specs
    }

    pub fn all_sunk(&self, shots: &Shots) -> bool {
        for y in 0..GRID_SIZE {
            for x in 0..GRID_SIZE {
                if self.cells[y][x].is_some() && !shots[y][x] {
                    return false;
                }
            }
        }
        true
    }

    pub fn random(rng: &mut impl Rng) -> Self {
        let mut board = Board::new();
        for ship in 0..SHIP_SIZES.len() {
            loop {
                let horizontal = rng.random_bool(0.5);
                let spec = ShipSpec {
                    ship: ship as u8,
                    x: rng.random_range(0..GRID_SIZE) as u8,
                    y: rng.random_range(0..GRID_SIZE) as u8,
                    horizontal,
                };
                if board.place(spec).is_ok() {
                    break;
                }
            }
        }
        board
    }
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

pub type Shots = [[bool; GRID_SIZE]; GRID_SIZE];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlaceError {
    UnknownShip,
    OutOfBounds,
    Overlap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotOutcome {
    Miss,
    Hit { sunk: bool },
}

#[derive(Clone, Debug)]
pub struct GameBoard {
    pub board: Board,
    pub shots: Shots,
}

impl GameBoard {
    pub fn new(board: Board) -> Self {
        Self {
            board,
            shots: [[false; GRID_SIZE]; GRID_SIZE],
        }
    }

    pub fn fire(&mut self, x: u8, y: u8) -> Option<ShotOutcome> {
        if self.shots[y as usize][x as usize] {
            return None;
        }
        self.shots[y as usize][x as usize] = true;
        Some(match self.board.ship_at(x, y) {
            Some(_) => ShotOutcome::Hit {
                sunk: self.board.all_sunk(&self.shots),
            },
            None => ShotOutcome::Miss,
        })
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

    #[test]
    fn place_ship_and_look_it_up() {
        let mut board = Board::new();
        board.place(spec(0, 2, 3, true)).unwrap();
        assert_eq!(board.ship_at(2, 3), Some(0));
        assert_eq!(board.ship_at(6, 3), Some(0));
        assert_eq!(board.ship_at(7, 3), None);
        assert_eq!(board.ship_at(2, 4), None);
    }

    #[test]
    fn rejects_out_of_bounds() {
        let mut board = Board::new();
        assert_eq!(
            board.place(spec(0, 6, 0, true)),
            Err(PlaceError::OutOfBounds)
        );
        assert_eq!(
            board.place(spec(0, 0, 9, false)),
            Err(PlaceError::OutOfBounds)
        );
    }

    #[test]
    fn rejects_overlap() {
        let mut board = Board::new();
        board.place(spec(0, 0, 0, true)).unwrap();
        assert_eq!(board.place(spec(1, 2, 0, false)), Err(PlaceError::Overlap));
        assert_eq!(board.place(spec(1, 5, 0, true)), Ok(()));
    }

    #[test]
    fn rejects_unknown_ship() {
        let mut board = Board::new();
        assert_eq!(
            board.place(spec(9, 0, 0, true)),
            Err(PlaceError::UnknownShip)
        );
    }

    #[test]
    fn fire_miss_hit_and_repeat() {
        let mut gb = GameBoard::new(Board::from_specs(&[spec(0, 0, 0, true)]).unwrap());
        assert_eq!(gb.fire(5, 5), Some(ShotOutcome::Miss));
        assert_eq!(gb.fire(0, 0), Some(ShotOutcome::Hit { sunk: false }));
        assert_eq!(gb.fire(0, 0), None);
    }

    #[test]
    fn fire_sinks_ship_on_last_cell() {
        let mut gb = GameBoard::new(Board::from_specs(&[spec(4, 0, 0, true)]).unwrap());
        assert_eq!(gb.fire(0, 0), Some(ShotOutcome::Hit { sunk: false }));
        assert_eq!(gb.fire(1, 0), Some(ShotOutcome::Hit { sunk: true }));
        assert!(gb.board.all_sunk(&gb.shots));
    }

    #[test]
    fn from_specs_roundtrips_through_ships() {
        let specs = vec![
            spec(0, 0, 0, true),
            spec(1, 0, 2, false),
            spec(2, 3, 3, true),
            spec(3, 5, 5, false),
            spec(4, 8, 8, true),
        ];
        let board = Board::from_specs(&specs).unwrap();
        let mut roundtrip = board.ships();
        roundtrip.sort_by_key(|s| s.ship);
        assert_eq!(roundtrip, specs);
    }

    #[test]
    fn random_board_is_always_valid() {
        let mut rng = rand::rng();
        for _ in 0..100 {
            let board = Board::random(&mut rng);
            let specs = board.ships();
            assert_eq!(specs.len(), SHIP_SIZES.len());
            Board::from_specs(&specs).unwrap();
        }
    }
}
