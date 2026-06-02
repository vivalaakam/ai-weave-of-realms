use serde::{Deserialize, Serialize};

/// A single hero-class entry, narrowed to the fields the engine needs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeroCandidate {
    pub(crate) class_id: u32,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) atlas_index: usize,
    pub(crate) cost: u32,
    pub(crate) hp: u32,
    pub(crate) atk: u32,
    pub(crate) def: u32,
    pub(crate) spd: u32,
}

impl HeroCandidate {
    pub fn get_cost(&self) -> u32 {
        self.cost
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_class_id(&self) -> u32 {
        self.class_id
    }

    pub fn get_hp(&self) -> u32 {
        self.hp
    }

    pub fn get_atk(&self) -> u32 {
        self.atk
    }

    pub fn get_def(&self) -> u32 {
        self.def
    }

    pub fn get_spd(&self) -> u32 {
        self.spd
    }

    pub fn get_mov(&self) -> u32 {
        self.spd + 20
    }

    pub fn get_atlas_index(&self) -> usize {
        self.atlas_index
    }
}
