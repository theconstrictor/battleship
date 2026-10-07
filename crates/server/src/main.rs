mod game;

use std::{
    collections::HashMap,
    net::UdpSocket,
    thread,
    time::{Duration, Instant, SystemTime},
};

use game::{Game, OutMsg, Target};
use game_core::{Fact, Intent, DEFAULT_PORT, PROTOCOL_ID};
use renet::{ClientId, ConnectionConfig, DefaultChannel, RenetServer, ServerEvent};
use renet_netcode::{NetcodeServerTransport, ServerAuthentication, ServerConfig};
use tracing::{debug, info, warn};
use tracing_subscriber::EnvFilter;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .json()
        .init();

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

    info!(port, "server listening");

    let mut game = Game::default();
    let mut names: HashMap<ClientId, String> = HashMap::new();
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
                        info!(client_id, "client connected");
                    } else {
                        warn!(client_id, "server full, rejecting client");
                        server.disconnect(client_id);
                    }
                }
                ServerEvent::ClientDisconnected { client_id, reason } => {
                    warn!(client_id, name = %names.remove(&client_id).unwrap_or_default(), reason = %reason, "client disconnected");
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
                    warn!(client_id, "unreadable message");
                    continue;
                };
                if let Intent::Join { name } = &intent {
                    names.insert(client_id, name.clone());
                }
                info!(client_id, name = %names.get(&client_id).map(String::as_str).unwrap_or(""), ?intent, "intent received");
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
    match &out.fact {
        Fact::Error { .. } => warn!(target = ?out.target, fact = ?out.fact, "fact sent"),
        Fact::Turn { .. } => debug!(target = ?out.target, fact = ?out.fact, "fact sent"),
        _ => info!(target = ?out.target, fact = ?out.fact, "fact sent"),
    }
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
