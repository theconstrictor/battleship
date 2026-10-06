mod support;

use std::time::Instant;

use game_core::{Fact, Intent, ShipSpec};
use net_client::{ClientEvent, NetClient};
use support::*;

#[test]
fn full_game_and_rematch() {
    let server = spawn_server();
    let a = connect_client(server.port);
    let b = connect_client(server.port);
    let (a_index, _) = pair_and_place(&a, &b);

    let winner = play_to_end(&a, &b, a_index, random_picker(), hunter_picker());
    let (winner_client, loser_client) = if winner == a_index { (&a, &b) } else { (&b, &a) };
    winner_client.send(Intent::Rematch);
    expect_fact(winner_client, Fact::Rematch);
    expect_fact(loser_client, Fact::Rematch);

    winner_client.send(Intent::ShipsPlaced { ships: random_fleet() });
    loser_client.send(Intent::ShipsPlaced { ships: random_fleet() });
    expect_fact(winner_client, Fact::GameStart);
    expect_fact(loser_client, Fact::GameStart);
    expect_fact(winner_client, Fact::Turn { your_turn: winner == 0 });
    expect_fact(loser_client, Fact::Turn { your_turn: winner != 0 });

    play_to_end(&a, &b, a_index, hunter_picker(), random_picker());
}

#[test]
fn lying_intents_get_errors() {
    let server = spawn_server();
    let good = connect_client(server.port);
    let chaos = connect_client(server.port);
    join(&good, "good");
    join(&chaos, "chaos");
    let good_index = paired_index(&good, "chaos");
    let chaos_index = paired_index(&chaos, "good");
    assert_eq!(good_index ^ 1, chaos_index);

    let mut short = fleet();
    short.pop();
    chaos.send(Intent::ShipsPlaced { ships: short });
    expect_error(&chaos, "Expected 5 ships, got 4");

    let mut overlapping = fleet();
    overlapping[1] = ShipSpec { ship: 1, x: 0, y: 0, horizontal: true };
    chaos.send(Intent::ShipsPlaced { ships: overlapping });
    expect_error(&chaos, "Ships overlap");

    good.send(Intent::ShipsPlaced { ships: fleet() });
    chaos.send(Intent::ShipsPlaced { ships: fleet() });
    expect_fact(&good, Fact::GameStart);
    expect_fact(&chaos, Fact::GameStart);
    expect_fact(&good, Fact::Turn { your_turn: good_index == 0 });
    expect_fact(&chaos, Fact::Turn { your_turn: chaos_index == 0 });

    if chaos_index == 1 {
        chaos.send(Intent::Shot { x: 0, y: 0 });
        expect_error(&chaos, "Not your turn");
        shoot(&good, &chaos, good_index, (5, 5));
    }

    chaos.send(Intent::Shot { x: 10, y: 0 });
    expect_error(&chaos, "Shot out of bounds");
    shoot(&chaos, &good, chaos_index, (0, 0));

    shoot(&good, &chaos, good_index, (9, 9));

    chaos.send(Intent::Shot { x: 0, y: 0 });
    expect_error(&chaos, "Cell already shot");
    shoot(&chaos, &good, chaos_index, (1, 0));

    chaos.send(Intent::Shot { x: 2, y: 0 });
    expect_error(&chaos, "Not your turn");
}

#[test]
fn survivor_sees_disconnect() {
    let server = spawn_server();
    let a = connect_client(server.port);
    let b = connect_client(server.port);
    let (a_index, _) = pair_and_place(&a, &b);
    two_opening_shots(&a, &b, a_index);

    b.disconnect();
    expect_error(&a, "Opponent disconnected");

    let c = connect_client(server.port);
    join(&c, "charlie");
    let c_index = paired_index(&c, "alice");
    let a_index_again = paired_index(&a, "charlie");
    assert_eq!(a_index_again, 0, "the survivor keeps its seat");
    assert_eq!(c_index, 1, "the newcomer takes the free seat");
}

#[test]
fn third_client_rejected() {
    let server = spawn_server();
    let a = connect_client(server.port);
    let b = connect_client(server.port);
    let (a_index, _) = pair_and_place(&a, &b);

    let c = NetClient::connect(([127, 0, 0, 1], server.port).into());
    let remaining = (Instant::now() + TIMEOUT).saturating_duration_since(Instant::now());
    match c.next_event(remaining) {
        Some(ClientEvent::Disconnected { reason }) => {
            assert!(!reason.is_empty(), "the rejection should come with a reason");
        }
        Some(ClientEvent::Connected) => panic!("the server must reject a third client"),
        Some(ClientEvent::Fact(fact)) => panic!("third client received a fact: {fact:?}"),
        None => panic!("third client was never rejected"),
    }

    two_opening_shots(&a, &b, a_index);
}
