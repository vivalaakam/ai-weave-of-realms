use engine::game_state::ROD_COST;
use engine::hero::{Hero, TeamId};
use engine::map::game_map::Direction;
use engine::map::tile::Tiles;
use engine::movement::{find_path, reachable_tiles};
use engine::{MapCoord};

use crate::actions::AiAction;
use crate::strategy::{AiContext, AiStrategy};

#[derive(Clone, Copy, Debug)]
pub enum AiStrategyKind {
    ResourceRush,
    Explore,
    Wander,
}

pub struct AiFactory {
    default_kind: AiStrategyKind,
}

impl Default for AiFactory {
    fn default() -> Self {
        Self { default_kind: AiStrategyKind::ResourceRush }
    }
}

impl AiFactory {
    pub fn new(default_kind: AiStrategyKind) -> Self {
        Self { default_kind }
    }

    pub fn build(&self, _team_id: TeamId) -> Box<dyn AiStrategy> {
        match self.default_kind {
            AiStrategyKind::ResourceRush => Box::new(ResourceRushStrategy),
            AiStrategyKind::Explore => Box::new(ExploreStrategy),
            AiStrategyKind::Wander => Box::new(WanderStrategy),
        }
    }
}

struct ResourceRushStrategy;
struct ExploreStrategy;
struct WanderStrategy;

impl AiStrategy for ResourceRushStrategy {
    fn name(&self) -> &'static str {
        "ResourceRush"
    }

    fn plan(&mut self, ctx: &AiContext<'_>) -> Vec<AiAction> {
        let resource_targets = resource_targets(ctx);
        let city_targets = city_targets(ctx);
        plan_for_team(ctx, &resource_targets, &city_targets, StrategyFocus::Resources)
    }
}

impl AiStrategy for ExploreStrategy {
    fn name(&self) -> &'static str {
        "Explore"
    }

    fn plan(&mut self, ctx: &AiContext<'_>) -> Vec<AiAction> {
        let resource_targets = resource_targets(ctx);
        let city_targets = city_targets(ctx);
        plan_for_team(ctx, &city_targets, &resource_targets, StrategyFocus::Cities)
    }
}

impl AiStrategy for WanderStrategy {
    fn name(&self) -> &'static str {
        "Wander"
    }

    fn plan(&mut self, ctx: &AiContext<'_>) -> Vec<AiAction> {
        let mut actions = Vec::new();
        let hero_ids = ctx.state.get_team_alive_heroes_ids(ctx.team_id);
        for hero_id in hero_ids {
            let Some(hero) = ctx.state.hero(hero_id) else { continue };
            if let Some(defender_id) = adjacent_enemy(ctx, hero) {
                actions.push(AiAction::Attack { attacker_id: hero_id, defender_id });
                continue;
            }
            actions.extend(fallback_step(ctx, hero));
        }
        append_end_turn(actions)
    }
}

enum StrategyFocus {
    Resources,
    Cities,
}

fn plan_for_team(
    ctx: &AiContext<'_>,
    primary_targets: &[MapCoord],
    secondary_targets: &[MapCoord],
    focus: StrategyFocus,
) -> Vec<AiAction> {
    let mut actions = Vec::new();

    if let Some(hire_action) = plan_hire_hero(ctx, &focus) {
        actions.push(hire_action);
    }

    let hero_ids = ctx.state.get_team_alive_heroes_ids(ctx.team_id);
    for hero_id in hero_ids {
        let Some(hero) = ctx.state.hero(hero_id) else { continue };
        if hero.get_mov_remaining() == 0 {
            continue;
        }

        if let Some(defender_id) = adjacent_enemy(ctx, hero) {
            actions.push(AiAction::Attack { attacker_id: hero_id, defender_id });
            continue;
        }

        if should_place_rod(ctx, hero) {
            actions.push(AiAction::PlaceRod { hero_id });
            continue;
        }

        if let Some(target) = nearest_target(hero.get_position(), primary_targets) {
            if let Some(mut hero_actions) = plan_path_to_target(ctx, hero, target) {
                actions.append(&mut hero_actions);
                continue;
            }
        }

        if let Some(target) = nearest_target(hero.get_position(), secondary_targets) {
            if let Some(mut hero_actions) = plan_path_to_target(ctx, hero, target) {
                actions.append(&mut hero_actions);
                continue;
            }
        }

        actions.extend(fallback_step(ctx, hero));
    }

    append_end_turn(actions)
}

fn append_end_turn(mut actions: Vec<AiAction>) -> Vec<AiAction> {
    actions.push(AiAction::EndTurn);
    actions
}

fn plan_hire_hero(ctx: &AiContext<'_>, focus: &StrategyFocus) -> Option<AiAction> {
    if !ctx.state.get_team_alive_heroes_ids(ctx.team_id).is_empty() {
        return None;
    }
    let team = ctx.state.get_team(ctx.team_id)?;
    let Some(candidate) = ctx.state.get_hero_candidate_at(0) else {
        return None;
    };
    if team.gold() < candidate.get_cost() {
        return None;
    }
    let coord = ctx.state.city_owner_for_team(ctx.team_id)?;
    if ctx.state.hero_at(&coord).is_some() {
        return None;
    }
    let candidate_idx = match focus {
        StrategyFocus::Resources => 0,
        StrategyFocus::Cities => 0,
    };
    Some(AiAction::HireHero { candidate_idx, coord })
}

