//! Team catalogue — static team metadata loaded from YAML.
//!
//! [`TeamCatalog`] holds the list of teams the game can spawn: playable teams,
//! AI-only factions and hostile "against everyone" races. It is the single
//! source of truth for team names, colours and logos.
//!
//! # Usage
//!
//! The binary should call [`init_team_catalog`](crate::config::init_team_catalog)
//! once at start-up (e.g. in `main`), passing the contents of `assets/teams.yaml`.

use serde::{Deserialize, Serialize};

use crate::config::tile_config::parse_hex_color;
use crate::error::EngineError;
use crate::hero::TeamId;
// ─── TeamKind ───────────────────────────────────────────────────────────────

/// What role a catalogue team plays in a game session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TeamKind {
    /// The player may choose to control this team (Human or CPU). Several
    /// playable teams may be Human simultaneously (hot-seat).
    Playable,
    /// Equivalent AI-only team, never selectable by the player.
    Faction,
    /// "Against everyone" non-playable team (goblins, undead, …).
    Race,
}

// ─── TeamLogo ───────────────────────────────────────────────────────────────

/// A team logo, either a sprite from the tile atlas or a 16x16 bitmap.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TeamLogo {
    /// Atlas sprite index in the tile sheet (`1_main.png`).
    Tile(u32),
    /// 256-bit / 16x16 monochrome bitmap (row-major, bit `y*16 + x`, 1 = filled).
    Bitmap(Box<[u8; 32]>),
}

impl TeamLogo {
    /// For a bitmap logo, returns whether the pixel at `(x, y)` (0..16) is set.
    /// Always `false` for tile logos or out-of-range coordinates.
    pub fn pixel(&self, x: u32, y: u32) -> bool {
        match self {
            TeamLogo::Tile(_) => false,
            TeamLogo::Bitmap(bits) => {
                if x >= 16 || y >= 16 {
                    return false;
                }
                let i = (y * 16 + x) as usize;
                let byte = bits[i / 8];
                (byte & (0x80 >> (i % 8))) != 0
            }
        }
    }
}

// ─── TeamDef / TeamCatalog ────────────────────────────────────────────────────

/// A single catalogue team definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamDef {
    pub(crate) id: TeamId,
    /// Display name; also the stable key used to resolve the logo at render time.
    pub(crate) name: String,
    /// Team colour as an RGB triple.
    pub(crate) color: (u8, u8, u8),
    /// Team role (playable / faction / race).
    pub(crate) kind: TeamKind,
    /// Team logo (atlas tile or bitmap).
    pub(crate) logo: TeamLogo,
}

impl TeamDef {
    pub fn get_id(&self) -> &TeamId {
        &self.id
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_color(&self) -> (u8, u8, u8) {
        self.color
    }

    pub fn get_kind(&self) -> &TeamKind {
        &self.kind
    }

    pub fn get_logo(&self) -> &TeamLogo {
        &self.logo
    }
}

/// The full team catalogue loaded from YAML, preserving file order.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamCatalog {
    teams: Vec<TeamDef>,
}

impl TeamCatalog {
    /// Parses a [`TeamCatalog`] from a YAML string.
    ///
    /// # Errors
    /// Returns [`EngineError::InvalidTiles`] if the YAML is malformed or a
    /// colour / logo field is invalid.
    pub fn from_yaml(yaml: &str) -> Result<Self, EngineError> {
        let raw: serde_yaml::Value = serde_yaml::from_str(yaml)
            .map_err(|e| EngineError::InvalidTiles(format!("failed to parse teams YAML: {e}")))?;
        let mapping = raw
            .get("teams")
            .and_then(|v| v.as_mapping())
            .ok_or_else(|| EngineError::InvalidTiles("teams YAML missing `teams` map".into()))?;

        let mut teams = Vec::with_capacity(mapping.len());
        for (key, value) in mapping {
            let key = key.as_str().unwrap_or("<non-string key>");
            let raw_def: RawTeamDef = serde_yaml::from_value(value.clone())
                .map_err(|e| EngineError::InvalidTiles(format!("invalid team `{key}`: {e}")))?;
            teams.push(raw_def.into_team_def(key)?);
        }
        Ok(Self { teams })
    }

    /// All teams in catalogue order.
    pub fn all(&self) -> &[TeamDef] {
        &self.teams
    }

    /// Teams the player may select (Human or CPU), in catalogue order.
    pub fn playable(&self) -> Vec<&TeamDef> {
        self.teams.iter().filter(|t| t.kind == TeamKind::Playable).collect()
    }

    /// AI-only factions, in catalogue order.
    pub fn factions(&self) -> Vec<&TeamDef> {
        self.teams.iter().filter(|t| t.kind == TeamKind::Faction).collect()
    }

    /// Hostile "against everyone" races, in catalogue order.
    pub fn races(&self) -> Vec<&TeamDef> {
        self.teams.iter().filter(|t| t.kind == TeamKind::Race).collect()
    }

    /// Looks up a team by its (unique) display name.
    pub fn by_name(&self, name: &str) -> Option<&TeamDef> {
        self.teams.iter().find(|t| t.name == name)
    }

    pub fn by_id(&self, id: &TeamId) -> Option<&TeamDef> {
        self.teams.iter().find(|t| t.id.eq(id))
    }
}

