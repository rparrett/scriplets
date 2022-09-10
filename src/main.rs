use std::sync::Mutex;
use mlua::prelude::*;
use bevy::prelude::*;

#[derive(Component)]
pub struct LuaState(Mutex<Lua>);

impl LuaState {
    fn new(lua: Lua) -> Self {
        Self(Mutex::new(lua))
    }
}

#[derive(Component)]
pub struct Unit;

#[derive(Component)]
pub struct Movement {
    speed: f32,
    next_move: Vec2
}

fn spawn_unit(mut commands: Commands) {
    let lua = Lua::new();
    commands.spawn()
        .insert(Unit)
        .insert(Movement{speed:1.0, next_move: Vec2::splat(0.0)})
        .insert(LuaState::new(lua))
        .insert_bundle(TransformBundle::default());
}

fn handle_movement(mut units: Query<(&mut Movement, &mut Transform), With<Unit>>) {
    for (mut movement, mut transform) in units.iter_mut() {
        transform.translation += movement.next_move.extend(0.0).clamp_length_max(1.0) * movement.speed;
        movement.next_move = Vec2::splat(0.0);
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_startup_system(spawn_unit)
        .add_system(handle_movement)
        .run();
}
