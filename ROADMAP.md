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
  transport, used by the integration tests to drive client actors (and
  reusable later by the headless AI opponent). The Bevy client uses
  `bevy_renet` instead:
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

**Status: done**

`crates/client` on `bevy 0.19` + `bevy_renet 5.0` (same `renet 2.0` stack as
the server; `net_client` is not used here — it stays the test harness):

- Launch via CLI args with defaults: `--name` (default `player`),
  `--server` (default `127.0.0.1:5000`); auto-connect on startup. No text
  field yet — that moves to step 5 polish
- A single `Screen` enum (`Menu`, `Waiting`, `Placement`, `Battle`,
  `GameOver`) as bevy `States`. Transitions happen only when Facts or
  connection events arrive, never when the client sends an Intent
- Menu: "connecting..." status; on failure the reason shows with a Retry
  button (no hidden auto-retry). Every disconnection (opponent left, server
  died, own quit) lands back in Menu with the reason shown; in-game screens
  have a quit button. The server now also notifies survivors whose opponent
  leaves before pairing — otherwise Waiting would hang forever
- Placement: palette of the 5 ships + click-to-pick-up (palette ship or a
  placed ship), R rotates the carried ship, green/red hover preview
  (overlap/out-of-bounds refused locally — the convenience check), Randomize
  fills via `Board::random`, Ready enabled only while the fleet is
  preview-valid. The server still re-validates on receive
- Battle: 200 cell entities (own board + enemy board, each with position,
  side, and damage components) mutated by one `apply_facts` system.
  `ShotResult` is keyed by `shooter` against my seat: my shots update the
  enemy board, the opponent's update my own damage overlay. The enemy board
  dims during the opponent's turn but stays clickable — a wrong click sends
  and earns the server's error (no local veto)
- Everything renders as `bevy_ui` nodes (zero assets); screens build and
  tear down their entities in `OnEnter`/`OnExit` hooks, so a Rematch simply
  re-enters Placement with fresh cells
- A single status line under the boards, written by `apply_facts`:
  `Turn` → "Your turn" / "Opponent's turn", `ShotResult` → hit/miss/sunk,
  `Error` → the message verbatim
- GameOver: both seats get a Rematch button; `Fact::Rematch` (broadcast)
  transitions both back to Placement, and a loser-of-the-race
  `Error: No finished game to rematch` just hides the button
- All rules stay server-side; the client never pre-checks a shot locally

## 5. What comes after

Roughly in order of value, all optional:

- **Alternate starting player** — currently player 0 always starts; flip a
  flag on each rematch
- **Connection timeouts / AFK handling** — server kicks a client that
  connects but never sends `Join`, or never places ships. Client side: a
  connect timeout so a dead server stops showing "connecting..." forever
  (netcode has no client-side connect timeout)
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
  placement UX (drag-and-drop), a real name/server text field in the menu
- **Observability** — structured logging on the server, and a debug overlay
  in the client showing wire traffic
- **Replays** — server records the intent/fact stream to a file; a viewer
  replays it (essentially free once the protocol is stable)
- **CI** — `cargo test` runs on push/PR today; add `cargo clippy` and keep
  the integration suite as part of it
