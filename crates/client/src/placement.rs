use bevy::prelude::*;
use bevy_renet::RenetClient;
use game_core::{
    Intent, ShipSpec,
    board::{SHIP_NAMES, SHIP_SIZES},
};

use crate::{
    net::send_intent,
    projection::{FleetEditor, Projection},
    ui::{
        COLOR_BUTTON, COLOR_BUTTON_DISABLED, COLOR_BUTTON_HOVER, COLOR_GHOST_BAD, COLOR_GHOST_OK,
        COLOR_SHIP, COLOR_TEXT, COLOR_WATER, CellPos, NoHoverFeedback, QuitButton, ScreenRoot,
        StatusLine, bottom_center_strip, cell_node, dim, fullscreen_column, grid_node,
        spawn_button, spawn_label, top_right_corner,
    },
};

#[derive(Component)]
pub(crate) struct PlacementCell;

#[derive(Component)]
pub(crate) struct PaletteButton {
    ship: u8,
}

#[derive(Component)]
pub(crate) struct PaletteCell {
    ship: u8,
}

#[derive(Component)]
pub(crate) struct RandomizeButton;

#[derive(Component)]
pub(crate) struct ReadyButton;

pub fn enter(mut commands: Commands, mut editor: ResMut<FleetEditor>) {
    editor.reset();
    commands
        .spawn((ScreenRoot, fullscreen_column()))
        .with_children(|parent| {
            spawn_label(parent, "Place your fleet", 30.0);

            parent
                .spawn(Node {
                    column_gap: Val::Px(48.0),
                    align_items: AlignItems::FlexStart,
                    ..default()
                })
                .with_children(|main| {
                    main.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|palette_column| {
                        spawn_label(palette_column, "Fleet", 18.0);
                        for ship in 0..5 {
                            palette_column
                                .spawn((
                                    Button,
                                    Node {
                                        width: Val::Px(180.0),
                                        flex_direction: FlexDirection::Column,
                                        align_items: AlignItems::Center,
                                        padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                                        row_gap: Val::Px(4.0),
                                        ..default()
                                    },
                                    BackgroundColor(COLOR_BUTTON),
                                    PaletteButton { ship: ship as u8 },
                                    NoHoverFeedback,
                                ))
                                .with_children(|entry| {
                                    entry.spawn((
                                        Text::new(SHIP_NAMES[ship]),
                                        TextFont::from_font_size(16.0),
                                        TextColor(COLOR_TEXT),
                                    ));
                                    entry
                                        .spawn(Node {
                                            column_gap: Val::Px(2.0),
                                            ..default()
                                        })
                                        .with_children(|preview| {
                                            for _ in 0..SHIP_SIZES[ship] {
                                                preview.spawn((
                                                    Node {
                                                        width: Val::Px(13.0),
                                                        height: Val::Px(13.0),
                                                        ..default()
                                                    },
                                                    BackgroundColor(COLOR_SHIP),
                                                    PaletteCell { ship: ship as u8 },
                                                ));
                                            }
                                        });
                                });
                        }
                    });

                    main.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(12.0),
                        ..default()
                    })
                    .with_children(|board_column| {
                        board_column.spawn(grid_node()).with_children(|grid| {
                            for y in 0..10 {
                                for x in 0..10 {
                                    grid.spawn((
                                        Button,
                                        cell_node(),
                                        BackgroundColor(COLOR_WATER),
                                        CellPos { x, y },
                                        PlacementCell,
                                    ));
                                }
                            }
                        });
                        board_column
                            .spawn(Node {
                                column_gap: Val::Px(10.0),
                                ..default()
                            })
                            .with_children(|controls| {
                                let randomize = spawn_button(controls, "Randomize");
                                controls.commands().entity(randomize).insert(RandomizeButton);
                                let ready = spawn_button(controls, "Ready");
                                controls
                                    .commands()
                                    .entity(ready)
                                    .insert((ReadyButton, NoHoverFeedback));
                            });
                    });
                });

            parent
                .spawn(bottom_center_strip())
                .with_children(|strip| {
                    strip.spawn((
                        Text::new(""),
                        TextFont::from_font_size(17.0),
                        TextColor(COLOR_TEXT),
                        StatusLine,
                    ));
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

type PlacementCells<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static CellPos),
    (Changed<Interaction>, With<PlacementCell>),
>;

pub fn cell_interactions(mut editor: ResMut<FleetEditor>, cells: PlacementCells) {
    for (interaction, pos) in &cells {
        match interaction {
            Interaction::Hovered => editor.hover = Some((pos.x, pos.y)),
            Interaction::Pressed if !editor.submitted => {
                if editor.carried.is_some() {
                    editor.place_carried_at(pos.x, pos.y);
                } else {
                    editor.pick_up_at(pos.x, pos.y);
                }
            }
            Interaction::None if editor.hover == Some((pos.x, pos.y)) => editor.hover = None,
            _ => {}
        }
    }
}

pub fn palette_interactions(
    mut editor: ResMut<FleetEditor>,
    palette: Query<(&Interaction, &PaletteButton), Changed<Interaction>>,
) {
    for (interaction, button) in &palette {
        if *interaction == Interaction::Pressed {
            editor.pick_up_from_palette(button.ship);
        }
    }
}

pub fn controls(
    mut editor: ResMut<FleetEditor>,
    mut projection: ResMut<Projection>,
    randomize: Query<&Interaction, (Changed<Interaction>, With<RandomizeButton>)>,
    ready: Query<&Interaction, (Changed<Interaction>, With<ReadyButton>)>,
    keys: Res<ButtonInput<KeyCode>>,
    client: Option<ResMut<RenetClient>>,
) {
    if randomize.iter().any(|i| *i == Interaction::Pressed) {
        editor.randomize(&mut rand::rng());
    }

    if ready.iter().any(|i| *i == Interaction::Pressed) && editor.ready_allowed() {
        let ships = editor
            .fleet()
            .expect("ready_allowed implies a complete fleet");
        if let Some(mut client) = client {
            send_intent(&mut client, Intent::ShipsPlaced { ships: ships.clone() });
            projection.record_fleet(ships);
            editor.submitted = true;
        }
    }

    if keys.just_pressed(KeyCode::KeyR) {
        editor.rotate_carried();
    }
    if keys.just_pressed(KeyCode::Escape) {
        editor.return_carried();
    }
}

type PlacementCellBackgrounds<'w, 's> = Query<
    'w,
    's,
    (&'static CellPos, &'static mut BackgroundColor),
    (
        With<PlacementCell>,
        Without<PaletteButton>,
        Without<PaletteCell>,
        Without<ReadyButton>,
    ),
>;

type PaletteButtonBackgrounds<'w, 's> = Query<
    'w,
    's,
    (&'static PaletteButton, &'static mut BackgroundColor),
    (
        With<PaletteButton>,
        Without<PlacementCell>,
        Without<PaletteCell>,
        Without<ReadyButton>,
    ),
>;

type PaletteCellBackgrounds<'w, 's> = Query<
    'w,
    's,
    (&'static PaletteCell, &'static mut BackgroundColor),
    (
        With<PaletteCell>,
        Without<PlacementCell>,
        Without<PaletteButton>,
        Without<ReadyButton>,
    ),
>;

type ReadyButtonBackgrounds<'w, 's> = Query<
    'w,
    's,
    &'static mut BackgroundColor,
    (
        With<ReadyButton>,
        Without<PlacementCell>,
        Without<PaletteButton>,
        Without<PaletteCell>,
    ),
>;

pub fn render(
    editor: Res<FleetEditor>,
    mut cells: PlacementCellBackgrounds,
    mut palette: PaletteButtonBackgrounds,
    mut palette_cells: PaletteCellBackgrounds,
    mut ready: ReadyButtonBackgrounds,
) {
    for (pos, mut background) in &mut cells {
        let (x, y) = (pos.x as usize, pos.y as usize);
        let color = if let Some(ghost) = ghost_color(&editor, x, y) {
            ghost
        } else if editor
            .placed
            .iter()
            .flatten()
            .any(|s| s.cells().any(|(cx, cy)| cx as usize == x && cy as usize == y))
        {
            COLOR_SHIP
        } else {
            COLOR_WATER
        };
        *background = color.into();
    }
    for (button, mut background) in &mut palette {
        let placed = editor.placed[button.ship as usize].is_some();
        let carried = editor.carried.is_some_and(|c| c.ship == button.ship);
        *background = if placed {
            COLOR_BUTTON_DISABLED.into()
        } else if carried {
            COLOR_BUTTON_HOVER.into()
        } else {
            COLOR_BUTTON.into()
        };
    }
    for (cell, mut background) in &mut palette_cells {
        *background = if editor.placed[cell.ship as usize].is_some() {
            dim(COLOR_SHIP).into()
        } else {
            COLOR_SHIP.into()
        };
    }
    for mut background in &mut ready {
        *background = if editor.ready_allowed() {
            COLOR_BUTTON.into()
        } else {
            COLOR_BUTTON_DISABLED.into()
        };
    }
}

fn ghost_color(editor: &FleetEditor, x: usize, y: usize) -> Option<Color> {
    let carried = editor.carried?;
    let (hx, hy) = editor.hover?;
    let spec = ShipSpec {
        x: hx,
        y: hy,
        ..carried
    };
    spec.cells()
        .any(|(cx, cy)| cx as usize == x && cy as usize == y)
        .then(|| {
            if editor.can_place(spec) {
                COLOR_GHOST_OK
            } else {
                COLOR_GHOST_BAD
            }
        })
}
