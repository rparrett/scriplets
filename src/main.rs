use std::{sync::Mutex, collections::HashMap, path::PathBuf, fs::File, f32::consts::PI};
use mlua::prelude::*;
use bevy::{prelude::*, window::PresentMode, render::camera::ScalingMode, input::mouse::{MouseWheel, MouseScrollUnit, MouseMotion}, time::Stopwatch, asset::AssetServerSettings};
use bevy_rapier2d::prelude::*;
use serde::{Deserialize, Deserializer};
use scriplets_derive::ComponentPrototype;
use strum::AsRefStr;

const CLEAR_COLOR: Color = Color::rgb(0.1, 0.1, 0.1);
const RESOLUTION: f32 = 16.0 / 9.0;

// General TODO list
// - split into client and server
// - code editing gui

// General ideas
//  Black box: a component that can store data when unit is running and extracted from a unit
//  corpse as an item and be read by other units.
//  
//  Items
//  Units with manipulators specify an area that they want to pick up from. They are given a list
//  of what can be picked up and then they choose what is picked up
//
//  Items with data
//  Similar to black box, can have data written and read. Can be encrypted. No actual encryption
//  will be done, just comparing the keys.
//
//  Possible new language: wasm

// TODO: program
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

#[derive(Deserialize)]
pub struct Prototypes {
    #[serde(deserialize_with = "hashmap_from_sequence")]
    movement: HashMap<String, Movement>
}

pub trait ComponentPrototype<'de, T: Component = Self>: Deserialize<'de> {
    fn name(&self) -> &str;
    fn to_component(&self) -> T;
    fn from_pt(prototypes_table: &Prototypes, name: &str) -> Option<T>;
}

pub fn hashmap_from_sequence<'de, D: Deserializer<'de>, C: ComponentPrototype<'de, T>, T: Component>(deserializer: D) -> Result<HashMap<String, C>, D::Error> {
    Ok(Vec::<C>::deserialize(deserializer)?.into_iter().map(|p| (p.name().to_string(), p)).collect())
}

// TODO: reimplement acceleration movement type to support steering around a point
//  Or make a new movement type which works as stated above
#[derive(Component, Deserialize, Clone, ComponentPrototype)]
#[prot_category(movement)]
pub struct Movement {
    name: String,
    movement_type: MovementType,
    // movement characteristics
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
    #[serde(default)]
    rotation_offset: f32,
    // input 
    #[serde(skip)]
    input_move: Vec2,
    #[serde(skip)]
    input_rotation: f32,
    #[serde(default)]
    hand_brake: bool
}

#[derive(Deserialize, Clone, AsRefStr)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum MovementType {
    Omnidirectional,
    AcceleratedSteering,
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

