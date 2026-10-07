use bevy::prelude::*;

use crate::{
    net::{MenuStatus, NetConfig, build_client},
    ui::{
        COLOR_TEXT, ScreenRoot, bottom_left_corner, dim, fullscreen_column, spawn_button,
        spawn_label,
    },
};

#[derive(Component)]
pub(crate) struct MenuStatusText;

#[derive(Component)]
pub(crate) struct RetryButton;

pub fn enter(mut commands: Commands, net: Res<NetConfig>) {
    commands
        .spawn((ScreenRoot, fullscreen_column()))
        .with_children(|parent| {
            spawn_label(parent, "Battleship", 44.0);
            parent.spawn((
                Text::new("sink the enemy fleet first"),
                TextFont::from_font_size(17.0),
                TextColor(dim(COLOR_TEXT)),
            ));
            parent.spawn((
                Text::new(""),
                TextFont::from_font_size(24.0),
                TextColor(COLOR_TEXT),
                MenuStatusText,
            ));
            let retry = spawn_button(parent, "Retry");
            parent
                .commands()
                .entity(retry)
                .insert((RetryButton, Visibility::Hidden));
            parent.spawn(bottom_left_corner()).with_children(|footer| {
                footer.spawn((
                    Text::new(format!("playing as {} · {}", net.name, net.server)),
                    TextFont::from_font_size(14.0),
                    TextColor(dim(COLOR_TEXT)),
                ));
            });
        });
}

pub fn exit(mut commands: Commands, roots: Query<Entity, With<ScreenRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}

pub fn update(
    menu: Res<MenuStatus>,
    mut status_texts: Query<&mut Text, With<MenuStatusText>>,
    mut retry_buttons: Query<&mut Visibility, With<RetryButton>>,
    retry_clicks: Query<&Interaction, (Changed<Interaction>, With<RetryButton>)>,
    net: Res<NetConfig>,
    mut commands: Commands,
) {
    let failed = match &*menu {
        MenuStatus::Connecting => None,
        MenuStatus::Failed(reason) => Some(reason),
    };
    for mut text in &mut status_texts {
        text.0 = match failed {
            None => "Connecting to server...".into(),
            Some(reason) => reason.clone(),
        };
    }
    for mut visibility in &mut retry_buttons {
        *visibility = if failed.is_some() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if retry_clicks.iter().any(|i| *i == Interaction::Pressed) {
        let (client, transport) = build_client(net.server);
        commands.insert_resource(client);
        commands.insert_resource(transport);
        commands.insert_resource(MenuStatus::Connecting);
    }
}
