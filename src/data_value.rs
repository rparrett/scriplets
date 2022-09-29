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
