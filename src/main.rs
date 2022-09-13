use std::sync::Mutex;
use mlua::prelude::*;
use bevy::{prelude::*, window::PresentMode, render::camera::ScalingMode};

const CLEAR_COLOR: Color = Color::rgb(0.1, 0.1, 0.1);
const RESOLUTION: f32 = 16.0 / 9.0;

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

pub struct UnitSprite(Handle<Image>);

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

fn spawn_camera(mut commands: Commands) {
    let mut camera = Camera2dBundle::default();

    camera.projection.top = 1.0;
    camera.projection.bottom = -1.0;
    camera.projection.right = 1.0 * RESOLUTION;
    camera.projection.left = -1.0 * RESOLUTION;

    camera.projection.scaling_mode = ScalingMode::None;

    commands.spawn_bundle(camera);
}

fn spawn_unit(mut commands: Commands, unit_sprite: Res<UnitSprite>) {
    let lua = Lua::new();
    lua.load("function on_tick(handle) handle:move(0.5, 0.5) end").exec().unwrap();
    commands.spawn()
        .insert(Unit)
        .insert(Movement{speed:1.0, next_move: Vec2::splat(0.0)})
        .insert(LuaState::new(lua))
        .insert_bundle(TransformBundle::default())
        .insert_bundle(SpriteBundle{texture: unit_sprite.0.clone(), sprite: Sprite { custom_size: Some(Vec2::splat(0.1)), ..default()}, ..default()});
}

fn print_unit_positions(units: Query<&Transform, With<Unit>>) {
    for (i, unit) in units.iter().enumerate() {
        println!("Unit #{}: {}, {}", i, unit.translation.x, unit.translation.y);
    }
}

fn handle_movement(mut units: Query<(&mut Movement, &mut Transform), With<Unit>>) {
    for (mut movement, mut transform) in units.iter_mut() {
        if movement.next_move != Vec2::ZERO {
            transform.translation += movement.next_move.extend(0.0).clamp_length_max(1.0) * movement.speed;
            movement.next_move = Vec2::ZERO;
        }
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

fn load_sprites(mut commands: Commands, assets: Res<AssetServer>) {
    let unit_sprite = assets.load("unit.png");
    commands.insert_resource(UnitSprite(unit_sprite));
}

fn main() {
    let height = 900.0;
    App::new()
        .insert_resource(ClearColor(CLEAR_COLOR))
        .insert_resource(WindowDescriptor {
            title: "Scriplets".to_string(),
            present_mode: PresentMode::Fifo,
            height,
            width: height * RESOLUTION,
            resizable: false,
            ..default()
        })
        .add_plugins(DefaultPlugins)
        .insert_resource(GameTickTimer(Timer::from_seconds(1.0/60.0, true)))
        .add_startup_system_to_stage(StartupStage::PreStartup, load_sprites)
        .add_startup_system(spawn_unit)
        .add_startup_system(spawn_camera)
        .add_system_to_stage(CoreStage::PreUpdate, unit_tick)
        .add_system(print_unit_positions)
        .add_system(handle_movement)
        .run();
}