// TODO: after making a planet map, methods for getting nearest transition tile or a tile adjacent
//  to transition tile
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
        methods.add_method_mut("toggle_hand_brake", |_lua, handle, ()| {
            if let Some(movement) = &mut handle.movement {
                movement.hand_brake = !movement.hand_brake;
            }
            Ok(())
        })
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
            let rotation_degrees = -(rotation_radians * 180.0) / PI;
            let table = lua.create_table()?;
            table.set("position", position)?;
            table.set("rotation", rotation_degrees)?;
            Ok(table)
        });
        fields.add_field_method_get("movement", |lua, handle| {
            if let Some(movement) = &handle.movement {
                let movement_type = movement.movement_type.as_ref();
                let speed = movement.speed;
                let max_speed = movement.max_speed;
                let max_speed_backwards = movement.max_speed_backwards;
                let acceleration = movement.acceleration;
                let braking_acceleration = movement.acceleration;
                let passive_deceleration = movement.passive_deceleration;
                let rotation_speed = movement.rotation_speed;
                let hand_brake = movement.hand_brake;
                let table = lua.create_table()?;
                table.set("movement_type", movement_type)?;
                table.set("speed", speed)?;
                table.set("max_speed", max_speed)?;
                table.set("max_speed_backwards", max_speed_backwards)?;
                table.set("acceleration", acceleration)?;
                table.set("braking_acceleration", braking_acceleration)?;
                table.set("passive_deceleration", passive_deceleration)?;
                table.set("rotation_speed", rotation_speed)?;
                table.set("is_hand_brake_pulled", hand_brake)?;
                Ok(LuaValue::Table(table))
            } else {
                Ok(LuaValue::Nil)
            }
        })
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
    component_prototypes: Res<Prototypes>)
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
                if !movement.hand_brake {
                    if movement.input_rotation != 0.0 {
                        let rotation = Quat::from_rotation_z(-(movement.rotation_speed * movement.input_rotation.clamp(-1.0, 1.0) * PI) / (180.0 * 60.0));
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
                }
            },
            MovementType::AcceleratedSteering => {
                let input_move_vec = movement.input_move.clamp(Vec2::NEG_X + Vec2::NEG_Y, Vec2::X + Vec2::Y);
                let max_speed = movement.max_speed;
                let max_speed_backwards = -movement.max_speed_backwards.unwrap_or(max_speed);
                let acceleration = movement.acceleration;
                let braking_acceleration = -movement.braking_acceleration.unwrap_or(acceleration);
                let passive_deceleration = movement.passive_deceleration;
                let new_speed = {
                    let acceleration = {
                        if movement.hand_brake {
                            if movement.speed > 0.0 {
                                braking_acceleration
                            } else {
                                -braking_acceleration
                            }
                        } else {
                            if (movement.speed > 0.0 && input_move_vec.x > 0.0) || (movement.speed < 0.0 && input_move_vec.x < 0.0) {
                                acceleration
                            } else if (movement.speed > 0.0 && input_move_vec.x < 0.0) || (movement.speed < 0.0 && input_move_vec.x > 0.0) {
                                braking_acceleration
                            } else if movement.speed != 0.0 {
                                -passive_deceleration
                            } else {
                                acceleration
                            }
                        }
                    };
                    (movement.speed + acceleration * input_move_vec.x / 60.0).clamp(max_speed_backwards, max_speed)
                };
                movement.speed = new_speed;
                if movement.speed != 0.0 {
                    let linear_delta = movement.speed / 60.0;
                    let starting_translation = transform.translation.truncate();
                    let mut rot_angle = (movement.rotation_speed * PI / (60.0 * 180.0)) * input_move_vec.y;
                    if movement.speed < 0.0 {
                        rot_angle = -rot_angle;
                    }
                    let result_rotation = transform.rotation * Quat::from_rotation_z(-rot_angle);
                    let turning_scale = linear_delta / rot_angle;
                    let rot_vec_normalized = Vec2::from_angle(rot_angle);
                    let turning_radius = transform.right().truncate() * turning_scale;
                    let turning_origin = starting_translation - turning_radius;
                    let result_translation = turning_radius.rotate(rot_vec_normalized) + turning_origin;
                    
                    let delta = result_translation - starting_translation;
                    let shape_pos = result_translation;
                    let shape_rot = result_rotation.to_euler(EulerRot::XYZ).2;
                    let max_toi = 1.0;
                    let filter = QueryFilter::default()
                        .exclude_collider(entity)
                        .exclude_sensors();
                    if rapier_context.cast_shape(shape_pos, shape_rot, delta, collider, max_toi, filter).is_none() {
                        transform.translation = result_translation.extend(0.0);
                        transform.rotation = result_rotation;
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

fn print_units_positions(units: Query<&Transform, With<Unit>>) {
    for (i, unit) in units.iter().enumerate() {
        println!("Unit #{}: x {}, y {}", i, unit.translation.x, unit.translation.y)
    }
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
    let prototypes: Prototypes = serde_json::from_reader(prototypes_file).unwrap();
    commands.insert_resource(prototypes)
}

fn main() {
    let height = 900.0;
    let mut app = App::new();
    app.insert_resource(ClearColor(CLEAR_COLOR))
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
        .insert_resource(GameClock(Stopwatch::default()))
        .add_startup_system_to_stage(StartupStage::PreStartup, load_assets)
        .add_startup_system(spawn_walls)
        .add_startup_system(spawn_unit)
        .add_startup_system(spawn_camera)
        .add_system_to_stage(CoreStage::First, tick_units_clocks)
        .add_system_to_stage(CoreStage::PreUpdate, unit_tick)
        .add_system(print_units_positions)
        .add_system(game_clock_tick)
        .add_system(handle_movement)
        .add_system(move_and_zoom_camera);
    #[cfg(feature = "debug")]
    app.add_plugin(RapierDebugRenderPlugin::default());
    app.run()
}
