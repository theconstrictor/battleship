mod game;

use std::{
    net::UdpSocket,
    thread,
    time::{Duration, Instant, SystemTime},
};

use game::{Game, OutMsg, Target};
use game_core::{Intent, DEFAULT_PORT, PROTOCOL_ID};
use renet::{ConnectionConfig, DefaultChannel, RenetServer, ServerEvent};
use renet_netcode::{NetcodeServerTransport, ServerAuthentication, ServerConfig};

fn main() {
    let port = port_from_args();
    let socket = UdpSocket::bind(("0.0.0.0", port)).expect("failed to bind port");
    let server_config = ServerConfig {
        current_time: SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap(),
        max_clients: 2,
        protocol_id: PROTOCOL_ID,
        public_addresses: vec![socket.local_addr().unwrap()],
        authentication: ServerAuthentication::Unsecure,
    };
    let mut server = RenetServer::new(ConnectionConfig::default());
    let mut transport = NetcodeServerTransport::new(server_config, socket).unwrap();

    println!("battleship server listening on 0.0.0.0:{port}");

    let mut game = Game::default();
    let mut last_tick = Instant::now();

    loop {
        let now = Instant::now();
        let delta = now - last_tick;
        last_tick = now;

        transport.update(delta, &mut server).unwrap();
        server.update(delta);

        while let Some(event) = server.get_event() {
            match event {
                ServerEvent::ClientConnected { client_id } => {
                    if game.on_connect(client_id) {
                        println!("client {client_id} connected");
                    } else {
                        println!("server full, rejecting client {client_id}");
                        server.disconnect(client_id);
                    }
                }
                ServerEvent::ClientDisconnected { client_id, reason } => {
                    println!("client {client_id} disconnected: {reason}");
                    for out in game.on_disconnect(client_id) {
                        dispatch(&mut server, out);
                    }
                }
            }
        }

        let client_ids: Vec<_> = server.clients_id_iter().collect();
        for client_id in client_ids {
            while let Some(bytes) = server.receive_message(client_id, DefaultChannel::ReliableOrdered) {
                let Ok(intent) = bincode::deserialize::<Intent>(&bytes) else {
                    println!("client {client_id} sent an unreadable message");
                    continue;
                };
                for out in game.apply(client_id, intent) {
                    dispatch(&mut server, out);
                }
            }
        }

        transport.send_packets(&mut server);

        thread::sleep(Duration::from_millis(2));
    }
}

fn dispatch(server: &mut RenetServer, out: OutMsg) {
    let bytes = bincode::serialize(&out.fact).expect("message serialization failed");
    match out.target {
        Target::All => server.broadcast_message(DefaultChannel::ReliableOrdered, bytes),
        Target::One(id) => server.send_message(id, DefaultChannel::ReliableOrdered, bytes),
    }
}

fn port_from_args() -> u16 {
    let mut args = std::env::args().skip(1);
    match args.next() {
        None => DEFAULT_PORT,
        Some(arg) if arg == "--port" => {
            let value = args.next().expect("--port requires a value");
            value.parse().expect("port must be a number")
        }
        Some(other) => {
            eprintln!("unknown argument: {other}");
            std::process::exit(1);
        }
    }
}
