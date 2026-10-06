use std::{
    net::UdpSocket,
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};

use game_core::{Board, Fact, Intent, ShipSpec};
use net_client::{ClientEvent, NetClient};
use rand::Rng;

pub const TIMEOUT: Duration = Duration::from_secs(15);
const TICK: Duration = Duration::from_millis(1);

pub struct ServerGuard {
    child: Child,
    pub port: u16,
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Spawn the real server binary on an ephemeral port.
pub fn spawn_server() -> ServerGuard {
    let socket = UdpSocket::bind(("127.0.0.1", 0)).expect("bind ephemeral udp port");
    let port = socket.local_addr().unwrap().port();
    drop(socket);
    let child = Command::new(env!("CARGO_BIN_EXE_server"))
        .arg("--port")
        .arg(port.to_string())
        .spawn()
        .expect("failed to spawn server binary");
    ServerGuard { child, port }
}

/// Connect and wait for the netcode handshake to complete.
pub fn connect_client(port: u16) -> NetClient {
    let client = NetClient::connect(([127, 0, 0, 1], port).into());
    match next_event(&client, TIMEOUT) {
        ClientEvent::Connected => client,
        other => panic!("client failed to connect, got {other:?}"),
    }
}

pub fn next_event(client: &NetClient, timeout: Duration) -> ClientEvent {
    client
        .next_event(timeout)
        .unwrap_or_else(|| panic!("no event within {timeout:?}"))
}

pub fn next_fact(client: &NetClient, timeout: Duration) -> Fact {
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match next_event(client, remaining) {
            ClientEvent::Fact(fact) => return fact,
            ClientEvent::Connected => {}
            ClientEvent::Disconnected { reason } => panic!("client disconnected: {reason}"),
        }
    }
}

pub fn expect_fact(client: &NetClient, expected: Fact) {
    let got = next_fact(client, TIMEOUT);
    assert_eq!(got, expected);
}

/// Wait for an error, tolerating benign facts that may arrive first
/// (turn flips and broadcast shot results racing the error).
pub fn expect_error(client: &NetClient, message: &str) {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match next_fact(client, remaining) {
            Fact::Error { message: got } => {
                assert_eq!(got, message);
                return;
            }
            Fact::Turn { .. } | Fact::ShotResult { .. } | Fact::GameStart | Fact::Rematch => {}
            other => panic!("unexpected fact while waiting for an error: {other:?}"),
        }
    }
}

pub fn join(client: &NetClient, name: &str) {
    client.send(Intent::Join { name: name.to_string() });
}

pub fn paired_index(client: &NetClient, opponent_name: &str) -> u8 {
    match next_fact(client, TIMEOUT) {
        Fact::Paired {
            your_index,
            opponent_name: got,
        } => {
            assert_eq!(got, opponent_name);
            your_index
        }
        other => panic!("expected Paired, got {other:?}"),
    }
}

/// The canonical test fleet: five horizontal ships along rows 0, 2, 4, 6, 8.
pub fn fleet() -> Vec<ShipSpec> {
    vec![
        ShipSpec { ship: 0, x: 0, y: 0, horizontal: true },
        ShipSpec { ship: 1, x: 0, y: 2, horizontal: true },
        ShipSpec { ship: 2, x: 0, y: 4, horizontal: true },
        ShipSpec { ship: 3, x: 0, y: 6, horizontal: true },
        ShipSpec { ship: 4, x: 0, y: 8, horizontal: true },
    ]
}

pub fn random_fleet() -> Vec<ShipSpec> {
    Board::random(&mut rand::rng()).ships()
}

