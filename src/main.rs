use bevy::{
    asset::LoadState,
    input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel},
    prelude::*,
    render::camera::ScalingMode,
    time::Stopwatch,
    window::PresentMode,
};
use bevy_rapier2d::prelude::*;
use prototypes::{ComponentPrototype, Movement, MovementType, Prototypes, PrototypesLoader};

use std::f32::consts::PI;

mod data_value;
mod program;
mod prototypes;

use program::{UnitHandle, UnitProgram};

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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum AppState {
    Loading,
    Playing,
}

#[derive(Component)]
pub struct Unit;

#[derive(Component)]
pub struct UnitClock(Stopwatch);

pub struct GameClock(Stopwatch);

pub struct UnitSprite(Handle<Image>);
pub struct WallSprite(Handle<Image>);
pub struct PrototypesHandle(Handle<Prototypes>);

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
    mut mouse_move_evr: EventReader<MouseMotion>,
) {
    let (mut camera, mut camera_transform) = camera.single_mut();
    for scroll_event in mouse_scroll_evr.iter() {
        match scroll_event.unit {
            MouseScrollUnit::Line => {
                camera.scale = (camera.scale - 0.5 * scroll_event.y).clamp(1.0, 20.0)
            }
            MouseScrollUnit::Pixel => {
                camera.scale = (camera.scale - 0.1 * scroll_event.y).clamp(1.0, 20.0)
            }
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
    prototypes_handle: Res<PrototypesHandle>,
    prototypes_assets: Res<Assets<Prototypes>>,
) {
    let component_prototypes = prototypes_assets.get(&prototypes_handle.0).unwrap();

    let unit_program = UnitProgram::new_lua_with_program(
        r#"
        function on_tick(handle)
            handle:move(1, 1)
        end
    "#
        .as_bytes(),
    );
    let movement = Movement::component_from_pt(&component_prototypes, "default").unwrap();
    commands
        .spawn()
        .insert(Unit)
        .insert(UnitClock(Stopwatch::default()))
        .insert(movement)
        .insert(unit_program)
        .insert(Collider::cuboid(0.499, 0.499))
        .insert(RigidBody::KinematicPositionBased)
        .insert_bundle(SpriteBundle {
            texture: unit_sprite.0.clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::splat(1.0)),
                ..default()
            },
            ..default()
        });
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
    commands
        .spawn()
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
    rapier_context: Res<RapierContext>,
) {
    for (entity, mut movement, mut transform, collider) in units.iter_mut() {
        match movement.movement_type {
            MovementType::Omnidirectional => {
                if !movement.hand_brake {
                    if movement.input_rotation != 0.0 {
                        let rotation = Quat::from_rotation_z(
                            -(movement.rotation_speed
                                * movement.input_rotation.clamp(-1.0, 1.0)
                                * PI)
                                / (180.0 * 60.0),
                        );
                        transform.rotation *= rotation;
                    }
                    if movement.input_move != Vec2::ZERO {
                        let unrotated_move =
                            movement.input_move.clamp_length_max(1.0) * (movement.speed / 60.0);
                        let delta = unrotated_move.rotate(transform.right().truncate());
                        let shape_pos = transform.translation.truncate();
                        let shape_rot = transform.rotation.to_euler(EulerRot::XYZ).2;
                        let max_toi = 1.0;
                        let filter = QueryFilter::default()
                            .exclude_collider(entity)
                            .exclude_sensors();
                        if rapier_context
                            .cast_shape(shape_pos, shape_rot, delta, collider, max_toi, filter)
                            .is_none()
                        {
                            transform.translation += delta.extend(0.0);
                        }
                        movement.input_move = Vec2::ZERO;
                    }
                }
            }
            MovementType::AcceleratedSteering => {
                let input_move_vec = movement
                    .input_move
                    .clamp(Vec2::NEG_X + Vec2::NEG_Y, Vec2::X + Vec2::Y);
                let max_speed = movement.max_speed;
                let max_speed_backwards = -movement.max_speed_backwards.unwrap_or(max_speed);
                let acceleration = movement.acceleration;
                let braking_acceleration = -movement.braking_acceleration.unwrap_or(acceleration);
                let passive_deceleration = movement.passive_deceleration;
                let is_moving_forward = movement.speed > 0.0;
                let is_moving_backwards = movement.speed < 0.0;
                let new_speed = {
                    let acceleration = {
                        if movement.hand_brake {
                            if movement.speed > 0.0 {
                                braking_acceleration
                            } else {
                                -braking_acceleration
                            }
                        } else if (movement.speed > 0.0 && input_move_vec.x > 0.0)
                            || (movement.speed < 0.0 && input_move_vec.x < 0.0)
                        {
                            acceleration
                        } else if (movement.speed > 0.0 && input_move_vec.x < 0.0)
                            || (movement.speed < 0.0 && input_move_vec.x > 0.0)
                        {
                            braking_acceleration
                        } else if movement.speed != 0.0 {
                            -passive_deceleration
                        } else {
                            acceleration
                        }
                    };
                    let new_speed_uncapped = (movement.speed
                        + acceleration * input_move_vec.x / 60.0)
                        .clamp(max_speed_backwards, max_speed);
                    if is_moving_forward {
                        new_speed_uncapped.clamp(0.0, f32::MAX)
                    } else if is_moving_backwards {
                        new_speed_uncapped.clamp(f32::MIN, 0.0)
                    } else {
                        new_speed_uncapped
                    }
                };
                movement.speed = new_speed;
                if movement.speed != 0.0 {
                    let linear_delta = movement.speed / 60.0;
                    let starting_translation = transform.translation.truncate()
                        + transform.up().truncate() * movement.rotation_offset;
                    let mut rot_angle =
                        (movement.rotation_speed * PI / (60.0 * 180.0)) * input_move_vec.y;
                    if movement.speed < 0.0 {
                        rot_angle = -rot_angle;
                    }
                    let result_rotation = transform.rotation * Quat::from_rotation_z(-rot_angle);
                    let turning_scale = linear_delta / rot_angle;
                    let rot_vec_normalized = Vec2::from_angle(rot_angle);
                    let turning_radius = transform.right().truncate()
                        + transform.up().truncate() * movement.rotation_offset * turning_scale;
                    let turning_origin = starting_translation - turning_radius;
                    let result_translation = turning_radius.rotate(rot_vec_normalized)
                        + turning_origin
                        - transform.up().truncate() * movement.rotation_offset;

                    let delta = result_translation - starting_translation;
                    let shape_pos = result_translation;
                    let shape_rot = result_rotation.to_euler(EulerRot::XYZ).2;
                    let max_toi = 1.0;
                    let filter = QueryFilter::default()
                        .exclude_collider(entity)
                        .exclude_sensors();
                    if rapier_context
                        .cast_shape(shape_pos, shape_rot, delta, collider, max_toi, filter)
                        .is_none()
                    {
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
    mut units: Query<
        (
            &mut UnitProgram,
            Option<&mut Movement>,
            &mut UnitClock,
            &Transform,
        ),
        With<Unit>,
    >,
    game_clock: Res<GameClock>,
) {
    for (mut unit_program, mut movement, clock, transform) in units.iter_mut() {
        let handle = UnitHandle {
            movement: movement.as_deref_mut(),
            transform,
            clock: &clock,
            game_clock: &game_clock,
        };
        unit_program.tick(handle)
    }
}

fn tick_units_clocks(mut units: Query<&mut UnitClock, With<Unit>>, time: Res<Time>) {
    units.iter_mut().for_each(|mut unit| {
        unit.0.tick(time.delta());
    })
}

fn game_clock_tick(mut clock: ResMut<GameClock>, time: Res<Time>) {
    clock.0.tick(time.delta());
}

fn print_units_positions(units: Query<&Transform, With<Unit>>) {
    for (i, unit) in units.iter().enumerate() {
        println!(
            "Unit #{}: x {}, y {}",
            i, unit.translation.x, unit.translation.y
        )
    }
}

fn load_assets(mut commands: Commands, assets: Res<AssetServer>) {
    let unit_sprite = assets.load("unit.png");
    commands.insert_resource(UnitSprite(unit_sprite));
    let wall_sprite = assets.load("wall.png");
    commands.insert_resource(WallSprite(wall_sprite));
    let prototypes = assets.load("prototypes.json");
    commands.insert_resource(PrototypesHandle(prototypes))
}

fn check_load_assets(
    mut state: ResMut<State<AppState>>,
    unit: Res<UnitSprite>,
    wall: Res<WallSprite>,
    prototypes: Res<PrototypesHandle>,
    asset_server: Res<AssetServer>,
) {
    if let LoadState::Loaded =
        asset_server.get_group_load_state([unit.0.id, wall.0.id, prototypes.0.id])
    {
        state.set(AppState::Playing).unwrap();
    }
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
        .add_asset::<Prototypes>()
        .init_asset_loader::<PrototypesLoader>()
        .add_state(AppState::Loading)
        .insert_resource(GameClock(Stopwatch::default()))
        .add_system_set(SystemSet::on_enter(AppState::Loading).with_system(load_assets))
        .add_system_set(SystemSet::on_update(AppState::Loading).with_system(check_load_assets))
        .add_system_set(
            SystemSet::on_enter(AppState::Playing)
                .with_system(spawn_walls)
                .with_system(spawn_unit)
                .with_system(spawn_camera),
        )
        .add_system_set(
            SystemSet::on_update(AppState::Playing)
                .with_system(print_units_positions)
                .with_system(game_clock_tick)
                .with_system(handle_movement)
                .with_system(move_and_zoom_camera),
        )
        .add_system_to_stage(CoreStage::First, tick_units_clocks)
        .add_system_to_stage(CoreStage::PreUpdate, unit_tick);

    #[cfg(feature = "debug")]
    app.add_plugin(RapierDebugRenderPlugin::default());
    app.run()
}
