//! Enums for representing data stored in data storages. Takes inspiration from mlua's Value.

use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use mlua::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum DataValue {
    Nil,
    Boolean(bool),
    Integer(LuaInteger),
    Number(LuaNumber),
    String(String),
    Sequence(Vec<DataValue>),
    Table(HashMap<DataValueHashEq, DataValue>)
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
            DataValueHashEq::Sequence(sq) => Self::Sequence(sq.into_iter().map(Into::into).collect())
        }
    }
}
