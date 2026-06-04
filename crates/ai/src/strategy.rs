use engine::game_state::GameState;
use engine::hero::TeamId;

use crate::actions::AiAction;

pub struct AiContext<'a> {
    pub team_id: TeamId,
    pub state: &'a GameState,
}

pub trait AiStrategy {
    fn name(&self) -> &'static str;
    fn plan(&mut self, ctx: &AiContext<'_>) -> Vec<AiAction>;
}
