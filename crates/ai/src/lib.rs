//! AI decision-making for CPU-controlled teams.

pub mod actions;
pub mod factory;
pub mod strategy;

pub use actions::AiAction;
pub use factory::{AiFactory, AiStrategyKind};
pub use strategy::{AiContext, AiStrategy};
