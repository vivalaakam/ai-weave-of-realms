use crate::error::EngineError;
use crate::game_state::GameState;
use crate::hero::{Hero, HeroId, TeamId};
use crate::hero_class::HeroClass;
use crate::map::game_map::{GameMap, MapCoord, ResourceKind, ResourceNode, RESOURCE_KIND_COUNT};
use crate::map::tile::Tiles;
use crate::rng::SeededRng;
use crate::score::{ScoreBoard, ScoreEvent};
use crate::team::Team;
use alloc::collections::{BTreeMap, VecDeque};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

const SAVE_MAGIC: [u8; 4] = *b"RPGS";
const SAVE_VERSION: u16 = 5;

pub(crate) fn to_save_bytes(
    map: &GameMap,
    heroes: &BTreeMap<HeroId, Hero>,
    score: &ScoreBoard,
    city_owners: &BTreeMap<MapCoord, TeamId>,
    resource_owners: &BTreeMap<MapCoord, TeamId>,
    land_owners: &BTreeMap<MapCoord, TeamId>,
    resource_rods: &BTreeMap<MapCoord, TeamId>,
    teams: &BTreeMap<TeamId, Team>,
    teams_order: &VecDeque<TeamId>,
    active_hero: &BTreeMap<TeamId, Option<HeroId>>,
    rng: &SeededRng,
    hero_pointer: HeroId,
) -> Result<Vec<u8>, EngineError> {
    to_save_bytes_with_name(
        map,
        heroes,
        score,
        city_owners,
        resource_owners,
        land_owners,
        resource_rods,
        teams,
        teams_order,
        active_hero,
        rng,
        hero_pointer,
        "",
    )
}

pub(crate) fn to_save_bytes_with_name(
    map: &GameMap,
    heroes: &BTreeMap<HeroId, Hero>,
    score: &ScoreBoard,
    city_owners: &BTreeMap<MapCoord, TeamId>,
    resource_owners: &BTreeMap<MapCoord, TeamId>,
    land_owners: &BTreeMap<MapCoord, TeamId>,
    resource_rods: &BTreeMap<MapCoord, TeamId>,
    teams: &BTreeMap<TeamId, Team>,
    teams_order: &VecDeque<TeamId>,
    active_hero: &BTreeMap<TeamId, Option<HeroId>>,
    rng: &SeededRng,
    hero_pointer: HeroId,
    name: &str,
) -> Result<Vec<u8>, EngineError> {
    let mut writer = SaveWriter::new();
    writer.push_bytes(&SAVE_MAGIC);
    writer.push_u16(SAVE_VERSION);
    writer.push_u16(0);
    writer.push_string(name)?;

    writer.push_u32(map.tile_width());
    writer.push_u32(map.tile_height());
    writer.push_bytes(&map.seed);

    let tile_count = to_u32(map.tiles().len(), "tiles")?;
    writer.push_u32(tile_count);
    for tile in map.tiles() {
        writer.push_u8(tile.kind.base_tile_id() as u8);
    }

    let enemy_count = to_u32(map.enemy_spawns().len(), "enemy spawns")?;
    writer.push_u32(enemy_count);
    for coord in map.enemy_spawns() {
        writer.push_u32(coord.x);
        writer.push_u32(coord.y);
    }

    let chest_count = to_u32(map.chest_spawns().len(), "chest spawns")?;
    writer.push_u32(chest_count);
    for coord in map.chest_spawns() {
        writer.push_u32(coord.x);
        writer.push_u32(coord.y);
    }

    let resource_count = to_u32(map.resource_nodes().len(), "resource nodes")?;
    writer.push_u32(resource_count);
    for node in map.resource_nodes() {
        writer.push_u32(node.coord.x);
        writer.push_u32(node.coord.y);
        writer.push_u8(node.kind.to_id());
    }

    let team_count = to_u32(teams.len(), "teams")?;
    writer.push_u32(team_count);
    for team in teams.values() {
        writer.push_u8(team.get_id());
        writer.push_string(&team.name)?;
        writer.push_u8(team.color.0);
        writer.push_u8(team.color.1);
        writer.push_u8(team.color.2);
        writer.push_u8(if team.is_player_controlled() { 1 } else { 0 });
        writer.push_u32(team.get_turn());
        // Treasury (save version 5+).
        writer.push_u32(team.gold());
        for amount in team.resources() {
            writer.push_u32(amount);
        }
    }

    let order_count = to_u32(teams_order.len(), "teams order")?;
    writer.push_u32(order_count);
    for team_id in teams_order.iter() {
        writer.push_u8(*team_id);
    }

    let active_count = to_u32(active_hero.len(), "active heroes")?;
    writer.push_u32(active_count);
    for (team_id, hero_id) in active_hero.iter() {
        writer.push_u8(*team_id);
        match hero_id {
            Some(id) => {
                writer.push_u8(1);
                writer.push_u8(*id);
            }
            None => writer.push_u8(0),
        }
    }

    let hero_count = to_u32(heroes.len(), "heroes")?;
    writer.push_u32(hero_count);
    for (hero_id, hero) in heroes.iter() {
        writer.push_u8(*hero_id);
        writer.push_string(&hero.name)?;
        writer.push_u32(hero.hp);
        writer.push_u32(hero.max_hp);
        writer.push_u32(hero.atk);
        writer.push_u32(hero.def);
        writer.push_u32(hero.spd);
        writer.push_u32(hero.mov);
        writer.push_u32(hero.mov_remaining);
        writer.push_u32(hero.position.x);
        writer.push_u32(hero.position.y);
        writer.push_u8(hero.team_id);
        writer.push_bytes(&hero.rng.state());
        writer.push_u8(hero.rng.position());
    }

    let score_count = to_u32(score.events().len(), "score events")?;
    writer.push_u32(score_count);
    for (event, _) in score.events() {
        write_score_event(&mut writer, event)?;
    }

    let city_count = to_u32(city_owners.len(), "city owners")?;
    writer.push_u32(city_count);
    for (coord, team_id) in city_owners.iter() {
        writer.push_u32(coord.x);
        writer.push_u32(coord.y);
        writer.push_u8(*team_id);
    }

    let resource_owner_count = to_u32(resource_owners.len(), "resource owners")?;
    writer.push_u32(resource_owner_count);
    for (coord, team_id) in resource_owners.iter() {
        writer.push_u32(coord.x);
        writer.push_u32(coord.y);
        writer.push_u8(*team_id);
    }

    let land_owner_count = to_u32(land_owners.len(), "land owners")?;
    writer.push_u32(land_owner_count);
    for (coord, team_id) in land_owners.iter() {
        writer.push_u32(coord.x);
        writer.push_u32(coord.y);
        writer.push_u8(*team_id);
    }

    let rod_count = to_u32(resource_rods.len(), "resource rods")?;
    writer.push_u32(rod_count);
    for (coord, team_id) in resource_rods.iter() {
        writer.push_u32(coord.x);
        writer.push_u32(coord.y);
        writer.push_u8(*team_id);
    }

    writer.push_u8(hero_pointer);
    writer.push_bytes(&rng.state());
    writer.push_u8(rng.position());

    Ok(writer.finish())
}

