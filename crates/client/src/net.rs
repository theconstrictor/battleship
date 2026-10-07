use std::{
    net::{SocketAddr, UdpSocket},
    time::SystemTime,
};

use bevy::prelude::*;
use bevy_renet::{
    RenetClient,
    netcode::{ClientAuthentication, NetcodeClientTransport},
    renet::{ConnectionConfig, DefaultChannel},
};
use game_core::{Fact, Intent, PROTOCOL_ID};

use crate::{
    projection::{FleetEditor, Projection, ScreenAction},
    screen::Screen,
    ui::{QuitButton, StatusLine},
};

#[derive(Resource, Debug)]
pub struct NetConfig {
    pub name: String,
    pub server: SocketAddr,
}

#[derive(Resource, Debug, Default)]
pub enum MenuStatus {
    #[default]
    Connecting,
    Failed(String),
}

/// Why the client is about to disconnect itself; shown on the Menu once the
/// disconnect lands, instead of the wire reason.
#[derive(Resource, Debug, Default)]
pub struct LocalDisconnect {
    pub message: Option<String>,
}

pub fn build_client(server: SocketAddr) -> (RenetClient, NetcodeClientTransport) {
    let client_id = loop {
        let id = rand::random::<u64>();
        if id != 0 {
            break id;
        }
    };
    let socket = UdpSocket::bind(("0.0.0.0", 0)).expect("failed to bind client socket");
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system time before unix epoch");
    let authentication = ClientAuthentication::Unsecure {
        protocol_id: PROTOCOL_ID,
        client_id,
        server_addr: server,
        user_data: None,
    };
    let transport = NetcodeClientTransport::new(current_time, authentication, socket)
        .expect("failed to build netcode transport");
    let client = RenetClient::new(ConnectionConfig::default());
    (client, transport)
}

pub fn setup(mut commands: Commands, net: Res<NetConfig>) {
    commands.spawn((Camera2d, IsDefaultUiCamera));
    let (client, transport) = build_client(net.server);
    commands.insert_resource(client);
    commands.insert_resource(transport);
}

/// Drain wire bytes into `Fact`s and feed them to the projection. The single
/// place where a Fact can move the Screen or mutate what is rendered.
pub fn apply_facts(
    mut client: ResMut<RenetClient>,
    mut projection: ResMut<Projection>,
    mut next_screen: ResMut<NextState<Screen>>,
    mut local: ResMut<LocalDisconnect>,
) {
    while let Some(bytes) = client.receive_message(DefaultChannel::ReliableOrdered) {
        let Ok(fact) = bincode::deserialize::<Fact>(&bytes) else {
            continue;
        };
        match projection.apply_fact(&fact) {
            ScreenAction::None => {}
            ScreenAction::Enter(screen) => next_screen.set(screen),
            ScreenAction::OpponentLeft => {
                local.message = Some("Opponent left the game".into());
                client.disconnect();
            }
        }
    }
}

/// The netcode handshake finished: announce ourselves and wait for a pair.
pub fn on_connected(mut client: ResMut<RenetClient>, net: Res<NetConfig>, mut next_screen: ResMut<NextState<Screen>>) {
    send_intent(&mut client, Intent::Join { name: net.name.clone() });
    next_screen.set(Screen::Waiting);
}

/// The connection ended, for any reason: land in the Menu with the reason.
pub fn on_disconnected(
    client: Option<Res<RenetClient>>,
    transport: Option<Res<NetcodeClientTransport>>,
    mut commands: Commands,
    mut menu: ResMut<MenuStatus>,
    mut local: ResMut<LocalDisconnect>,
    mut next_screen: ResMut<NextState<Screen>>,
) {
    let wire = transport
        .as_ref()
        .and_then(|t| t.disconnect_reason())
        .map(|reason| reason.to_string())
        .or_else(|| {
            client
                .as_ref()
                .and_then(|c| c.disconnect_reason())
                .map(|reason| reason.to_string())
        });
    let message = local
        .message
        .take()
        .or(wire)
        .unwrap_or_else(|| "Disconnected".into());
    *menu = MenuStatus::Failed(message);
    commands.remove_resource::<RenetClient>();
    commands.remove_resource::<NetcodeClientTransport>();
    next_screen.set(Screen::Menu);
}

/// In-game quit: say goodbye to the server and land in the Menu.
pub fn quit_click(
    interactions: Query<&Interaction, (Changed<Interaction>, With<QuitButton>)>,
    client: Option<ResMut<RenetClient>>,
    transport: Option<ResMut<NetcodeClientTransport>>,
    mut local: ResMut<LocalDisconnect>,
) {
    if !interactions.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    local.message = Some("You left the game".into());
    if let (Some(mut client), Some(mut transport)) = (client, transport) {
        transport.disconnect();
        client.disconnect();
    }
}

pub fn render_status_lines(
    projection: Res<Projection>,
    editor: Res<FleetEditor>,
    mut texts: Query<&mut Text, With<StatusLine>>,
) {
    let waiting_for_opponent = editor.submitted && projection.status.is_empty();
    for mut text in &mut texts {
        text.0 = if waiting_for_opponent {
            "Fleet submitted. Waiting for opponent...".into()
        } else {
            projection.status.clone()
        };
    }
}

pub fn send_intent(client: &mut RenetClient, intent: Intent) {
    let bytes = bincode::serialize(&intent).expect("intent serialization failed");
    client.send_message(DefaultChannel::ReliableOrdered, bytes);
}
