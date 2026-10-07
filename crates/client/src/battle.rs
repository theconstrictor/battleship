use bevy::prelude::*;
use bevy_renet::RenetClient;
use game_core::Intent;

use crate::{
    net::{NetConfig, send_intent},
    projection::{EnemyCellView, OwnCellView, Projection},
    ui::{
        COLOR_HIT, COLOR_MISS, COLOR_PANEL, COLOR_SHIP, COLOR_SUNK, COLOR_TEXT, COLOR_WATER,
        CellPos, QuitButton, ScreenRoot, StatusLine, bottom_center_strip, cell_node, dim,
        fullscreen_column, grid_node, panel_node, spawn_button, spawn_label, top_right_corner,
    },
};

#[derive(Component)]
pub(crate) struct OwnBoardCell;

#[derive(Component)]
pub(crate) struct EnemyBoardCell;

pub fn enter(mut commands: Commands, projection: Res<Projection>, net: Res<NetConfig>) {
    let title = match projection.opponent_name.as_deref() {
        Some(name) => format!("Battle vs {name}"),
        None => "Battle".into(),
    };
    let own_label = format!("Your fleet ({})", net.name);
    let enemy_label = match projection.opponent_name.as_deref() {
        Some(name) => format!("Enemy fleet ({name})"),
        None => "Enemy fleet".into(),
    };
    commands
        .spawn((ScreenRoot, fullscreen_column()))
        .with_children(|parent| {
            spawn_label(parent, &title, 28.0);

            parent
                .spawn(Node {
                    column_gap: Val::Px(48.0),
                    align_items: AlignItems::FlexStart,
                    ..default()
                })
                .with_children(|boards| {
                    boards
                        .spawn(Node {
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            row_gap: Val::Px(6.0),
                            ..default()
                        })
                        .with_children(|own| {
                            spawn_label(own, &own_label, 17.0);
                            own.spawn(grid_node()).with_children(|grid| {
                                for y in 0..10 {
                                    for x in 0..10 {
                                        grid.spawn((
                                            cell_node(),
                                            BackgroundColor(COLOR_WATER),
                                            CellPos { x, y },
                                            OwnBoardCell,
                                        ));
                                    }
                                }
                            });
                        });

                    boards
                        .spawn(Node {
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            row_gap: Val::Px(6.0),
                            ..default()
                        })
                        .with_children(|enemy| {
                            spawn_label(enemy, &enemy_label, 17.0);
                            enemy.spawn(grid_node()).with_children(|grid| {
                                for y in 0..10 {
                                    for x in 0..10 {
                                        grid.spawn((
                                            Button,
                                            cell_node(),
                                            BackgroundColor(COLOR_WATER),
                                            CellPos { x, y },
                                            EnemyBoardCell,
                                        ));
                                    }
                                }
                            });
                        });
                });

            parent
                .spawn(bottom_center_strip())
                .with_children(|strip| {
                    strip
                        .spawn((panel_node(), BackgroundColor(COLOR_PANEL)))
                        .with_children(|panel| {
                            panel.spawn((
                                Text::new(""),
                                TextFont::from_font_size(20.0),
                                TextColor(COLOR_TEXT),
                                StatusLine,
                            ));
                        });
                });

            parent.spawn(top_right_corner()).with_children(|corner| {
                let quit = spawn_button(corner, "Quit");
                corner.commands().entity(quit).insert(QuitButton);
            });
        });
}

pub fn exit(mut commands: Commands, roots: Query<Entity, With<ScreenRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}

type EnemyClickCells<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static CellPos),
    (Changed<Interaction>, With<EnemyBoardCell>),
>;

/// Shots are never pre-checked locally: any click on the enemy board is
/// sent, even out of turn or on an already-shot cell. The authority
/// answers with a ShotResult or a cheap Error.
pub fn enemy_clicks(clicks: EnemyClickCells, mut client: Option<ResMut<RenetClient>>) {
    for (interaction, pos) in &clicks {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(client) = &mut client {
            send_intent(client, Intent::Shot { x: pos.x, y: pos.y });
        }
    }
}

type EnemyBoardCells<'w, 's> = Query<
    'w,
    's,
    (&'static CellPos, &'static mut BackgroundColor),
    (With<EnemyBoardCell>, Without<OwnBoardCell>),
>;

pub fn render(
    projection: Res<Projection>,
    mut own: Query<(&CellPos, &mut BackgroundColor), With<OwnBoardCell>>,
    mut enemy: EnemyBoardCells,
) {
    let my_turn = projection.your_turn != Some(false);
    for (pos, mut background) in &mut own {
        *background = match projection.own_cell(pos.x as usize, pos.y as usize) {
            OwnCellView::Water => COLOR_WATER,
            OwnCellView::Ship => COLOR_SHIP,
            OwnCellView::Miss => COLOR_MISS,
            OwnCellView::Hit => COLOR_HIT,
        }
        .into();
    }
    for (pos, mut background) in &mut enemy {
        let color = match projection.enemy_cell(pos.x as usize, pos.y as usize) {
            EnemyCellView::Unknown => COLOR_WATER,
            EnemyCellView::Miss => COLOR_MISS,
            EnemyCellView::Hit => COLOR_HIT,
            EnemyCellView::Sunk => COLOR_SUNK,
        };
        *background = if my_turn { color } else { dim(color) }.into();
    }
}
