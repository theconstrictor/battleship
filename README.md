# Battleship

A networked PvP battleship game, built as a learning exercise around a clean
client-server split:

- **`server`** — a plain Rust binary (no Bevy) that owns the entire game state
- **`game_core`** — the rules and wire protocol, shared by server and clients
- **`client`** — a Bevy client that renders facts and sends intents

## The workspace

```
Cargo.toml            → workspace manifest, shared deps + versions
crates/
  game_core/          → pure rules + wire protocol (no networking, no Bevy)
  server/             → plain Rust binary (no Bevy at all)
  net_client/         → plain-Rust client transport: drives the integration
                        tests under server/tests/ (and future headless bots);
                        the Bevy client uses bevy_renet instead
  client/             → the Bevy game (bevy_renet, not net_client)
```

The root manifest declares the workspace members and centralizes dependency
versions (`serde`, `rand`, `renet`, `bevy`, `bincode`). All crates use
`edition = 2024`.

## `game_core` — the rules, in a vacuum

**`board.rs`** knows nothing about sockets or players. Its whole world:

- `SHIP_SIZES = [5, 4, 3, 3, 2]` and `SHIP_NAMES` — Carrier, Battleship, Cruiser,
  Submarine, Destroyer
- `ShipSpec { ship, x, y, horizontal }` — a serializable description of one
  ship's placement. `cells()` expands it into the coordinates it occupies,
  `in_bounds()` checks the grid
- `Board` — a 10×10 grid of `Option<ship_index>`. `place()` returns `Result`
  with three failure modes: `UnknownShip`, `OutOfBounds`, `Overlap`.
  `from_specs()` validates a whole fleet at once
- `GameBoard` — a `Board` + a shot grid. `fire(x, y)` returns `None` if the
  cell was already shot, otherwise `Miss` or `Hit { sunk }`
- Extras: `Board::random()` for the future client's "randomize" button, and
  `ships()` which reconstructs `ShipSpec`s from a grid (used by tests to
  round-trip)

**`protocol.rs`** is the contract both sides agree on, with the directionality
baked into the types:

```rust
Intent:  Join{name}, ShipsPlaced{ships}, Shot{x,y}, Rematch
         // "requests and claims; the server may reject any of them"

Fact:    Paired{your_index, opponent_name}, GameStart,
         ShotResult{x,y,hit,sunk,game_over,shooter},
         Turn{your_turn}, GameOver{winner}, Rematch, Error{message}
         // "statements about authoritative state; clients cannot reject them"
```

Clients speak `Intent`s; the server answers with `Fact`s. The rename also
exposes a subtlety: `Intent::Rematch` (a request) and `Fact::Rematch` (an
authoritative reset) are different things despite the same word.

## `server` — the authority

**`game.rs` — `Game` is a pure function.** The server's brain has no idea
sockets exist:

```rust
struct Game {
    players: Vec<Player>,  // names + boards, index = player number
    ids: Vec<ClientId>,    // renet connection ids, same order
    phase: Phase,          // Lobby → Placement → Battle → GameOver
    turn: usize,           // whose turn (player index)
}
```

Three entry points, each returning `Vec<OutMsg>` where
`OutMsg = { target: All | One(id), fact: Fact }`:

- `on_connect(id)` — rejects the third connection
- `on_disconnect(id)` — tears down mid-game state, tells the survivor
- `apply(from, intent)` — the big match on `Intent`

`apply` encodes every lie a client could tell, as early returns of
`Fact::Error`:

- `ShipsPlaced` outside placement phase
- wrong ship count, duplicate ship types, overlapping ships
- `Shot` outside battle, out of turn, out of bounds, already-shot cell
- `Rematch` when no game finished

When the second `ShipsPlaced` lands, the phase flips to `Battle` and it emits
`GameStart` + two `Turn` facts. On a legal shot it fires at the opponent's
board, broadcasts the `ShotResult` (with `shooter` so each client knows whose
shot it was), then either ends the game or flips `turn` and re-sends `Turn` to
each player.

**`main.rs` — the thin shell.** A ~70-line loop that moves bytes:

```rust
loop {
    transport.update(delta, &mut server);  // UDP packets in, handshake events
    server.update(delta);                  // reliable channel bookkeeping

    // drain connect/disconnect events → game.on_connect / on_disconnect
    // drain bytes per client → bincode::deserialize::<Intent> → game.apply → dispatch

    transport.send_packets(&mut server);   // bytes out
    sleep(2ms);
}
```

`dispatch()` serializes each `Fact` and sends it to `All` or `One`. The server
writes a JSON log (one line per event: connects, intents received, facts
sent, rejections) to stdout — redirect it for later analysis, e.g.
`cargo run -p server > server.log`, and use `RUST_LOG=debug` for turn facts.

## `client` — facts in, intents out

The Bevy client renders a projection of facts and sends intents; it never
moves its own Screen. A single `Screen` enum (`Menu`, `Waiting`, `Placement`,
`Battle`, `GameOver`) is the bevy `States`; transitions happen only when
facts or connection events arrive.