/// Join, pair, place, and reach Battle. Returns each client's player index.
pub fn pair_and_place(a: &NetClient, b: &NetClient) -> (u8, u8) {
    join(a, "alice");
    join(b, "bob");
    let a_index = paired_index(a, "bob");
    let b_index = paired_index(b, "alice");
    assert_eq!(a_index ^ 1, b_index, "players must get distinct indices");
    a.send(Intent::ShipsPlaced { ships: fleet() });
    b.send(Intent::ShipsPlaced { ships: fleet() });
    expect_fact(a, Fact::GameStart);
    expect_fact(b, Fact::GameStart);
    expect_fact(a, Fact::Turn { your_turn: a_index == 0 });
    expect_fact(b, Fact::Turn { your_turn: b_index == 0 });
    (a_index, b_index)
}

/// One legal shot, asserted on both clients (the shot is broadcast).
pub fn shoot(shooter: &NetClient, other: &NetClient, shooter_index: u8, target: (u8, u8)) {
    shooter.send(Intent::Shot { x: target.0, y: target.1 });
    expect_shot_result(shooter, shooter_index, target);
    expect_shot_result(other, shooter_index, target);
}

fn expect_shot_result(client: &NetClient, shooter_index: u8, target: (u8, u8)) {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match next_fact(client, remaining) {
            Fact::ShotResult { x, y, hit, sunk, game_over, shooter } => {
                assert_eq!(shooter, shooter_index);
                assert_eq!((x, y), target);
                assert!(!game_over, "the game unexpectedly ended");
                let _ = (hit, sunk);
                return;
            }
            Fact::Turn { .. } => {}
            other => panic!("expected ShotResult, got {other:?}"),
        }
    }
}

/// Player 0 shoots (0, 0), player 1 shoots (9, 9): two legal, non-fatal
/// shots that leave the game firmly mid-battle.
pub fn two_opening_shots(a: &NetClient, b: &NetClient, a_index: u8) {
    if a_index == 0 {
        shoot(a, b, 0, (0, 0));
        shoot(b, a, 1, (9, 9));
    } else {
        shoot(b, a, 0, (0, 0));
        shoot(a, b, 1, (9, 9));
    }
}

#[derive(Default)]
pub struct ShotTracker {
    shots: Vec<(u8, u8)>,
    targets: Vec<(u8, u8)>,
}

impl ShotTracker {
    pub fn is_new(&self, x: u8, y: u8) -> bool {
        !self.shots.contains(&(x, y))
    }

    pub fn note_result(&mut self, x: u8, y: u8, hit: bool, sunk: bool) {
        self.shots.push((x, y));
        if hit && !sunk {
            for (dx, dy) in [(1i16, 0i16), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x as i16 + dx, y as i16 + dy);
                if (0..10).contains(&nx) && (0..10).contains(&ny) {
                    let target = (nx as u8, ny as u8);
                    if self.is_new(nx as u8, ny as u8) {
                        self.targets.push(target);
                    }
                }
            }
        }
    }
}

pub fn random_picker() -> impl FnMut(&mut ShotTracker) -> (u8, u8) {
    let mut rng = rand::rng();
    move |tracker| loop {
        let (x, y) = (rng.random_range(0..10), rng.random_range(0..10));
        if tracker.is_new(x, y) {
            return (x, y);
        }
    }
}

/// Parity (checkerboard) search; on a non-fatal hit, shoots the neighbors
/// next; falls back to a full scan if parity is exhausted.
pub fn hunter_picker() -> impl FnMut(&mut ShotTracker) -> (u8, u8) {
    move |tracker| {
        while let Some((x, y)) = tracker.targets.pop() {
            if tracker.is_new(x, y) {
                return (x, y);
            }
        }
        let parity = (0..10u8)
            .flat_map(|y| (0..10u8).map(move |x| (x, y)))
            .filter(|(x, y)| (x + y) % 2 == 0);
        for (x, y) in parity {
            if tracker.is_new(x, y) {
                return (x, y);
            }
        }
        for y in 0..10u8 {
            for x in 0..10u8 {
                if tracker.is_new(x, y) {
                    return (x, y);
                }
            }
        }
        panic!("no unshot cells left");
    }
}

