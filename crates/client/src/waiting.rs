use bevy::prelude::*;

use crate::{
    net::NetConfig,
    ui::{COLOR_TEXT, QuitButton, ScreenRoot, dim, fullscreen_column, spawn_button, spawn_label},
};

pub fn enter(mut commands: Commands, net: Res<NetConfig>) {
    commands
        .spawn((ScreenRoot, fullscreen_column()))
        .with_children(|parent| {
            spawn_label(parent, "Waiting for an opponent...", 28.0);
            parent.spawn((
                Text::new(format!("connected to {}", net.server)),
                TextFont::from_font_size(15.0),
                TextColor(dim(COLOR_TEXT)),
            ));
            let quit = spawn_button(parent, "Quit");
            parent.commands().entity(quit).insert(QuitButton);
        });
}

pub fn exit(mut commands: Commands, roots: Query<Entity, With<ScreenRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}