pub(crate) fn read_save_name(bytes: &[u8]) -> Result<String, EngineError> {
    let mut reader = SaveReader::new(bytes);
    let magic = reader.read_bytes(4)?;
    if magic != SAVE_MAGIC {
        return Err(save_error("invalid save magic"));
    }

    let version = reader.read_u16()?;
    if version > SAVE_VERSION {
        return Err(save_error(format!("unsupported save version {version}")));
    }
    let _flags = reader.read_u16()?;

    if version >= 2 {
        reader.read_string()
    } else {
        Ok(String::new())
    }
}

pub(crate) fn from_save_bytes(bytes: &[u8]) -> Result<GameState, EngineError> {
    let mut reader = SaveReader::new(bytes);
    let magic = reader.read_bytes(4)?;
    if magic != SAVE_MAGIC {
        return Err(save_error("invalid save magic"));
    }

    let version = reader.read_u16()?;
    if version > SAVE_VERSION {
        return Err(save_error(format!("unsupported save version {version}")));
    }
    let _flags = reader.read_u16()?;
    if version >= 2 {
        let _name = reader.read_string()?;
    }

    let width = reader.read_u32()?;
    let height = reader.read_u32()?;
    if width == 0 || height == 0 {
        return Err(save_error("map dimensions must be non-zero"));
    }

    let seed = reader.read_array_32()?;
    let tile_count = reader.read_u32()? as usize;
    let expected = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| save_error("map size overflow"))?;
    if tile_count != expected {
        return Err(save_error("tile count does not match map dimensions"));
    }

    let mut tiles = Vec::with_capacity(tile_count);
    for _ in 0..tile_count {
        let tile_id = reader.read_u8()?;
        let kind = Tiles::from_id(tile_id as u32).map_err(|_| save_error("invalid tile id"))?;
        tiles.push(crate::map::tile::Tile::new(kind));
    }

    let enemy_count = reader.read_u32()? as usize;
    let mut enemy_spawns = Vec::with_capacity(enemy_count);
    for _ in 0..enemy_count {
        let x = reader.read_u32()?;
        let y = reader.read_u32()?;
        enemy_spawns.push(MapCoord::new(x, y));
    }

    let chest_count = reader.read_u32()? as usize;
    let mut chest_spawns = Vec::with_capacity(chest_count);
    for _ in 0..chest_count {
        let x = reader.read_u32()?;
        let y = reader.read_u32()?;
        chest_spawns.push(MapCoord::new(x, y));
    }

    let mut map = GameMap::new(width, height, tiles, seed)?;
    map.set_spawn_points(enemy_spawns, chest_spawns)?;
    if version >= 3 {
        let resource_count = reader.read_u32()? as usize;
        let mut resource_nodes = Vec::with_capacity(resource_count);
        for _ in 0..resource_count {
            let x = reader.read_u32()?;
            let y = reader.read_u32()?;
            let kind = ResourceKind::from_id(reader.read_u8()?)
                .map_err(|_| save_error("invalid resource kind"))?;
            resource_nodes.push(ResourceNode { coord: MapCoord::new(x, y), kind });
        }
        map.set_resource_nodes(resource_nodes)?;
    }

    let team_count = reader.read_u32()? as usize;
    let mut teams = BTreeMap::new();
    for _ in 0..team_count {
        let id = reader.read_u8()?;
        let name = reader.read_string()?;
        let r = reader.read_u8()?;
        let g = reader.read_u8()?;
        let b = reader.read_u8()?;
        let player_controlled = reader.read_u8()? == 1;
        let turn = reader.read_u32()?;
        let mut team = Team::new(id, name, (r, g, b), player_controlled);
        team.set_turn(turn);
        // Treasury (save version 5+).
        if version >= 5 {
            team.set_gold(reader.read_u32()?);
            for index in 0..RESOURCE_KIND_COUNT {
                team.set_resource(index, reader.read_u32()?);
            }
        }
        teams.insert(id, team);
    }

    let order_count = reader.read_u32()? as usize;
    let mut teams_order = VecDeque::with_capacity(order_count);
    for _ in 0..order_count {
        teams_order.push_back(reader.read_u8()?);
    }

    let active_count = reader.read_u32()? as usize;
    let mut active_hero = BTreeMap::new();
    for _ in 0..active_count {
        let team_id = reader.read_u8()?;
        let has_hero = reader.read_u8()? == 1;
        let hero_id = if has_hero { Some(reader.read_u8()?) } else { None };
        active_hero.insert(team_id, hero_id);
    }

    let hero_count = reader.read_u32()? as usize;
    let mut heroes = BTreeMap::new();
    for _ in 0..hero_count {
        let hero_id = reader.read_u8()?;
        let name = reader.read_string()?;
        let hp = reader.read_u32()?;
        let max_hp = reader.read_u32()?;
        let atk = reader.read_u32()?;
        let def = reader.read_u32()?;
        let spd = reader.read_u32()?;
        let mov = reader.read_u32()?;
        let mov_remaining = reader.read_u32()?;
        let x = reader.read_u32()?;
        let y = reader.read_u32()?;
        let team_id = reader.read_u8()?;
        let rng_state = reader.read_array_32()?;
        let rng_position = reader.read_u8()?;
        if rng_position > 32 {
            return Err(save_error("invalid hero RNG position"));
        }
        let mut hero = Hero::new_with_stats(
            hero_id,
            HeroClass::Knight,
            name,
            hp,
            atk,
            def,
            spd,
            MapCoord::new(x, y),
            team_id,
        );
        hero.max_hp = max_hp;
        hero.mov = mov;
        hero.mov_remaining = mov_remaining;
        hero.rng = SeededRng::from_state_and_position(rng_state, rng_position);
        heroes.insert(hero_id, hero);
    }

    let score_count = reader.read_u32()? as usize;
    let mut score = ScoreBoard::new();
    for _ in 0..score_count {
        let event = read_score_event(&mut reader)?;
        score.record(event);
    }

    let city_count = reader.read_u32()? as usize;
    let mut city_owners = BTreeMap::new();
    for _ in 0..city_count {
        let x = reader.read_u32()?;
        let y = reader.read_u32()?;
        let team_id = reader.read_u8()?;
        city_owners.insert(MapCoord::new(x, y), team_id);
    }

    let mut resource_owners = BTreeMap::new();
    if version >= 3 {
        let resource_owner_count = reader.read_u32()? as usize;
        for _ in 0..resource_owner_count {
            let x = reader.read_u32()?;
            let y = reader.read_u32()?;
            let team_id = reader.read_u8()?;
            resource_owners.insert(MapCoord::new(x, y), team_id);
        }
    }

    let mut land_owners = BTreeMap::new();
    let mut resource_rods = BTreeMap::new();
    if version >= 4 {
        let land_owner_count = reader.read_u32()? as usize;
        for _ in 0..land_owner_count {
            let x = reader.read_u32()?;
            let y = reader.read_u32()?;
            let team_id = reader.read_u8()?;
            land_owners.insert(MapCoord::new(x, y), team_id);
        }

        let rod_count = reader.read_u32()? as usize;
        for _ in 0..rod_count {
            let x = reader.read_u32()?;
            let y = reader.read_u32()?;
            let team_id = reader.read_u8()?;
            resource_rods.insert(MapCoord::new(x, y), team_id);
        }
    }

    let hero_pointer = reader.read_u8()?;
    let rng_state = reader.read_array_32()?;
    let rng_position = reader.read_u8()?;
    if rng_position > 32 {
        return Err(save_error("invalid session RNG position"));
    }

    Ok(GameState::from_parts(
        map,
        heroes,
        score,
        city_owners,
        resource_owners,
        land_owners,
        resource_rods,
        teams,
        teams_order,
        active_hero,
        rng_state,
        rng_position,
        hero_pointer,
    ))
}

