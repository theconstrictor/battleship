# Roadmap

Build order, current status, and open ideas. The guiding principle throughout:
the server owns the rules, clients render facts and send intents, and nothing
re-validates a rule the authority already owns.

## 1. `game_core` — rules + protocol

**Status: done**

- 10×10 `Board`, ship placement with overlap/bounds validation
- `GameBoard::fire` → Miss / Hit{sunk}
- `Intent` / `Fact` protocol enums (serde + bincode)
- Shared constants: `PROTOCOL_ID`, `DEFAULT_PORT`
- 8 unit tests

## 2. `server` — the authority

**Status: done**

- Plain Rust, renet + netcode (UDP), no Bevy
- `Game` as a pure function: `apply(from, intent) -> Vec<OutMsg>`
- Pairing first two clients, phase machine (Lobby → Placement → Battle → GameOver)
- Every rejection path: out-of-turn, out-of-bounds, already-shot, invalid fleets
- Disconnect handling with game reset, rematch support
- 10 unit tests

## 3. `net_client` + integration tests — real sockets

**Status: done**

A test harness, not a product: a client networking layer plus an
integration suite that exercises the server end-to-end without a GUI.

- `crates/net_client` — a small library wrapping `RenetClient` + netcode
  transport (same stack the Bevy client will use, so this doubles as the
  reference implementation of the client networking layer):
  `connect`, `send(intent)`, `poll_facts`, `disconnect`
- `crates/server/tests/integration.rs` — scenarios as plain `#[test]`s that
  spawn the real server binary on an ephemeral port (server supports
  `--port`) and drive client actors in-process via threads, asserting on
  the fact stream with native asserts — the test is the spec, no CLI flags
  or exit-code contract needed
- Test-support actors in `tests/support/`: `Random` (random fleet + shots),
  `Hunter` (parity search, then finish hits), `Chaos` (lies: out-of-turn,
  out-of-bounds, duplicate shots, bad fleets; expects the matching
  `Fact::Error` after each), `Quitter` (drops mid-battle)
- Scenarios:
  - `full_game_and_rematch` — two actors play to `GameOver`, then rematch
  - `lying_intents_get_errors` — chaos actor, every lie earns an error
  - `survivor_sees_disconnect` — quitter drops mid-battle; survivor asserts
    the disconnect error and lobby reset
  - `third_client_rejected` — a third connection is refused

This closes the gap between the socket-free unit tests and real-world
behavior (handshake timing, netcode connect/disconnect events, message
ordering) before any UI exists.

## 4. Bevy client

**Status: planned**

`crates/client` using `bevy_renet`:

- Main menu: name + server IP entry (homemade text field — bevy core has no
  text input widget), connect button
- Placement screen: click to place, R to rotate, randomize, ready (disabled
  until the fleet is valid — but the server still re-validates on receive)
- Battle screen: own board + enemy board, status line driven by `Fact::Turn`,
  `ShotResult` updates cell colors, `GameOver` screen with rematch
- Client state machine mirrors the server phases (Menu → Lobby → Placement →
  Battle → GameOver) via bevy `States`
- Client-side convenience only: hover highlight, valid-placement preview
- All rules stay server-side; the client never rejects its own shot locally

## 5. What comes after

Roughly in order of value, all optional:

- **Alternate starting player** — currently player 0 always starts; flip a
  flag on each rematch
- **Connection timeouts / AFK handling** — server kicks a client that
  connects but never sends `Join`, or never places ships
- **Reconnect support** — token in the connect handshake so a dropped client
  can rejoin its own game instead of killing the match
- **Matchmaking** — room codes and a queue instead of "first two in"; the
  server becomes multi-match (`HashMap<RoomId, Game>`), which is a great
  refactor to learn the `Game` boundary
- **Secure netcode** — `ServerAuthentication::Secure` with connect tokens
  instead of `Unsecure`
- **AI opponent** — a local CPU battleship player speaking the same
  `Intent`/`Fact` protocol against the server; reuse of `net_client` from
  step 3
- **Client polish** — sounds, hit/sink animations, board history, ship
  placement UX (drag-and-drop)
- **Observability** — structured logging on the server, and a debug overlay
  in the client showing wire traffic
- **Replays** — server records the intent/fact stream to a file; a viewer
  replays it (essentially free once the protocol is stable)
- **CI** — `cargo test` + `cargo clippy` on push, plus the integration
  suite as a job