fn resource_targets(ctx: &AiContext<'_>) -> Vec<MapCoord> {
    let mut targets = Vec::new();
    let map = &ctx.state.map;
    if !map.resource_nodes().is_empty() {
        for node in map.resource_nodes() {
            if ctx.state.resource_owner(node.coord) != Some(ctx.team_id) {
                targets.push(node.coord);
            }
        }
        return targets;
    }

    let w = map.tile_width();
    let h = map.tile_height();
    for y in 0..h {
        for x in 0..w {
            let coord = MapCoord::new(x, y);
            let Ok(tile) = map.get_tile(coord) else { continue };
            if matches!(tile.kind, Tiles::Gold | Tiles::Resource)
                && ctx.state.resource_owner(coord) != Some(ctx.team_id)
            {
                targets.push(coord);
            }
        }
    }
    targets
}

fn city_targets(ctx: &AiContext<'_>) -> Vec<MapCoord> {
    let mut targets = Vec::new();
    let map = &ctx.state.map;
    let w = map.tile_width();
    let h = map.tile_height();
    for y in 0..h {
        for x in 0..w {
            let coord = MapCoord::new(x, y);
            let Ok(tile) = map.get_tile(coord) else { continue };
            if matches!(tile.kind, Tiles::City | Tiles::CityEntrance)
                && ctx.state.city_owner(&coord) != Some(ctx.team_id)
            {
                targets.push(coord);
            }
        }
    }
    targets
}

fn should_place_rod(ctx: &AiContext<'_>, hero: &Hero) -> bool {
    let team = match ctx.state.get_team(ctx.team_id) {
        Some(team) => team,
        None => return false,
    };
    if team.gold() < ROD_COST {
        return false;
    }
    let pos = hero.get_position();
    if ctx.state.resource_rod_owner(*pos).is_some() {
        return false;
    }
    if ctx.state.map.resource_node_at(*pos).is_some() {
        return true;
    }
    if let Ok(tile) = ctx.state.map.get_tile(*pos) {
        return matches!(tile.kind, Tiles::Gold | Tiles::Resource);
    }
    false
}

fn plan_path_to_target(
    ctx: &AiContext<'_>,
    hero: &Hero,
    target: MapCoord,
) -> Option<Vec<AiAction>> {
    let start = *hero.get_position();
    let budget = hero.get_mov_remaining();
    if budget == 0 {
        return None;
    }
    let path = find_path(&ctx.state.map, start, target, budget, ctx.state.tile_config())?;
    if path.len() <= 1 {
        return None;
    }
    let mut actions = Vec::new();
    for window in path.windows(2) {
        let dir = direction_from_step(window[0], window[1])?;
        actions.push(AiAction::Move { hero_id: hero.get_id(), direction: dir });
    }
    Some(actions)
}

fn direction_from_step(from: MapCoord, to: MapCoord) -> Option<Direction> {
    match (to.x as i32 - from.x as i32, to.y as i32 - from.y as i32) {
        (0, -1) => Some(Direction::North),
        (1, 0) => Some(Direction::East),
        (0, 1) => Some(Direction::South),
        (-1, 0) => Some(Direction::West),
        _ => None,
    }
}

fn fallback_step(ctx: &AiContext<'_>, hero: &Hero) -> Vec<AiAction> {
    let start = *hero.get_position();
    let budget = hero.get_mov_remaining();
    if budget == 0 {
        return Vec::new();
    }
    let mut options = reachable_tiles(&ctx.state.map, start, budget, ctx.state.tile_config());
    options.sort_by_key(|coord| (coord.y, coord.x));
    let target = options.first().copied();
    let Some(target) = target else {
        return Vec::new();
    };
    let path = find_path(&ctx.state.map, start, target, budget, ctx.state.tile_config());
    if let Some(path) = path {
        if let Some(dir) = path.windows(2).find_map(|w| direction_from_step(w[0], w[1])) {
            return vec![AiAction::Move { hero_id: hero.get_id(), direction: dir }];
        }
    }
    Vec::new()
}

fn adjacent_enemy(ctx: &AiContext<'_>, hero: &Hero) -> Option<engine::hero::HeroId> {
    let pos = hero.get_position();
    let w = ctx.state.map.tile_width();
    let h = ctx.state.map.tile_height();
    let dirs = [Direction::North, Direction::East, Direction::South, Direction::West];
    for dir in dirs {
        let Some(coord) = dir.apply(*pos, w, h) else { continue };
        if let Some(other) = ctx.state.hero_at(&coord) {
            if other.get_team_id() != ctx.team_id && other.is_alive() {
                return Some(other.get_id());
            }
        }
    }
    None
}

fn nearest_target(start: &MapCoord, targets: &[MapCoord]) -> Option<MapCoord> {
    targets
        .iter()
        .copied()
        .min_by_key(|coord| manhattan(*start, *coord))
}

fn manhattan(a: MapCoord, b: MapCoord) -> u32 {
    a.x.abs_diff(b.x) + a.y.abs_diff(b.y)
}