- **`projection.rs`** is the pure client brain: a `Projection` resource that
  `apply_fact(&Fact) -> ScreenAction` mutates, plus the placement
  `FleetEditor` (palette, carry, rotate, the overlap/bounds convenience
  check). Both are unit-tested without sockets.
- **`net.rs`** holds `RenetClient` + `NetcodeClientTransport` resources
  (bevy_renet), drains wire bytes into `apply_facts`, watches
  connect/disconnect, and lands every disconnection back in Menu with a
  reason and a Retry button.
- **Screens** build their `bevy_ui` nodes in `OnEnter`/`OnExit`. Placement
  has the palette, R to rotate, a green/red hover ghost, Randomize
  (`Board::random`), and Ready gated on a preview-valid fleet. Battle has
  200 cell entities mutated by one render system: `ShotResult` is keyed by
  `shooter` against my seat, the enemy board dims on the opponent's turn but
  stays clickable — shots are never pre-checked locally. GameOver offers
  Rematch to both seats; the loser of the race gets
  `Error: No finished game to rematch`, which hides the button.

All rules stay server-side; the Authority re-validates every intent,
including fleets that passed the convenience check.

## Why it's structured this way

1. **Authority is testable without sockets** — `game.rs` tests construct a
   `Game`, feed it `Intent`s as if they came off the wire, and assert on the
   outgoing `Fact`s. One test plays a complete game shot-by-shot to
   `GameOver`; others verify each rejection path.
2. **Player index vs connection id** — game logic never uses renet's
   `ClientId` for identity; it uses join order (`0`/`1`). That's what
   `shooter` and `winner` carry over the wire, so a client only needs "am I
   0 or 1" — learned from `Fact::Paired{your_index}`.
3. **The wire format can't drift** — both sides use the same `Intent`/`Fact`
   types from `game_core`.
4. **Battleship trivia** — `sunk` is computed by `GameBoard::fire` locally
   (the shot that hits the ship's last cell), and `game_over` additionally
   checks `all_sunk` — sinking one ship doesn't end the game.

## Tests — 38 total, all green

- **8 in `game_core`** — placement/lookup, out-of-bounds, overlap, unknown
  ship, hit/miss/repeat, sinking on last cell, `ships()` round-trip, 100
  random boards always valid
- **12 in `server`** — third-client rejection, pairing, lone-join waits,
  game start, out-of-turn/out-of-bounds/already-shot rejection, full game to
  `GameOver`, invalid fleets, rematch reset, mid-game disconnect recovery
- **4 integration tests** (`crates/server/tests/`) — spawn the real server
  binary and drive `net_client` actors over real UDP sockets: full game +
  rematch, lying intents earning `Fact::Error`s, survivor sees a mid-battle
  disconnect, third client rejected
- **14 in `client`** — the pure projection brain: fact routing by seat,
  screen actions per fact, rematch reset, rematch-race error handling, and
  the fleet editor's pick-up/place/rotate/overlap/ready/randomize logic

## Run it

```bash
cargo test                    # 38 tests
cargo run -p server           # "battleship server listening on 0.0.0.0:5000"
cargo run -p server -- --port 6000
cargo run -p client           # defaults: name "player", server 127.0.0.1:5000
cargo run -p client -- --name alice --server 127.0.0.1:6000
cargo run -p client -- --server myhost.example.com:6000   # DNS name works too
```

## Run prebuilt binaries

**Releases**: [semantic-release](https://semantic-release.gitbook.io) runs on
`master` and derives versions from conventional commit messages (`feat:` →
minor, `fix:` → patch, `feat!`/`BREAKING CHANGE` → major). On release it
bumps the workspace version in `Cargo.toml` + `Cargo.lock`, tags the repo,
creates a GitHub release, and the `attach` job in the same workflow run
builds and uploads `server` and `client` binaries for all four architectures
as release assets.

The release job pushes the version bump and tag using a personal access
token stored as the repo secret `RELEASE_TOKEN` (a classic PAT with `repo`
scope); the `attach` job reuses it to upload the binaries to the release.

Every other CI run also uploads binaries as workflow artifacts (Actions →
pick a run → Artifacts): `server-<arch>` and `client-<arch>` for linux
x86_64/aarch64, macos aarch64, and windows x86_64. Download the pair
matching your machine (on Windows the binaries are `server.exe` /
`client.exe`) and run them directly, no Rust toolchain needed:

```bash
./server                 # listens on 0.0.0.0:5000, JSON log to stdout
./server --port 6000
./server > server.log    # redirect the JSON log for later analysis

./client                                 # defaults: name "player", server 127.0.0.1:5000
./client --name alice --server 127.0.0.1:6000
./client --server myhost.example.com:6000   # DNS name works too
```

One deliberate simplification to flag: **player 0 always goes first**,
including after rematches. Real battleship alternates the starting player.
