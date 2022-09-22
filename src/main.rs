use std::{sync::Mutex, collections::HashMap, path::PathBuf, fs::File};
use mlua::prelude::*;
use bevy::{prelude::*, window::PresentMode, render::camera::ScalingMode, input::mouse::{MouseWheel, MouseScrollUnit, MouseMotion}, time::Stopwatch, asset::AssetServerSettings};
use bevy_rapier2d::prelude::*;
use serde::{Deserialize, Deserializer};

const CLEAR_COLOR: Color = Color::rgb(0.1, 0.1, 0.1);
const RESOLUTION: f32 = 16.0 / 9.0;

#[derive(Component)]
pub struct LuaState {
    lua: Mutex<Lua>
}

impl LuaState {
    fn new(lua: Lua) -> Self {
        Self{
            lua: Mutex::new(lua)
        }
    }
}

#[derive(Component)]
pub struct Unit;

pub trait ComponentPrototype<'de, T: Component = Self>: Deserialize<'de> {
    fn name(&self) -> &String;
    fn to_component(&self) -> T;
    fn from_pt(prototypes_table: &ComponentPrototypes, name: &str) -> Option<T>;
}

#[derive(Deserialize)]
pub struct ComponentPrototypes {
    #[serde(deserialize_with = "hashmap_from_sequence")]
    movement: HashMap<String, Movement>
}

pub fn hashmap_from_sequence<'de, D: Deserializer<'de>, C: ComponentPrototype<'de, T>, T: Component>(deserializer: D) -> Result<HashMap<String, C>, D::Error> {
    Ok(Vec::<C>::deserialize(deserializer)?.into_iter().map(|p| (p.name().clone(), p)).collect())
}

#[derive(Component, Deserialize, Clone)]
pub struct Movement {
    name: String,
    movement_type: MovementType,
    #[serde(default)]
    speed: f32, // tiles / second
    #[serde(default)]
    max_speed: f32,
    #[serde(default)]
    max_speed_backwards: Option<f32>,
    #[serde(default)]
    acceleration: f32, // tiles / second^2
    #[serde(default)]
    braking_acceleration: Option<f32>,
    #[serde(default)]
    passive_deceleration: f32,
    #[serde(default)]
    rotation_speed: f32, // degrees / second
    #[serde(skip)]
    input_move: Vec2,
    #[serde(skip)]
    input_rotation: f32
}

impl ComponentPrototype<'_> for Movement {
    fn name(&self) -> &String {
        &self.name
    }

    fn to_component(&self) -> Self {
        self.clone()
    }

    fn from_pt(prototypes_table: &ComponentPrototypes, name: &str) -> Option<Self> {
        prototypes_table.movement.get(name).map(Self::to_component)
    }
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum MovementType {
    Omnidirectional,
    Accelerated,
    Train
}

#[derive(Component)]
pub struct UnitClock(Stopwatch);

pub struct GameClock(Stopwatch);

pub struct UnitSprite(Handle<Image>);
pub struct WallSprite(Handle<Image>);

pub struct UnitHandle<'a> {
    movement: Option<&'a mut Movement>,
    transform: &'a Transform,
    clock: &'a UnitClock,
    game_clock: &'a GameClock
}

