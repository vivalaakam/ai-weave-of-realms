use engine::hero::HeroId;
use engine::map::game_map::Direction;
use engine::MapCoord;

/// One atomic action the AI wants to perform during its turn.
#[derive(Debug, Clone)]
pub enum AiAction {
    Move { hero_id: HeroId, direction: Direction },
    PlaceRod { hero_id: HeroId },
    HireHero { candidate_idx: usize, coord: MapCoord },
    EndTurn,
    Attack { attacker_id: HeroId, defender_id: HeroId },
}
