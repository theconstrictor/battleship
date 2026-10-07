use bevy::prelude::*;
use bevy_renet::RenetClient;
use game_core::Intent;

use crate::{
    net::send_intent,
    projection::Projection,
    ui::{COLOR_TEXT, QuitButton, ScreenRoot, dim, fullscreen_column, spawn_button},
};

#[derive(Component)]
pub(crate) struct OutcomeText;

#[derive(Component)]
pub(crate) struct RematchButton;

pub fn enter(mut commands: Commands, projection: Res<Projection>) {
    let subtext = match projection.opponent_name.as_deref() {
        Some(name) => format!("vs {name}"),
        None => String::new(),
    };
    commands
        .spawn((ScreenRoot, fullscreen_column()))
        .with_children(|parent| {
            parent.spawn((
                Text::new(""),
                TextFont::from_font_size(42.0),
                TextColor(COLOR_TEXT),
                OutcomeText,
            ));
            parent.spawn((
                Text::new(subtext),
                TextFont::from_font_size(17.0),
                TextColor(dim(COLOR_TEXT)),
            ));
            let rematch = spawn_button(parent, "Rematch");
            parent.commands().entity(rematch).insert(RematchButton);
            let quit = spawn_button(parent, "Quit");
            parent.commands().entity(quit).insert(QuitButton);
        });
}

pub fn exit(mut commands: Commands, roots: Query<Entity, With<ScreenRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}

pub fn update(
    projection: Res<Projection>,
    mut outcomes: Query<&mut Text, With<OutcomeText>>,
    mut rematch_buttons: Query<&mut Visibility, With<RematchButton>>,
    rematch_clicks: Query<&Interaction, (Changed<Interaction>, With<RematchButton>)>,
    mut client: Option<ResMut<RenetClient>>,
) {
    for mut text in &mut outcomes {
        text.0 = match (projection.winner, projection.seat) {
            (Some(winner), Some(seat)) if winner == seat => "You win!".into(),
            (Some(_), Some(_)) => "You lose.".into(),
            _ => "Game over".into(),
        };
    }
    for mut visibility in &mut rematch_buttons {
        *visibility = if projection.rematch_rejected {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
    if !projection.rematch_rejected
        && rematch_clicks.iter().any(|i| *i == Interaction::Pressed)
        && let Some(client) = &mut client
    {
        send_intent(client, Intent::Rematch);
    }
}
