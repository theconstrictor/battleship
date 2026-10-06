use std::{
    collections::VecDeque,
    net::{SocketAddr, UdpSocket},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant, SystemTime},
};

use game_core::{Fact, Intent, PROTOCOL_ID};
use renet::{ConnectionConfig, DefaultChannel, RenetClient};
use renet_netcode::{ClientAuthentication, NetcodeClientTransport};

/// What the client observed on the wire, in order.
#[derive(Debug, PartialEq)]
pub enum ClientEvent {
    Connected,
    Disconnected { reason: String },
    Fact(Fact),
}

enum Command {
    Send(Intent),
    Disconnect,
}

/// A client connection, pumped by a background thread.
pub struct NetClient {
    cmd_tx: Sender<Command>,
    event_rx: Receiver<ClientEvent>,
}

impl NetClient {
    pub fn connect(server_addr: SocketAddr) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        thread::spawn(move || pump(server_addr, cmd_rx, event_tx));
        Self { cmd_tx, event_rx }
    }

    /// Queue an intent. If the handshake has not finished yet, the intent is
    /// buffered and flushed once connected.
    pub fn send(&self, intent: Intent) {
        self.cmd_tx.send(Command::Send(intent)).expect("client pump died");
    }

    pub fn try_next_event(&self) -> Option<ClientEvent> {
        self.event_rx.try_recv().ok()
    }

    pub fn next_event(&self, timeout: Duration) -> Option<ClientEvent> {
        self.event_rx.recv_timeout(timeout).ok()
    }

    /// Gracefully disconnect: the pump keeps running until the disconnect
    /// packet has actually reached the server.
    pub fn disconnect(&self) {
        let _ = self.cmd_tx.send(Command::Disconnect);
    }
}

impl Drop for NetClient {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(Command::Disconnect);
    }
}

/// The background thread's main loop: runs until the connection ends.
///
/// Every tick it drains queued commands (buffering `Send` intents until the
/// handshake completes), pumps the netcode transport and renet channels,
/// announces `Connected` once the handshake finishes, forwards deserialized
/// `Fact`s to the event channel, and finally reports any disconnection with
/// its reason. The transport is pumped before the disconnect check, so a
/// graceful `disconnect()` still flushes its goodbye packet to the server.
fn pump(server_addr: SocketAddr, cmd_rx: Receiver<Command>, event_tx: Sender<ClientEvent>) {
    let client_id = loop {
        let id = rand::random::<u64>();
        if id != 0 {
            break id;
        }
    };
    let Ok(socket) = UdpSocket::bind(("0.0.0.0", 0)) else {
        return;
    };
    let current_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
    let authentication = ClientAuthentication::Unsecure {
        protocol_id: PROTOCOL_ID,
        client_id,
        server_addr,
        user_data: None,
    };
    let Ok(mut transport) = NetcodeClientTransport::new(current_time, authentication, socket) else {
        return;
    };
    let mut client = RenetClient::new(ConnectionConfig::default());
    let mut pending: VecDeque<Intent> = VecDeque::new();
    let mut announced = false;
    let mut last_tick = Instant::now();

    loop {
        let delta = last_tick.elapsed();
        last_tick = Instant::now();

        while let Ok(command) = cmd_rx.try_recv() {
            match command {
                Command::Send(intent) => {
                    if client.is_connected() {
                        flush(&mut client, intent);
                    } else {
                        pending.push_back(intent);
                    }
                }
                Command::Disconnect => client.disconnect(),
            }
        }

        let _ = transport.update(delta, &mut client);
        client.update(delta);
        let _ = transport.send_packets(&mut client);

        if !announced && client.is_connected() {
            announced = true;
            for intent in pending.drain(..) {
                flush(&mut client, intent);
            }
            if event_tx.send(ClientEvent::Connected).is_err() {
                return;
            }
        }

        if let Some(reason) = client.disconnect_reason() {
            let text = transport
                .disconnect_reason()
                .map(|reason| reason.to_string())
                .unwrap_or_else(|| reason.to_string());
            let _ = event_tx.send(ClientEvent::Disconnected { reason: text });
            return;
        }

        while let Some(bytes) = client.receive_message(DefaultChannel::ReliableOrdered) {
            if let Ok(fact) = bincode::deserialize::<Fact>(&bytes)
                && event_tx.send(ClientEvent::Fact(fact)).is_err()
            {
                return;
            }
        }

        thread::sleep(Duration::from_millis(1));
    }
}

/// Serialize one intent to bincode and hand it to renet's reliable channel.
///
/// This is the single point where an `Intent` becomes wire bytes; the pump
/// uses it for live sends and when draining intents queued during the
/// handshake.
fn flush(client: &mut RenetClient, intent: Intent) {
    let bytes = bincode::serialize(&intent).expect("intent serialization failed");
    client.send_message(DefaultChannel::ReliableOrdered, bytes);
}
