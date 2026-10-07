use bevy::prelude::*;

pub const CELL_PX: f32 = 30.0;
pub const GAP_PX: f32 = 2.0;
pub const GRID_PX: f32 = 10.0 * CELL_PX + 9.0 * GAP_PX;

pub const COLOR_TEXT: Color = Color::srgb(0.92, 0.94, 0.96);
pub const COLOR_WATER: Color = Color::srgb(0.12, 0.30, 0.55);
pub const COLOR_SHIP: Color = Color::srgb(0.55, 0.58, 0.62);
pub const COLOR_HIT: Color = Color::srgb(0.85, 0.25, 0.20);
pub const COLOR_MISS: Color = Color::srgb(0.30, 0.38, 0.46);
pub const COLOR_SUNK: Color = Color::srgb(0.55, 0.12, 0.10);
pub const COLOR_GHOST_OK: Color = Color::srgb(0.30, 0.65, 0.35);
pub const COLOR_GHOST_BAD: Color = Color::srgb(0.75, 0.25, 0.25);
pub const COLOR_BUTTON: Color = Color::srgb(0.28, 0.34, 0.42);
pub const COLOR_BUTTON_HOVER: Color = Color::srgb(0.38, 0.46, 0.56);
pub const COLOR_BUTTON_DISABLED: Color = Color::srgb(0.18, 0.20, 0.24);
pub const COLOR_PANEL: Color = Color::srgb(0.10, 0.14, 0.20);

pub fn dim(color: Color) -> Color {
    let [r, g, b, a] = color.to_srgba().to_f32_array();
    Color::srgba(r * 0.45, g * 0.45, b * 0.45, a)
}

#[derive(Component)]
pub struct ScreenRoot;

#[derive(Component)]
pub struct QuitButton;

#[derive(Component)]
pub struct StatusLine;

/// Skips the generic hover feedback; the owning screen sets this button's
/// background itself (palette entries, the gated Ready button).
#[derive(Component)]
pub struct NoHoverFeedback;

#[derive(Component)]
pub struct CellPos {
    pub x: u8,
    pub y: u8,
}

pub fn grid_node() -> Node {
    Node {
        width: Val::Px(GRID_PX),
        flex_wrap: FlexWrap::Wrap,
        column_gap: Val::Px(GAP_PX),
        row_gap: Val::Px(GAP_PX),
        ..default()
    }
}

pub fn cell_node() -> Node {
    Node {
        width: Val::Px(CELL_PX),
        height: Val::Px(CELL_PX),
        ..default()
    }
}

pub fn spawn_button(parent: &mut ChildSpawnerCommands, label: &str) -> Entity {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(20.0), Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(COLOR_BUTTON),
        ))
        .with_children(|inner| {
            inner.spawn((
                Text::new(label),
                TextFont::from_font_size(20.0),
                TextColor(COLOR_TEXT),
            ));
        })
        .id()
}

pub fn spawn_label(parent: &mut ChildSpawnerCommands, label: &str, font_size: f32) -> Entity {
    parent
        .spawn((
            Text::new(label),
            TextFont::from_font_size(font_size),
            TextColor(COLOR_TEXT),
        ))
        .id()
}

pub fn fullscreen_column() -> Node {
    Node {
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        row_gap: Val::Px(14.0),
        ..default()
    }
}

pub fn top_right_corner() -> Node {
    Node {
        position_type: PositionType::Absolute,
        top: Val::Px(12.0),
        right: Val::Px(16.0),
        ..default()
    }
}

pub fn bottom_left_corner() -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(16.0),
        bottom: Val::Px(12.0),
        ..default()
    }
}

/// A full-width strip pinned to the bottom edge that centers its children.
pub fn bottom_center_strip() -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        right: Val::Px(0.0),
        bottom: Val::Px(16.0),
        justify_content: JustifyContent::Center,
        ..default()
    }
}

pub fn panel_node() -> Node {
    Node {
        padding: UiRect::axes(Val::Px(28.0), Val::Px(10.0)),
        ..default()
    }
}

type FeedbackButtons<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static mut BackgroundColor),
    (
        Changed<Interaction>,
        With<Button>,
        Without<CellPos>,
        Without<NoHoverFeedback>,
    ),
>;

pub fn button_feedback(mut query: FeedbackButtons) {
    for (interaction, mut background) in &mut query {
        *background = match interaction {
            Interaction::Pressed | Interaction::Hovered => COLOR_BUTTON_HOVER.into(),
            Interaction::None => COLOR_BUTTON.into(),
        };
    }
}