/// Drive both clients until they have each seen GameOver.
/// Returns the winner's index; panics on any protocol surprise.
pub fn play_to_end(
    a: &NetClient,
    b: &NetClient,
    a_index: u8,
    mut pick_a: impl FnMut(&mut ShotTracker) -> (u8, u8),
    mut pick_b: impl FnMut(&mut ShotTracker) -> (u8, u8),
) -> u8 {
    let b_index = a_index ^ 1;
    let mut tracker_a = ShotTracker::default();
    let mut tracker_b = ShotTracker::default();
    let mut armed_a = a_index == 0;
    let mut armed_b = b_index == 0;
    let mut a_over = false;
    let mut b_over = false;
    let mut winner: Option<u8> = None;
    let mut last_shooter_a: Option<u8> = None;
    let mut last_shooter_b: Option<u8> = None;
    let deadline = Instant::now() + TIMEOUT * 4;
    loop {
        assert!(Instant::now() < deadline, "game did not finish in time");

        while let Some(event) = a.try_next_event() {
            match event {
                ClientEvent::Fact(Fact::Turn { your_turn }) => armed_a = your_turn,
                ClientEvent::Fact(Fact::ShotResult { x, y, hit, sunk, game_over, shooter }) => {
                    assert!(last_shooter_a != Some(shooter), "same player shot twice in a row");
                    last_shooter_a = Some(shooter);
                    if shooter == a_index {
                        tracker_a.note_result(x, y, hit, sunk);
                    }
                    if game_over {
                        a_over = true;
                    }
                }
                ClientEvent::Fact(Fact::GameOver { winner: w }) => {
                    match winner {
                        Some(prev) => assert_eq!(prev, w, "the two clients disagree on the winner"),
                        None => winner = Some(w),
                    }
                    a_over = true;
                }
                ClientEvent::Fact(Fact::Error { message }) => panic!("unexpected error on a: {message}"),
                ClientEvent::Disconnected { reason } => panic!("a disconnected: {reason}"),
                ClientEvent::Connected => {}
                other => panic!("unexpected event on a: {other:?}"),
            }
        }

        while let Some(event) = b.try_next_event() {
            match event {
                ClientEvent::Fact(Fact::Turn { your_turn }) => armed_b = your_turn,
                ClientEvent::Fact(Fact::ShotResult { x, y, hit, sunk, game_over, shooter }) => {
                    assert!(last_shooter_b != Some(shooter), "same player shot twice in a row");
                    last_shooter_b = Some(shooter);
                    if shooter == b_index {
                        tracker_b.note_result(x, y, hit, sunk);
                    }
                    if game_over {
                        b_over = true;
                    }
                }
                ClientEvent::Fact(Fact::GameOver { winner: w }) => {
                    match winner {
                        Some(prev) => assert_eq!(prev, w, "the two clients disagree on the winner"),
                        None => winner = Some(w),
                    }
                    b_over = true;
                }
                ClientEvent::Fact(Fact::Error { message }) => panic!("unexpected error on b: {message}"),
                ClientEvent::Disconnected { reason } => panic!("b disconnected: {reason}"),
                ClientEvent::Connected => {}
                other => panic!("unexpected event on b: {other:?}"),
            }
        }

        if armed_a {
            let (x, y) = pick_a(&mut tracker_a);
            assert!(tracker_a.is_new(x, y), "picker offered an already-shot cell");
            a.send(Intent::Shot { x, y });
            armed_a = false;
        }
        if armed_b {
            let (x, y) = pick_b(&mut tracker_b);
            assert!(tracker_b.is_new(x, y), "picker offered an already-shot cell");
            b.send(Intent::Shot { x, y });
            armed_b = false;
        }

        if a_over && b_over {
            break;
        }
        thread::sleep(TICK);
    }
    winner.expect("game ended without a winner")
}
