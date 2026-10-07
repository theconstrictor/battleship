# Battleship

A networked PvP battleship game with a strict authority split: one server
owns the rules, clients render facts and send intents.

## Language

**Authority**:
The server. The single owner of all rules and game state.
_Avoid_: Server-side logic, game master

**Intent**:
A request or claim a client sends to the Authority. The Authority may accept
or reject any of them (`Join`, `ShipsPlaced`, `Shot`, `Rematch`).
_Avoid_: Command, message

**Fact**:
An authoritative statement about game state, sent by the Authority to
clients. Clients render them and cannot reject them (`Paired`, `GameStart`,
`ShotResult`, `Turn`, `GameOver`, `Rematch`, `Error`).
_Avoid_: Event, notification

**Phase**:
The Authority's state machine: `Lobby → Placement → Battle → GameOver`.
Owned by the server; clients never hold a phase of their own.
_Avoid_: Stage, round

**Screen**:
A client's fact-driven projection of what to render (`Menu`, `Waiting`,
`Placement`, `Battle`, `GameOver`). Transitions only in response to Facts or
connection events — never in response to the client's own Intents.
_Avoid_: State, phase (see Phase)

**Projection**:
Client-side rendering state derived from Facts, never from the client's own
actions.

**Convenience check**:
A client-side preview of a rule the Authority will decide anyway, allowed
only where a rejected submission is expensive to redo (fleet placement:
bounds/overlap preview, Ready disabled until valid). Shots get no
convenience check — a rejected shot is one cheap `Fact::Error`. The
Authority still re-validates everything on receive; convenience checks
never veto an Intent.

**Seat**:
A player's index (0 or 1), assigned by the Authority and learned from
`Fact::Paired`. Used to interpret `shooter`/`winner`/`your_turn` fields.
_Avoid_: Player id, slot

## Example dialogue

Dev: "When the player clicks Ready, should the client flip to Battle?"
Expert: "No. The client sends `ShipsPlaced`; when `GameStart` arrives, the
Screen becomes Battle. The client never moves its own Screen."

Dev: "Can the placement screen gray out overlapping cells?"
Expert: "That's a Projection question. Whatever it renders, the Authority
re-validates the fleet on receive."
