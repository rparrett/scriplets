//! Enums for representing data stored in data storages. Takes inspiration from mlua's Value.

use mlua::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum DataValue {
    Nil,
    Boolean(bool),
    Integer(LuaInteger),
    Number(LuaNumber),
    String(String),
    Sequence(Vec<DataValue>),
    Table(HashMap<DataValueHashEq, DataValue>),
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(untagged)]
pub enum DataValueHashEq {
    Nil,
    Boolean(bool),
    Integer(LuaInteger),
    String(String),
    Sequence(Vec<DataValueHashEq>),
}

impl From<DataValueHashEq> for DataValue {
    fn from(data: DataValueHashEq) -> Self {
        match data {
            DataValueHashEq::Nil => Self::Nil,
            DataValueHashEq::Boolean(b) => Self::Boolean(b),
            DataValueHashEq::Integer(i) => Self::Integer(i),
            DataValueHashEq::String(s) => Self::String(s),
            DataValueHashEq::Sequence(sq) => {
                Self::Sequence(sq.into_iter().map(Into::into).collect())
            }
        }
    }
}

impl<'lua> FromLua<'lua> for DataValue {
    fn from_lua(lua_value: LuaValue<'lua>, _lua: &'lua Lua) -> LuaResult<Self> {
        let type_name = lua_value.type_name();
        match lua_value {
            LuaValue::Nil => Ok(Self::Nil),
            LuaValue::Boolean(b) => Ok(Self::Boolean(b)),
            LuaValue::Integer(i) => Ok(Self::Integer(i)),
            LuaValue::Number(n) => Ok(Self::Number(n)),
            LuaValue::String(s) => Ok(Self::String(s.to_str()?.into())),
            LuaValue::Table(t) => {
                if let Ok(seq) = t
                    .clone()
                    .sequence_values::<DataValue>()
                    .collect::<LuaResult<Vec<DataValue>>>()
                {
                    Ok(Self::Sequence(seq))
                } else {
                    Ok(Self::Table(
                        t.pairs()
                            .collect::<LuaResult<HashMap<DataValueHashEq, DataValue>>>()?,
                    ))
                }
            }
            _ => Err(LuaError::FromLuaConversionError {
                from: type_name,
                to: "DataValue",
                message: Some("type not supported".into()),
            }),
        }
    }
}

impl<'lua> ToLua<'lua> for DataValue {
    fn to_lua(self, lua: &'lua Lua) -> LuaResult<LuaValue<'lua>> {
        match self {
            Self::Nil => Ok(LuaValue::Nil),
            Self::Boolean(b) => Ok(LuaValue::Boolean(b)),
            Self::Integer(i) => Ok(LuaValue::Integer(i)),
            Self::Number(n) => Ok(LuaValue::Number(n)),
            Self::String(s) => s.to_lua(lua),
            Self::Sequence(seq) => seq.to_lua(lua),
            Self::Table(t) => t.to_lua(lua),
        }
    }
}

impl TryFrom<DataValue> for DataValueHashEq {
    type Error = DataValueConversionError;

    fn try_from(value: DataValue) -> Result<Self, Self::Error> {
        match value {
            DataValue::Nil => Ok(Self::Nil),
            DataValue::Boolean(b) => Ok(Self::Boolean(b)),
            DataValue::Integer(i) => Ok(Self::Integer(i)),
            DataValue::Number(n) => Err(Self::Error::Number(n)),
            DataValue::String(s) => Ok(Self::String(s)),
            DataValue::Sequence(sq) => Ok(Self::Sequence(
                sq.into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<Vec<Self>, Self::Error>>()?,
            )),
            DataValue::Table(t) => Err(Self::Error::Table(t)),
        }
    }
}

impl<'lua> FromLua<'lua> for DataValueHashEq {
    fn from_lua(lua_value: LuaValue<'lua>, lua: &'lua Lua) -> LuaResult<Self> {
        DataValue::from_lua(lua_value, lua)?
            .try_into()
            .map_err(LuaError::external)
    }
}

impl<'lua> ToLua<'lua> for DataValueHashEq {
    fn to_lua(self, lua: &'lua Lua) -> LuaResult<LuaValue<'lua>> {
        DataValue::from(self).to_lua(lua)
    }
}

#[derive(Debug, Clone, Error)]
pub enum DataValueConversionError {
    #[error("DataValueHashEq can't contain f64")]
    Number(LuaNumber),
    #[error("DataValueHashEq can't contain HashMap")]
    Table(HashMap<DataValueHashEq, DataValue>),
}