impl LuaUserData for UnitHandle<'_> {
    fn add_methods<'lua, M: LuaUserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method_mut("move", |_lua, handle, args: (f32, f32)| {
            if let Some(movement) = &mut handle.movement {
                movement.input_move = Vec2::from(args);
            };
            Ok(())
        });
        methods.add_method_mut("rotate", |_lua, handle, rot: f32| {
            if let Some(movement) = &mut handle.movement {
                movement.input_rotation = rot;
            }
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
        fields.add_field_method_get("gps", |lua, handle| {
            let position: [f32; 2] = handle.transform.translation.truncate().into();
            let rotation_radians = handle.transform.rotation.to_euler(EulerRot::XYZ).2;
            let rotation_degrees = -(rotation_radians * 180.0) / std::f32::consts::PI;
            let table = lua.create_table()?;
            table.set("position", position)?;
            table.set("rotation", rotation_degrees)?;
            Ok(table)
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

fn spawn_unit(
    mut commands: Commands,
    unit_sprite: Res<UnitSprite>,
    component_prototypes: Res<ComponentPrototypes>)
{
    let lua = Lua::new();
    lua.load(r#"
        function on_tick(handle)
           handle:move(1, 1)
        end
        "#).exec().unwrap();
    let movement = Movement::from_pt(&component_prototypes, "default").unwrap();
    commands.spawn()
        .insert(Unit)
        .insert(UnitClock(Stopwatch::default()))
        .insert(movement)
        .insert(LuaState::new(lua))
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
        match movement.movement_type {
             MovementType::Omnidirectional => {
                 if movement.input_rotation != 0.0 {
                     let rotation = Quat::from_rotation_z(-(movement.rotation_speed * movement.input_rotation.clamp(-1.0, 1.0) * std::f32::consts::PI) / (180.0 * 60.0));
                     transform.rotation *= rotation;
                 }
                 if movement.input_move != Vec2::ZERO {
                    let unrotated_move = movement.input_move.clamp_length_max(1.0) * (movement.speed / 60.0);
                    let delta = Mat2::from_cols(transform.right().truncate(), transform.up().truncate()) * unrotated_move;
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
            },
            MovementType::Accelerated => {
                let move_vec = movement.input_move.clamp(Vec2::NEG_X + Vec2::NEG_Y, Vec2::X + Vec2::Y);
                if move_vec.y != 0.0 {
                     let rotation = Quat::from_rotation_z(-(movement.rotation_speed * move_vec.y * std::f32::consts::PI) / (180.0 * 60.0));
                     transform.rotation *= rotation;
                }
                if move_vec.x != 0.0 {
                    let max_speed = movement.max_speed;
                    let max_speed_backwards = -movement.max_speed_backwards.unwrap_or(max_speed);
                    let acceleration = movement.acceleration;
                    let braking_acceleration = -movement.braking_acceleration.unwrap_or(acceleration);
                    let passive_deceleration = movement.passive_deceleration;
                    let new_speed = {
                        let acceleration = {
                            if (movement.speed > 0.0 && move_vec.x > 0.0) || (movement.speed < 0.0 && move_vec.x < 0.0) {
                                acceleration
                            } else if (movement.speed > 0.0 && move_vec.x < 0.0) || (movement.speed < 0.0 && move_vec.x > 0.0) {
                                braking_acceleration
                            } else if movement.speed != 0.0 {
                                -passive_deceleration
                            } else {
                                acceleration
                            }
                        };
                        (movement.speed + acceleration * move_vec.x).clamp(max_speed_backwards, max_speed)
                    };
                    movement.speed = new_speed
                }
                if movement.speed != 0.0 {
                    let delta = transform.up().truncate() * (movement.speed / 60.0);
                    let shape_pos = transform.translation.truncate();
                    let shape_rot = transform.rotation.to_euler(EulerRot::XYZ).2;
                    let max_toi = 1.0;
                    let filter = QueryFilter::default()
                        .exclude_collider(entity)
                        .exclude_sensors();
                    if rapier_context.cast_shape(shape_pos, shape_rot, delta, collider, max_toi, filter).is_none() {
                        transform.translation += delta.extend(0.0);
                    }
                    movement.input_move = Vec2::ZERO
                }
            }
            _ => {}
        }
    }
}

fn unit_tick(
    mut units: Query<(&LuaState, Option<&mut Movement>, &mut UnitClock, &Transform), With<Unit>>,
    game_clock: Res<GameClock>) 
{
    for (lua, mut movement, clock, transform) in units.iter_mut() {
        let lua_lock = lua.lua.lock().unwrap();
        {
            let globals = lua_lock.globals();
            if let Some(on_tick) = globals.get::<_, Option<LuaFunction>>("on_tick").unwrap() {
                lua_lock.scope(|s| {
                    let handle = UnitHandle {
                        movement: movement.as_deref_mut(),
                        transform,
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

fn load_assets(
    mut commands: Commands,
    assets: Res<AssetServer>,
    asset_settings: Res<AssetServerSettings>)
{
    let unit_sprite = assets.load("unit.png");
    commands.insert_resource(UnitSprite(unit_sprite));
    let wall_sprite = assets.load("wall.png");
    commands.insert_resource(WallSprite(wall_sprite));
    let prototypes_path = PathBuf::from(&asset_settings.asset_folder).join("prototypes.json");
    let prototypes_file = File::open(prototypes_path).unwrap();
    let prototypes: ComponentPrototypes = serde_json::from_reader(prototypes_file).unwrap();
    commands.insert_resource(prototypes)
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
        .add_startup_system_to_stage(StartupStage::PreStartup, load_assets)
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
