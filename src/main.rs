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
    cooldown_timer: Timer
}

impl Movement {
    fn new(speed: f32) -> Self {
        let cooldown_timer = Timer::from_seconds(60.0 / speed, false);
        Self{speed, cooldown_timer}
    }
}

fn spawn_unit(mut commands: Commands) {
    let lua = Lua::new();
    commands.spawn()
        .insert(Unit)
        .insert(LuaState::new(lua))
        .insert(Movement::new(60.0))
        .insert_bundle(TransformBundle::default());
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_startup_system(spawn_unit)
        .run();
}