// ─── Raw deserialization helpers ──────────────────────────────────────────────

#[derive(Deserialize)]
struct RawTeamDef {
    id: TeamId,
    name: String,
    color: String,
    kind: TeamKind,
    logo: RawLogo,
}

#[derive(Deserialize)]
struct RawLogo {
    #[serde(default)]
    tile: Option<u32>,
    #[serde(default)]
    bitmap: Option<String>,
}

impl RawTeamDef {
    fn into_team_def(self, key: &str) -> Result<TeamDef, EngineError> {
        let color = parse_hex_color(&self.color).ok_or_else(|| {
            EngineError::InvalidTiles(format!("team `{key}`: invalid colour `{}`", self.color))
        })?;
        let logo = match (self.logo.tile, self.logo.bitmap) {
            (Some(index), None) => TeamLogo::Tile(index),
            (None, Some(hex)) => TeamLogo::Bitmap(
                parse_bitmap(&hex)
                    .map_err(|e| EngineError::InvalidTiles(format!("team `{key}`: {e}")))?,
            ),
            (Some(_), Some(_)) => {
                return Err(EngineError::InvalidTiles(format!(
                    "team `{key}`: logo has both `tile` and `bitmap`"
                )));
            }
            (None, None) => {
                return Err(EngineError::InvalidTiles(format!(
                    "team `{key}`: logo has neither `tile` nor `bitmap`"
                )));
            }
        };
        Ok(TeamDef { id: self.id, name: self.name, color, kind: self.kind, logo })
    }
}

/// Parses a 64-character hex string into 32 bytes (256 bits, MSB-first).
fn parse_bitmap(hex: &str) -> Result<Box<[u8; 32]>, String> {
    let hex = hex.trim();
    if hex.len() != 64 {
        return Err(format!("bitmap must be 64 hex chars, got {}", hex.len()));
    }
    let mut bytes = [0u8; 32];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
            .map_err(|_| format!("bitmap has invalid hex at byte {i}"))?;
    }
    Ok(Box::new(bytes))
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = r##"
teams:
  red:
    name: "Red"
    color: "#dc3232"
    kind: playable
    logo:
      tile: 1078
  empire:
    name: "Empire"
    color: "#c8c8d2"
    kind: faction
    logo:
      tile: 1086
  goblins:
    name: "Goblins"
    color: "#5a8a3c"
    kind: race
    logo:
      bitmap: "8000000000000000000000000000000000000000000000000000000000000001"
"##;

    #[test]
    fn parses_catalog_in_order() {
        let cat = TeamCatalog::from_yaml(YAML).unwrap();
        assert_eq!(cat.all().len(), 3);
        assert_eq!(cat.all()[0].name, "Red");
        assert_eq!(cat.all()[1].name, "Empire");
        assert_eq!(cat.all()[2].name, "Goblins");
    }

    #[test]
    fn filters_by_kind() {
        let cat = TeamCatalog::from_yaml(YAML).unwrap();
        assert_eq!(cat.playable().len(), 1);
        assert_eq!(cat.factions().len(), 1);
        assert_eq!(cat.races().len(), 1);
        assert_eq!(cat.playable()[0].name, "Red");
    }

    #[test]
    fn parses_color() {
        let cat = TeamCatalog::from_yaml(YAML).unwrap();
        assert_eq!(cat.by_name("Red").unwrap().color, (0xdc, 0x32, 0x32));
    }

    #[test]
    fn bitmap_pixels_msb_first() {
        let cat = TeamCatalog::from_yaml(YAML).unwrap();
        let logo = &cat.by_name("Goblins").unwrap().logo;
        // First hex byte 0x80 → bit 0 set → pixel (0,0).
        assert!(logo.pixel(0, 0));
        assert!(!logo.pixel(1, 0));
        // Last hex byte 0x01 → bit 255 set → pixel (15,15).
        assert!(logo.pixel(15, 15));
        assert!(!logo.pixel(14, 15));
    }

    #[test]
    fn tile_logo_has_no_pixels() {
        let cat = TeamCatalog::from_yaml(YAML).unwrap();
        assert!(matches!(cat.by_name("Red").unwrap().logo, TeamLogo::Tile(1078)));
        assert!(!cat.by_name("Red").unwrap().logo.pixel(0, 0));
    }

    #[test]
    fn shipped_catalog_parses() {
        // Guards the real assets/teams.yaml shipped with the game so a malformed
        // entry fails the test suite instead of panicking at start-up.
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/teams.yaml");
        let yaml = std::fs::read_to_string(path).expect("read assets/teams.yaml");
        let cat = TeamCatalog::from_yaml(&yaml).expect("parse assets/teams.yaml");
        assert_eq!(cat.playable().len(), 8);
        assert_eq!(cat.factions().len(), 4);
        assert!(!cat.races().is_empty());
    }

    #[test]
    fn rejects_dual_logo() {
        let bad = r##"
teams:
  x:
    name: "X"
    color: "#ffffff"
    kind: race
    logo:
      tile: 1
      bitmap: "8000000000000000000000000000000000000000000000000000000000000001"
"##;
        assert!(TeamCatalog::from_yaml(bad).is_err());
    }
}
