use mlua::prelude::*;
use bevy::prelude::*;
use super::{Movement, UnitClock, GameClock};
use std::{sync::Mutex, f32::consts::PI};

#[derive(Component)]
pub struct UnitProgram {
    state: UnitProgramState,
    pub program: Box<[u8]>
}

impl UnitProgram {
    pub fn tick(&mut self, handle: UnitHandle<'_>) {
        self.state.tick(handle)
    } 

    pub fn reload(&mut self) {
        self.state.reload(self.program.as_ref())
    }

    pub fn new_lua() -> Self {
        UnitProgram {
            state: UnitProgramState::new_lua(),
            program: Box::new([])
        }
    }

    pub fn new_lua_with_program(program: &[u8]) -> Self {
        UnitProgram {
            state: UnitProgramState::new_lua_with_program(program),
            program: program.into()
        }
    }
}

pub enum UnitProgramState {
    Lua(Mutex<Lua>),
    // wasm TODO
}

impl UnitProgramState {
    pub fn tick(&mut self, handle: UnitHandle<'_>) { // TODO: error handling?
        match self {
            Self::Lua(lua) => {
                let lua = lua.get_mut().unwrap();
                if let Some(on_tick_fn) = lua.globals().get::<_, Option<LuaFunction>>("on_tick").unwrap() {
                    lua.scope(|s| {
                        let lua_handle = s.create_nonstatic_userdata(LuaUnitHandle{handle})?;
                        on_tick_fn.call(lua_handle)?;
                        Ok(())
                    }).unwrap();
                };
            }
        }
    }

    pub fn reload(&mut self, program: &[u8]) {
        *self = self.new_with_program(program);
    }

    pub fn resetted(&mut self) -> Self {
        match self {
            Self::Lua(_) => Self::new_lua()
        }
    }

    pub fn new_lua() -> Self {
        Self::Lua(Mutex::new(Lua::new()))
    }

    pub fn new_with_program(&self, program: &[u8]) -> Self {
        match self {
            Self::Lua(_) => Self::new_lua_with_program(program)
        }
    }

    pub fn new_lua_with_program(program: &[u8]) -> Self {
        let mut result = Self::new_lua();
        match result {
            Self::Lua(ref lua) => {
                let lua = lua.lock().unwrap();
                lua.load(program).exec().unwrap();
            }
        };
        result
    }
}

pub struct UnitHandle<'a> {
    pub movement: Option<&'a mut Movement>,
    pub transform: &'a Transform,
    pub clock: &'a UnitClock,
    pub game_clock: &'a GameClock
}

pub struct LuaUnitHandle<'a> {
    handle: UnitHandle<'a>
}

// TODO: after making a planet map, methods for getting nearest transition tile or a tile adjacent
//  to transition tile
impl LuaUserData for LuaUnitHandle<'_> {
    fn add_methods<'lua, M: LuaUserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method_mut("move", |_lua, lua_handle, args: (f32, f32)| {
            if let Some(movement) = &mut lua_handle.handle.movement {
                movement.input_move = Vec2::from(args);
            };
            Ok(())
        });
        methods.add_method_mut("rotate", |_lua, lua_handle, rot: f32| {
            if let Some(movement) = &mut lua_handle.handle.movement {
                movement.input_rotation = rot;
            }
            Ok(())
        });
        methods.add_method_mut("toggle_hand_brake", |_lua, lua_handle, ()| {
            if let Some(movement) = &mut lua_handle.handle.movement {
                movement.hand_brake = !movement.hand_brake;
            }
            Ok(())
        })
    }

    fn add_fields<'lua, F: LuaUserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field_method_get("time_since_start", |_lua, lua_handle| {
            Ok(lua_handle.handle.clock.0.elapsed_secs())
        });
        fields.add_field_method_get("global_time", |_lua, lua_handle| {
            Ok(lua_handle.handle.game_clock.0.elapsed_secs())
        });
        fields.add_field_method_get("gps", |lua, lua_handle| {
            let position: [f32; 2] = lua_handle.handle.transform.translation.truncate().into();
            let rotation_radians = lua_handle.handle.transform.rotation.to_euler(EulerRot::XYZ).2;
            let rotation_degrees = -(rotation_radians * 180.0) / PI;
            let table = lua.create_table()?;
            table.set("position", position)?;
            table.set("rotation", rotation_degrees)?;
            Ok(table)
        });
        fields.add_field_method_get("movement", |lua, lua_handle| {
            if let Some(movement) = &lua_handle.handle.movement {
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
