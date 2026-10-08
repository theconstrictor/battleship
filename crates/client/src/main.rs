mod battle;
mod gameover;
mod menu;
mod net;
mod placement;
mod projection;
mod screen;
mod ui;
mod waiting;

use std::net::{SocketAddr, ToSocketAddrs};

use bevy::prelude::*;
use bevy_renet::{
    RenetClient, RenetClientPlugin, client_just_connected, client_just_disconnected,
    netcode::NetcodeClientPlugin,
};
use game_core::DEFAULT_PORT;

use net::{LocalDisconnect, MenuStatus, NetConfig};
use projection::{FleetEditor, Projection};
use screen::Screen;

fn main() {
    let (name, server) = args();
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Battleship".into(),
                resolution: (1280, 800).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins((RenetClientPlugin, NetcodeClientPlugin))
        .init_state::<Screen>()
        .insert_resource(NetConfig { name, server })
        .insert_resource(Projection::default())
        .insert_resource(FleetEditor::default())
        .insert_resource(MenuStatus::default())
        .insert_resource(LocalDisconnect::default())
        .add_systems(Startup, net::setup)
        .add_systems(
            Update,
            (
                net::apply_facts.run_if(resource_exists::<RenetClient>),
                net::on_connected.run_if(client_just_connected),
                net::on_disconnected.run_if(client_just_disconnected),
                net::quit_click,
                net::render_status_lines,
                ui::button_feedback,
            ),
        )
        .add_systems(OnEnter(Screen::Menu), menu::enter)
        .add_systems(OnExit(Screen::Menu), menu::exit)
        .add_systems(Update, menu::update.run_if(in_state(Screen::Menu)))
        .add_systems(OnEnter(Screen::Waiting), waiting::enter)
        .add_systems(OnExit(Screen::Waiting), waiting::exit)
        .add_systems(OnEnter(Screen::Placement), placement::enter)
        .add_systems(OnExit(Screen::Placement), placement::exit)
        .add_systems(
            Update,
            (
                placement::cell_interactions,
                placement::palette_interactions,
                placement::controls,
                placement::render,
            )
                .chain()
                .run_if(in_state(Screen::Placement)),
        )
        .add_systems(OnEnter(Screen::Battle), battle::enter)
        .add_systems(OnExit(Screen::Battle), battle::exit)
        .add_systems(
            Update,
            (battle::enemy_clicks, battle::render)
                .chain()
                .run_if(in_state(Screen::Battle)),
        )
        .add_systems(OnEnter(Screen::GameOver), gameover::enter)
        .add_systems(OnExit(Screen::GameOver), gameover::exit)
        .add_systems(Update, gameover::update.run_if(in_state(Screen::GameOver)))
        .run();
}

fn args() -> (String, SocketAddr) {    let mut name = "player".to_string();
    let mut server = SocketAddr::from(([127, 0, 0, 1], DEFAULT_PORT));
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--name" => name = args.next().expect("--name requires a value"),
            "--server" => {
                let value = args.next().expect("--server requires a value");
                server = resolve_server(&value);
            }
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(1);
            }
        }
    }
    (name, server)
}

/// Resolve `host:port` to a socket address; the host may be a domain name,
/// a hostname, or an IP literal.
fn resolve_server(host_port: &str) -> SocketAddr {
    let mut addrs = host_port.to_socket_addrs().unwrap_or_else(|error| {
        eprintln!("failed to resolve server address {host_port}: {error}");
        std::process::exit(1);
    });
    addrs.next().unwrap_or_else(|| {
        eprintln!("server address {host_port} resolved to no addresses");
        std::process::exit(1);
    })
}
