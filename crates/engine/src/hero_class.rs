//! Hero class definitions — determines stats, sprite, and identity.

use serde::{Deserialize, Serialize};

/// All playable hero classes.
///
/// Each variant maps to a unique sprite in the tile atlas and defines base stats.
/// The discriminant values are stable — they correspond to `class_id` in `heroes.yaml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum HeroClass {
    Knight = 0,
    Paladin = 1,
    Guardian = 2,
    Hoplite = 3,
    Warden = 4,
    Templar = 5,
    Shieldbearer = 6,
    Sentinel = 7,
    Warrior = 8,
    Berserker = 9,
    Samurai = 10,
    Gladiator = 11,
    Rogue = 12,
    Assassin = 13,
    Ranger = 14,
    Scout = 15,
    Mage = 16,
    Necromancer = 17,
    Healer = 18,
    Priest = 19,
    Enchanter = 20,
    Alchemist = 21,
    Druid = 22,
    Sorcerer = 23,
}

impl HeroClass {
    /// Returns all hero classes in order.
    pub fn all() -> &'static [HeroClass] {
        &[
            HeroClass::Knight,
            HeroClass::Paladin,
            HeroClass::Guardian,
            HeroClass::Hoplite,
            HeroClass::Warden,
            HeroClass::Templar,
            HeroClass::Shieldbearer,
            HeroClass::Sentinel,
            HeroClass::Warrior,
            HeroClass::Berserker,
            HeroClass::Samurai,
            HeroClass::Gladiator,
            HeroClass::Rogue,
            HeroClass::Assassin,
            HeroClass::Ranger,
            HeroClass::Scout,
            HeroClass::Mage,
            HeroClass::Necromancer,
            HeroClass::Healer,
            HeroClass::Priest,
            HeroClass::Enchanter,
            HeroClass::Alchemist,
            HeroClass::Druid,
            HeroClass::Sorcerer,
        ]
    }

    /// Returns the atlas index for this hero class's sprite in the tile sheet.
    ///
    /// The atlas uses 49-column rows:
    /// - Row 0 (atlas 24-31): tanks
    /// - Row 1 (atlas 73-80): fighters
    /// - Row 2 (atlas 122-129): specialists
    pub fn atlas_index(self) -> usize {
        match self {
            HeroClass::Knight => 24,
            HeroClass::Paladin => 25,
            HeroClass::Guardian => 26,
            HeroClass::Hoplite => 27,
            HeroClass::Warden => 28,
            HeroClass::Templar => 29,
            HeroClass::Shieldbearer => 30,
            HeroClass::Sentinel => 31,
            HeroClass::Warrior => 73,
            HeroClass::Berserker => 74,
            HeroClass::Samurai => 75,
            HeroClass::Gladiator => 76,
            HeroClass::Rogue => 77,
            HeroClass::Assassin => 78,
            HeroClass::Ranger => 79,
            HeroClass::Scout => 80,
            HeroClass::Mage => 122,
            HeroClass::Necromancer => 123,
            HeroClass::Healer => 124,
            HeroClass::Priest => 125,
            HeroClass::Enchanter => 126,
            HeroClass::Alchemist => 127,
            HeroClass::Druid => 128,
            HeroClass::Sorcerer => 129,
        }
    }

    /// Display name for this hero class.
    pub fn display_name(self) -> &'static str {
        match self {
            HeroClass::Knight => "Knight",
            HeroClass::Paladin => "Paladin",
            HeroClass::Guardian => "Guardian",
            HeroClass::Hoplite => "Hoplite",
            HeroClass::Warden => "Warden",
            HeroClass::Templar => "Templar",
            HeroClass::Shieldbearer => "Shieldbearer",
            HeroClass::Sentinel => "Sentinel",
            HeroClass::Warrior => "Warrior",
            HeroClass::Berserker => "Berserker",
            HeroClass::Samurai => "Samurai",
            HeroClass::Gladiator => "Gladiator",
            HeroClass::Rogue => "Rogue",
            HeroClass::Assassin => "Assassin",
            HeroClass::Ranger => "Ranger",
            HeroClass::Scout => "Scout",
            HeroClass::Mage => "Mage",
            HeroClass::Necromancer => "Necromancer",
            HeroClass::Healer => "Healer",
            HeroClass::Priest => "Priest",
            HeroClass::Enchanter => "Enchanter",
            HeroClass::Alchemist => "Alchemist",
            HeroClass::Druid => "Druid",
            HeroClass::Sorcerer => "Sorcerer",
        }
    }

    /// Flavour description for this hero class.
    pub fn description(self) -> &'static str {
        match self {
            HeroClass::Knight => "Stalwart defender with heavy armour",
            HeroClass::Paladin => "Holy warrior, toughest in the realm",
            HeroClass::Guardian => "Immovable shield of the realm",
            HeroClass::Hoplite => "Disciplined phalanx fighter",
            HeroClass::Warden => "Nature's unbreakable protector",
            HeroClass::Templar => "Faith and steel in equal measure",
            HeroClass::Shieldbearer => "Wall of iron, will of stone",
            HeroClass::Sentinel => "Vigilant watchman, ever ready",
            HeroClass::Warrior => "Battle-hardened berserker",
            HeroClass::Berserker => "Rage-fuelled devastation",
            HeroClass::Samurai => "Blade strikes with deadly precision",
            HeroClass::Gladiator => "Arena champion, crowd favourite",
            HeroClass::Rogue => "Swift shadow, deadly backstab",
            HeroClass::Assassin => "One strike, one kill",
            HeroClass::Ranger => "Wilderness hunter, bow master",
            HeroClass::Scout => "Eyes everywhere, fastest on the field",
            HeroClass::Mage => "Arcane devastator, glass cannon",
            HeroClass::Necromancer => "Commands the dead, drains the living",
            HeroClass::Healer => "Mends wounds, sustains allies",
            HeroClass::Priest => "Divine blessing, purifying light",
            HeroClass::Enchanter => "Buffs allies, hexes foes",
            HeroClass::Alchemist => "Potions and explosions in equal measure",
            HeroClass::Druid => "Nature's wrath made manifest",
            HeroClass::Sorcerer => "Raw elemental power incarnate",
        }
    }

    /// Base HP for this hero class.
    pub fn base_hp(self) -> u32 {
        match self {
            HeroClass::Knight => 120,
            HeroClass::Paladin => 130,
            HeroClass::Guardian => 140,
            HeroClass::Hoplite => 110,
            HeroClass::Warden => 125,
            HeroClass::Templar => 115,
            HeroClass::Shieldbearer => 135,
            HeroClass::Sentinel => 120,
            HeroClass::Warrior => 100,
            HeroClass::Berserker => 90,
            HeroClass::Samurai => 95,
            HeroClass::Gladiator => 105,
            HeroClass::Rogue => 80,
            HeroClass::Assassin => 75,
            HeroClass::Ranger => 85,
            HeroClass::Scout => 78,
            HeroClass::Mage => 70,
            HeroClass::Necromancer => 65,
            HeroClass::Healer => 85,
            HeroClass::Priest => 90,
            HeroClass::Enchanter => 75,
            HeroClass::Alchemist => 80,
            HeroClass::Druid => 88,
            HeroClass::Sorcerer => 68,
        }
    }

    /// Base attack for this hero class.
    pub fn base_atk(self) -> u32 {
        match self {
            HeroClass::Knight => 15,
            HeroClass::Paladin => 12,
            HeroClass::Guardian => 10,
            HeroClass::Hoplite => 18,
            HeroClass::Warden => 14,
            HeroClass::Templar => 20,
            HeroClass::Shieldbearer => 8,
            HeroClass::Sentinel => 16,
            HeroClass::Warrior => 28,
            HeroClass::Berserker => 32,
            HeroClass::Samurai => 30,
            HeroClass::Gladiator => 26,
            HeroClass::Rogue => 24,
            HeroClass::Assassin => 28,
            HeroClass::Ranger => 20,
            HeroClass::Scout => 18,
            HeroClass::Mage => 30,
            HeroClass::Necromancer => 28,
            HeroClass::Healer => 8,
            HeroClass::Priest => 10,
            HeroClass::Enchanter => 22,
            HeroClass::Alchemist => 18,
            HeroClass::Druid => 14,
            HeroClass::Sorcerer => 34,
        }
    }

    /// Base defence for this hero class.
    pub fn base_def(self) -> u32 {
        match self {
            HeroClass::Knight => 25,
            HeroClass::Paladin => 28,
            HeroClass::Guardian => 30,
            HeroClass::Hoplite => 22,
            HeroClass::Warden => 26,
            HeroClass::Templar => 20,
            HeroClass::Shieldbearer => 32,
            HeroClass::Sentinel => 24,
            HeroClass::Warrior => 12,
            HeroClass::Berserker => 8,
            HeroClass::Samurai => 10,
            HeroClass::Gladiator => 14,
            HeroClass::Rogue => 6,
            HeroClass::Assassin => 4,
            HeroClass::Ranger => 10,
            HeroClass::Scout => 8,
            HeroClass::Mage => 6,
            HeroClass::Necromancer => 4,
            HeroClass::Healer => 14,
            HeroClass::Priest => 16,
            HeroClass::Enchanter => 8,
            HeroClass::Alchemist => 10,
            HeroClass::Druid => 18,
            HeroClass::Sorcerer => 2,
        }
    }

    /// Base speed for this hero class (movement = 20 + spd).
    pub fn base_spd(self) -> u32 {
        match self {
            HeroClass::Knight => 8,
            HeroClass::Paladin => 6,
            HeroClass::Guardian => 5,
            HeroClass::Hoplite => 10,
            HeroClass::Warden => 7,
            HeroClass::Templar => 11,
            HeroClass::Shieldbearer => 4,
            HeroClass::Sentinel => 9,
            HeroClass::Warrior => 12,
            HeroClass::Berserker => 14,
            HeroClass::Samurai => 13,
            HeroClass::Gladiator => 11,
            HeroClass::Rogue => 18,
            HeroClass::Assassin => 20,
            HeroClass::Ranger => 16,
            HeroClass::Scout => 22,
            HeroClass::Mage => 12,
            HeroClass::Necromancer => 14,
            HeroClass::Healer => 10,
            HeroClass::Priest => 8,
            HeroClass::Enchanter => 15,
            HeroClass::Alchemist => 13,
            HeroClass::Druid => 9,
            HeroClass::Sorcerer => 11,
        }
    }
}