struct SaveWriter {
    buffer: Vec<u8>,
}

impl SaveWriter {
    fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    fn finish(self) -> Vec<u8> {
        self.buffer
    }

    fn push_u8(&mut self, value: u8) {
        self.buffer.push(value);
    }

    fn push_u16(&mut self, value: u16) {
        self.buffer.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u32(&mut self, value: u32) {
        self.buffer.extend_from_slice(&value.to_le_bytes());
    }

    fn push_bytes(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    fn push_string(&mut self, value: &str) -> Result<(), EngineError> {
        let bytes = value.as_bytes();
        let len = bytes.len();
        if len > u16::MAX as usize {
            return Err(save_error("string too long"));
        }
        self.push_u16(len as u16);
        self.push_bytes(bytes);
        Ok(())
    }
}

struct SaveReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> SaveReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], EngineError> {
        let end = self.offset.checked_add(len).ok_or_else(|| save_error("read overflow"))?;
        if end > self.bytes.len() {
            return Err(save_error("unexpected end of save data"));
        }
        let slice = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(slice)
    }

    fn read_u8(&mut self) -> Result<u8, EngineError> {
        Ok(self.read_bytes(1)?[0])
    }

    fn read_u16(&mut self) -> Result<u16, EngineError> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32(&mut self) -> Result<u32, EngineError> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_string(&mut self) -> Result<String, EngineError> {
        let len = self.read_u16()? as usize;
        let bytes = self.read_bytes(len)?;
        core::str::from_utf8(bytes)
            .map(|value| value.to_string())
            .map_err(|_| save_error("invalid UTF-8 string"))
    }

