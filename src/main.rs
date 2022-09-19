use std::sync::Mutex;
use mlua::prelude::*;
use bevy::{prelude::*, window::PresentMode, render::camera::ScalingMode, input::mouse::{MouseWheel, MouseScrollUnit, MouseMotion}, time::Stopwatch};
use bevy_rapier2d::prelude::*;

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
    name: String,
    speed: f32,
    input_move: Vec2
}

#[derive(Component)]
pub struct UnitClock(Stopwatch);

pub struct GameClock(Stopwatch);

pub struct UnitSprite(Handle<Image>);
pub struct WallSprite(Handle<Image>);

pub struct UnitHandle<'a> {
    movement: &'a mut Movement,
    clock: &'a UnitClock,
    game_clock: &'a GameClock
}

impl LuaUserData for UnitHandle<'_> {
    fn add_methods<'lua, M: LuaUserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method_mut("move", |_lua, mut handle, args: (f32, f32)| {
            handle.movement.input_move = Vec2::from(args);
            Ok(())
        });
    }

    fn add_fields<'lua, F: LuaUserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field_method_get("time_since_start", |_lua, handle| {
            Ok(handle.clock.0.elapsed_secs())
        });
        fields.add_field_method_get("global_time", |_lua, handle| {
            Ok(handle.game_clock.0.elapsed_secs())
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

fn move_and_zoom_camera(
    mut camera: Query<(&mut OrthographicProjection, &mut Transform), With<Camera2d>>,
    input: Res<Input<MouseButton>>,
    mut mouse_scroll_evr: EventReader<MouseWheel>,
    mut mouse_move_evr: EventReader<MouseMotion>)
{
    let (mut camera, mut camera_transform) = camera.single_mut();
    for scroll_event in mouse_scroll_evr.iter() {
        match scroll_event.unit {
            MouseScrollUnit::Line => camera.scale = (camera.scale - 0.5 * scroll_event.y).clamp(1.0, 20.0),
            MouseScrollUnit::Pixel => camera.scale = (camera.scale - 0.1 * scroll_event.y).clamp(1.0, 20.0)
        }
    }
    for move_event in mouse_move_evr.iter() {
        if input.pressed(MouseButton::Middle) {
            let mut delta = move_event.delta * 0.0025 * camera.scale;
            delta.x = -delta.x;
            camera_transform.translation += delta.extend(0.0);
        }
    }
}

fn spawn_unit(mut commands: Commands, unit_sprite: Res<UnitSprite>) {
    let lua = Lua::new();
    lua.load("function on_tick(handle) handle:move(1, 1) end").exec().unwrap();
    commands.spawn()
        .insert(Unit)
        .insert(UnitClock(Stopwatch::default()))
        .insert(Movement{name: "".into(), speed:1.0, input_move: Vec2::splat(0.0)})
        .insert(LuaState::new(lua))
        .insert_bundle(TransformBundle::default())
        .insert(Collider::cuboid(0.499, 0.499))
        .insert(RigidBody::KinematicPositionBased)
        .insert_bundle(SpriteBundle {
            texture: unit_sprite.0.clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::splat(1.0)),
                ..default()
            },
            ..default()});
}

fn spawn_walls(mut commands: Commands, wall_sprite: Res<WallSprite>) {
    for i in 1..=5 {
        spawn_wall(&mut commands, i as f32, 5.0, &wall_sprite.0)
    }
    for j in 0..=4 {
        spawn_wall(&mut commands, 5.0, j as f32, &wall_sprite.0)
    }
    spawn_wall(&mut commands, -1.0, 5.0, &wall_sprite.0)
}

fn spawn_wall(commands: &mut Commands, x: f32, y: f32, sprite: &Handle<Image>) {
    let transform = TransformBundle::from(Transform::from_xyz(x, y, 0.0));
    commands.spawn()
        .insert(Collider::cuboid(0.5, 0.5))
        .insert(RigidBody::Fixed)
        .insert_bundle(SpriteBundle {
            texture: sprite.clone(),
            transform: transform.local,
            global_transform: transform.global,
            sprite: Sprite {
                custom_size: Some(Vec2::splat(1.0)),
                ..default()
            },
            ..default()
        });
}

fn handle_movement(
    mut units: Query<(Entity, &mut Movement, &mut Transform, &Collider), With<Unit>>,
    rapier_context: Res<RapierContext>)
{
    for (entity, mut movement, mut transform, collider) in units.iter_mut() {
        if movement.input_move != Vec2::ZERO {
            let delta = movement.input_move.clamp_length_max(1.0) * (movement.speed / 60.0);
            let shape_pos = transform.translation.truncate();
            let shape_rot = transform.rotation.to_euler(EulerRot::XYZ).2;
            let max_toi = 1.0;
            let filter = QueryFilter::default()
                .exclude_collider(entity)
                .exclude_sensors();
            if rapier_context.cast_shape(shape_pos, shape_rot, delta, collider, max_toi, filter).is_none() {
                transform.translation += delta.extend(0.0);
            }
            movement.input_move = Vec2::ZERO;
        }
    }
}

fn unit_tick(
    mut units: Query<(&LuaState, &mut Movement, &mut UnitClock), With<Unit>>,
    game_clock: Res<GameClock>) 
{
    for (lua, mut movement, clock) in units.iter_mut() {
        let lua_lock = lua.0.lock().unwrap();
        {
            let globals = lua_lock.globals();
            if let Some(on_tick) = globals.get::<_, Option<LuaFunction>>("on_tick").unwrap() {
                lua_lock.scope(|s| {
                    let handle = UnitHandle {
                        movement: &mut movement,
                        clock: &clock,
                        game_clock: &game_clock
                    };
                    let lua_handle = s.create_nonstatic_userdata(handle)?;
                    on_tick.call(lua_handle)?;
                    Ok(())
                }).unwrap();
            };
        };
    }
}

fn tick_units_clocks(mut units: Query<&mut UnitClock, With<Unit>>, time: Res<Time>) {
    units.iter_mut().for_each(|mut unit| {unit.0.tick(time.delta());})
}

fn game_clock_tick(mut clock: ResMut<GameClock>, time: Res<Time>) {
    clock.0.tick(time.delta());
}

fn load_sprites(mut commands: Commands, assets: Res<AssetServer>) {
    let unit_sprite = assets.load("unit.png");
    commands.insert_resource(UnitSprite(unit_sprite));
    let wall_sprite = assets.load("wall.png");
    commands.insert_resource(WallSprite(wall_sprite));
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
        .add_plugin(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(32.0))
        .add_plugin(RapierDebugRenderPlugin::default())
        .insert_resource(GameClock(Stopwatch::default()))
        .add_startup_system_to_stage(StartupStage::PreStartup, load_sprites)
        .add_startup_system(spawn_walls)
        .add_startup_system(spawn_unit)
        .add_startup_system(spawn_camera)
        .add_system_to_stage(CoreStage::First, tick_units_clocks)
        .add_system_to_stage(CoreStage::PreUpdate, unit_tick)
        .add_system(game_clock_tick)
        .add_system(handle_movement)
        .add_system(move_and_zoom_camera)
        .run();
}
