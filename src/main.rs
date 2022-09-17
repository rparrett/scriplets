use std::sync::Mutex;
use mlua::prelude::*;
use bevy::{prelude::*, window::PresentMode, render::camera::ScalingMode, input::mouse::{MouseWheel, MouseScrollUnit, MouseMotion}};
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
    speed: f32,
    next_move: Vec2
}

pub struct GameTickTimer(Timer);

pub struct UnitSprite(Handle<Image>);
pub struct WallSprite(Handle<Image>);

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

fn move_and_zoom_camera(
    mut camera: Query<&mut Transform, With<Camera2d>>,
    input: Res<Input<MouseButton>>,
    mut mouse_scroll_evr: EventReader<MouseWheel>,
    mut mouse_move_evr: EventReader<MouseMotion>)
{
    let mut camera = camera.single_mut();
    for scroll_event in mouse_scroll_evr.iter() {
        match scroll_event.unit {
            MouseScrollUnit::Line => camera.scale = (camera.scale - 0.5 * scroll_event.y).clamp_length(1.0, 20.0),
            MouseScrollUnit::Pixel => camera.scale = (camera.scale - 0.1 * scroll_event.y).clamp_length(1.0, 20.0)
        }
    }
    for move_event in mouse_move_evr.iter() {
        if input.pressed(MouseButton::Middle) {
            let mut delta = move_event.delta * 0.0015 * camera.scale.length();
            delta.x = -delta.x;
            camera.translation += delta.extend(0.0);
        }
    }
}

fn spawn_unit(mut commands: Commands, unit_sprite: Res<UnitSprite>) {
    let lua = Lua::new();
    lua.load("function on_tick(handle) handle:move(1, 1) end").exec().unwrap();
    commands.spawn()
        .insert(Unit)
        .insert(Movement{speed:1.0, next_move: Vec2::splat(0.0)})
        .insert(LuaState::new(lua))
        .insert_bundle(TransformBundle::default())
        .insert(Collider::cuboid(0.5, 0.5))
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
    for i in 0..=5 {
        spawn_wall(&mut commands, i as f32, 5.0, &wall_sprite.0)
    }
    for j in 0..=4 {
        spawn_wall(&mut commands, 5.0, j as f32, &wall_sprite.0)
    }
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

fn handle_movement(mut units: Query<(Entity, &mut Movement, &mut Transform, &Collider), With<Unit>>, rapier_context: Res<RapierContext>) {
    for (entity, mut movement, mut transform, collider) in units.iter_mut() {
        if movement.next_move != Vec2::ZERO {
            let delta = movement.next_move.extend(0.0).clamp_length_max(1.0) * (movement.speed / 60.0);
            let shape_pos = transform.translation.truncate();
            let shape_rot = transform.rotation.to_euler(EulerRot::XYZ).2;
            let max_toi = 1.0;
            let filter = QueryFilter::default().exclude_collider(entity);
            if rapier_context.cast_shape(shape_pos, shape_rot, delta.truncate(), collider, max_toi, filter).is_none() {
                transform.translation += delta;
            }
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
        .insert_resource(GameTickTimer(Timer::from_seconds(1.0/60.0, true)))
        .add_startup_system_to_stage(StartupStage::PreStartup, load_sprites)
        .add_startup_system(spawn_walls)
        .add_startup_system(spawn_unit)
        .add_startup_system(spawn_camera)
        .add_system_to_stage(CoreStage::PreUpdate, unit_tick)
        .add_system(handle_movement)
        .add_system(move_and_zoom_camera)
        .run();
}
