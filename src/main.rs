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

pub struct GameTickTimer(Timer);

pub struct UnitHandle<'a> {
    movement: &'a mut Movement
}

impl LuaUserData for UnitHandle<'_> {
    fn add_methods<'lua, M: LuaUserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method_mut("move", |_lua, mut handle, args: (f32, f32)| {
            handle.movement.next_move = Vec2::from(args);
            Ok(())
        });
    }
}

fn spawn_unit(mut commands: Commands) {
    let lua = Lua::new();
    lua.load("function on_tick(handle) handle:move(0.5, 0.5) end").exec().unwrap();
    commands.spawn()
        .insert(Unit)
        .insert(Movement{speed:1.0, next_move: Vec2::splat(0.0)})
        .insert(LuaState::new(lua))
        .insert_bundle(TransformBundle::default());
}

fn print_unit_positions(units: Query<&Transform, With<Unit>>) {
    for (i, unit) in units.iter().enumerate() {
        println!("Unit #{}: {}, {}", i, unit.translation.x, unit.translation.y);
    }
}

fn handle_movement(mut units: Query<(&mut Movement, &mut Transform), With<Unit>>) {
    for (mut movement, mut transform) in units.iter_mut() {
        transform.translation += movement.next_move.extend(0.0).clamp_length_max(1.0) * movement.speed;
        movement.next_move = Vec2::splat(0.0);
    }
}

fn unit_tick(mut units: Query<(&LuaState, &mut Movement), With<Unit>>, mut game_tick_timer: ResMut<GameTickTimer>, time: Res<Time>) {
    if game_tick_timer.0.tick(time.delta()).just_finished() {
        for (lua, mut movement) in units.iter_mut() {
            let lua_lock = lua.0.lock().unwrap();
            {
                let globals = lua_lock.globals();
                if let Some(on_tick) = globals.get::<_, Option<LuaFunction>>("on_tick").unwrap() {
                    lua_lock.scope(|s| {
                        let handle = UnitHandle{movement: &mut movement};
                        let lua_handle = s.create_nonstatic_userdata(handle)?;
                        on_tick.call(lua_handle)?;
                        Ok(())
                    }).unwrap();
                };
            };
        }
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(GameTickTimer(Timer::from_seconds(1.0/60.0, true)))
        .add_startup_system(spawn_unit)
        .add_system(print_unit_positions)
        .add_system(handle_movement)
        .add_system_to_stage(CoreStage::PreUpdate, unit_tick)
        .run();
}
