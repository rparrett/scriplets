//! Enums for representing data stored in data storages. Takes inspiration from mlua's Value.

use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use mlua::prelude::*;
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

impl TryFrom<DataValue> for DataValueHashEq {
    type Error = DataValueConversionError;

    fn try_from(value: DataValue) -> Result<Self, Self::Error> {
        match value {
            DataValue::Nil => Ok(Self::Nil),
            DataValue::Boolean(b) => Ok(Self::Boolean(b)),
            DataValue::Integer(i) => Ok(Self::Integer(i)),
            DataValue::Number(n) => Err(Self::Error::Number(n)),
            DataValue::String(s) => Ok(Self::String(s)),
            DataValue::Sequence(sq) => Ok(Self::Sequence(sq.into_iter().map(TryInto::try_into).collect::<Result<Vec<Self>, Self::Error>>()?)),
            DataValue::Table(t) => Err(Self::Error::Table(t))
        }
    }
}

#[derive(Debug, Clone, Error)]
pub enum DataValueConversionError {
    #[error("DataValueHashEq can't contain f64")]
    Number(LuaNumber),
    #[error("DataValueHashEq can't contain HashMap")]
    Table(HashMap<DataValueHashEq, DataValue>),
}
