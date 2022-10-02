//! Implements loader for a custom asset type.

use bevy::{
    asset::{AssetLoader, LoadContext, LoadedAsset},
    prelude::*,
    reflect::TypeUuid,
    utils::{BoxedFuture, HashMap},
};
use blake3::Hash;
use scriplets_derive::{ComponentPrototype, Prototype};
use serde::{Deserialize, Deserializer};
use strum::AsRefStr;

pub trait Prototype<'de>: Deserialize<'de> {
    fn name(&self) -> &str;
    fn from_pt<'a, 'b>(prototypes_table: &'a Prototypes, name: &'b str) -> Option<&'a Self>;
}

pub trait ComponentPrototype<'de, T: Component = Self>: Prototype<'de> {
    fn to_component(&self) -> T;
    fn component_from_pt(prototypes_table: &Prototypes, name: &str) -> Option<T> {
        Self::from_pt(prototypes_table, name).map(Self::to_component)
    }
}

// TODO: reimplement acceleration movement type to support steering around a point
//  Or make a new movement type which works as stated above
#[derive(Component, Prototype, ComponentPrototype, Deserialize, Clone)]
#[prot_category(movement)]
pub struct Movement {
    pub name: String,
    pub movement_type: MovementType,
    // movement characteristics
    #[serde(default)]
    pub speed: f32, // tiles / second
    #[serde(default)]
    pub max_speed: f32,
    #[serde(default)]
    pub max_speed_backwards: Option<f32>,
    #[serde(default)]
    pub acceleration: f32, // tiles / second^2
    #[serde(default)]
    pub braking_acceleration: Option<f32>,
    #[serde(default)]
    pub passive_deceleration: f32,
    #[serde(default)]
    pub rotation_speed: f32, // degrees / second
    #[serde(default)]
    pub rotation_offset: f32,
    // input
    #[serde(skip)]
    pub input_move: Vec2,
    #[serde(skip)]
    pub input_rotation: f32,
    #[serde(skip)]
    pub hand_brake: bool,
}

#[derive(Deserialize, Clone, AsRefStr)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum MovementType {
    Omnidirectional,
    AcceleratedSteering,
    Train,
}

#[derive(Deserialize, TypeUuid)]
#[uuid = "a5034e09-33ec-4127-ad1e-36fe280e817a"]
pub struct Prototypes {
    #[serde(skip)]
    pub hash: Option<Hash>,
    #[serde(deserialize_with = "hashmap_from_sequence")]
    pub movement: HashMap<String, Movement>,
}

pub fn hashmap_from_sequence<'de, D: Deserializer<'de>, P: Prototype<'de>>(
    deserializer: D,
) -> Result<HashMap<String, P>, D::Error> {
    Ok(Vec::<P>::deserialize(deserializer)?
        .into_iter()
        .map(|p| (p.name().to_string(), p))
        .collect())
}

#[derive(Default)]
pub struct PrototypesLoader;

impl AssetLoader for PrototypesLoader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<(), bevy::asset::Error>> {
        Box::pin(async move {
            let hash = blake3::hash(&bytes);
            let mut prototypes: Prototypes = serde_json::from_slice(&bytes).unwrap();
            prototypes.hash = Some(hash);

            load_context.set_default_asset(LoadedAsset::new(prototypes));
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["json"]
    }
}
