/// mage-engine: High-performance Magic: The Gathering engine for game tree search.
///
/// Focused on Vintage Supreme Draft format cards. Designed for:
/// - Fast state cloning (all owned data, no references)
/// - Efficient move generation
/// - Both MCTS and alpha-beta search support
///
/// # Architecture
///
/// - `types`: Core type definitions (ObjectId, PlayerId, enums)
/// - `mana`: Mana pool and cost system
/// - `card`: Static card definitions database
/// - `permanent`: Runtime permanent state on battlefield
/// - `player`: Player state (life, hand, library, graveyard)
/// - `game`: Central game state (Clone for search)
/// - `stack`: The stack (spells and abilities)
/// - `action`: Legal actions / move types
/// - `combat`: Combat damage resolution
/// - `movegen`: Move generation (legal_actions, apply_action)
/// - `search`: Search algorithms (MCTS, alpha-beta) and evaluation

pub mod types;
pub mod mana;
pub mod card;
pub mod permanent;
pub mod player;
pub mod game;
pub mod stack;
pub mod action;
pub mod combat;
pub mod movegen;
pub mod search;

#[cfg(test)]
mod tests;