    fn read_array_32(&mut self) -> Result<[u8; 32], EngineError> {
        let bytes = self.read_bytes(32)?;
        let mut out = [0u8; 32];
        out.copy_from_slice(bytes);
        Ok(out)
    }
}

fn to_u32(len: usize, label: &'static str) -> Result<u32, EngineError> {
    u32::try_from(len).map_err(|_| save_error(format!("{label} count too large")))
}

fn save_error(message: impl Into<String>) -> EngineError {
    EngineError::Save(message.into())
}

fn write_score_event(writer: &mut SaveWriter, event: &ScoreEvent) -> Result<(), EngineError> {
    match event {
        ScoreEvent::CityCapture { city } => {
            writer.push_u8(0);
            writer.push_u32(city.x);
            writer.push_u32(city.y);
        }
        ScoreEvent::EnemyDefeated { enemy_id } => {
            writer.push_u8(1);
            writer.push_u8(*enemy_id);
        }
        ScoreEvent::ResourceCollected { coord } => {
            writer.push_u8(2);
            writer.push_u32(coord.x);
            writer.push_u32(coord.y);
        }
        ScoreEvent::GoldCollected { coord } => {
            writer.push_u8(3);
            writer.push_u32(coord.x);
            writer.push_u32(coord.y);
        }
        ScoreEvent::TurnSurvived => {
            writer.push_u8(4);
        }
    }
    Ok(())
}

fn read_score_event(reader: &mut SaveReader<'_>) -> Result<ScoreEvent, EngineError> {
    match reader.read_u8()? {
        0 => Ok(ScoreEvent::CityCapture {
            city: MapCoord::new(reader.read_u32()?, reader.read_u32()?),
        }),
        1 => Ok(ScoreEvent::EnemyDefeated { enemy_id: reader.read_u8()? }),
        2 => Ok(ScoreEvent::ResourceCollected {
            coord: MapCoord::new(reader.read_u32()?, reader.read_u32()?),
        }),
        3 => Ok(ScoreEvent::GoldCollected {
            coord: MapCoord::new(reader.read_u32()?, reader.read_u32()?),
        }),
        4 => Ok(ScoreEvent::TurnSurvived),
        _ => Err(save_error("invalid score event id")),
    }
}
