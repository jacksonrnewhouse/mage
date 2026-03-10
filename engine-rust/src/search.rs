/// Search engine API: the interface between the game engine and AI algorithms.
/// Designed to support both MCTS and alpha-beta/minimax search.

use crate::action::Action;
use crate::card::CardDef;
use crate::game::GameState;
use crate::types::*;

/// Trait for game state evaluation (heuristic function for alpha-beta).
pub trait Evaluator {
    /// Evaluate the game state from the perspective of the given player.
    /// Returns a value where higher is better for that player.
    /// Range: [-1.0, 1.0] where 1.0 = certain win, -1.0 = certain loss.
    fn evaluate(&self, state: &GameState, player: PlayerId, db: &[CardDef]) -> f64;
}

/// Simple material-based evaluator for initial testing.
pub struct MaterialEvaluator;

impl Evaluator for MaterialEvaluator {
    fn evaluate(&self, state: &GameState, player: PlayerId, _db: &[CardDef]) -> f64 {
        match state.result {
            GameResult::Win(winner) => {
                if winner == player {
                    return 1.0;
                } else {
                    return -1.0;
                }
            }
            GameResult::Draw => return 0.0,
            GameResult::InProgress => {}
        }

        let me = &state.players[player as usize];
        let opp = &state.players[state.opponent(player) as usize];

        let mut score = 0.0;

        // Life differential (normalized)
        score += (me.life - opp.life) as f64 * 0.02;

        // Card advantage
        let my_cards = me.hand.len() as f64;
        let opp_cards = opp.hand.len() as f64;
        score += (my_cards - opp_cards) * 0.05;

        // Board presence
        let my_creatures: f64 = state
            .creatures_controlled_by(player)
            .map(|c| (c.power() + c.toughness()) as f64)
            .sum();
        let opp_creatures: f64 = state
            .creatures_controlled_by(state.opponent(player))
            .map(|c| (c.power() + c.toughness()) as f64)
            .sum();
        score += (my_creatures - opp_creatures) * 0.03;

        // Mana development
        let my_mana_sources = state.permanents_controlled_by(player).count() as f64;
        let opp_mana_sources = state
            .permanents_controlled_by(state.opponent(player))
            .count() as f64;
        score += (my_mana_sources - opp_mana_sources) * 0.02;

        // Clamp to [-1, 1]
        score.max(-0.99).min(0.99)
    }
}

/// Node in a search tree (for MCTS).
#[derive(Debug, Clone)]
pub struct MctsNode {
    pub action: Option<Action>,
    pub visits: u32,
    pub total_value: f64,
    pub children: Vec<MctsNode>,
    pub untried_actions: Vec<Action>,
}

impl MctsNode {
    pub fn new(action: Option<Action>, untried: Vec<Action>) -> Self {
        MctsNode {
            action,
            visits: 0,
            total_value: 0.0,
            children: Vec::new(),
            untried_actions: untried,
        }
    }

    /// UCB1 value for this node.
    pub fn ucb1(&self, parent_visits: u32, exploration: f64) -> f64 {
        if self.visits == 0 {
            return f64::INFINITY;
        }
        let exploitation = self.total_value / self.visits as f64;
        let exploration_term = exploration * ((parent_visits as f64).ln() / self.visits as f64).sqrt();
        exploitation + exploration_term
    }

    /// Select the best child by UCB1.
    pub fn best_child(&self, exploration: f64) -> Option<usize> {
        if self.children.is_empty() {
            return None;
        }
        let parent_visits = self.visits;
        self.children
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                a.ucb1(parent_visits, exploration)
                    .partial_cmp(&b.ucb1(parent_visits, exploration))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
    }

    /// Select the most-visited child (best move after search).
    pub fn most_visited_child(&self) -> Option<&MctsNode> {
        self.children.iter().max_by_key(|c| c.visits)
    }
}

/// Run MCTS for a given number of iterations.
/// Returns the best action found.
pub fn mcts_search(
    state: &GameState,
    db: &[CardDef],
    iterations: u32,
    exploration: f64,
) -> Option<Action> {
    let player = state.priority_player;
    let actions = state.legal_actions(db);
    if actions.is_empty() {
        return None;
    }
    if actions.len() == 1 {
        return Some(actions[0].clone());
    }

    let mut root = MctsNode::new(None, actions);

    for _ in 0..iterations {
        let mut node_path = vec![0usize]; // indices into the tree
        let mut sim_state = state.clone();

        // Selection: walk down the tree using UCB1
        let mut current = &mut root;
        while current.untried_actions.is_empty() && !current.children.is_empty() {
            if let Some(best_idx) = current.best_child(exploration) {
                let action = current.children[best_idx].action.clone().unwrap();
                sim_state.apply_action(&action, db);
                node_path.push(best_idx);
                current = &mut current.children[best_idx];
            } else {
                break;
            }
        }

        // Expansion: expand one untried action
        if !current.untried_actions.is_empty() && !sim_state.is_terminal() {
            let action = current.untried_actions.pop().unwrap();
            sim_state.apply_action(&action, db);
            let child_actions = sim_state.legal_actions(db);
            let child = MctsNode::new(Some(action), child_actions);
            current.children.push(child);
            let idx = current.children.len() - 1;
            node_path.push(idx);
            let _ = &mut current.children[idx]; // reference kept for potential future expansion
        }

        // Simulation: random playout
        let result = random_playout(&mut sim_state, db, 100);
        let value = match result {
            GameResult::Win(winner) if winner == player => 1.0,
            GameResult::Win(_) => 0.0,
            GameResult::Draw => 0.5,
            GameResult::InProgress => 0.5, // Hit depth limit
        };

        // Backpropagation
        // Walk back up using the path
        backpropagate(&mut root, &node_path, value);
    }

    root.most_visited_child()
        .and_then(|c| c.action.clone())
}

fn backpropagate(root: &mut MctsNode, path: &[usize], value: f64) {
    root.visits += 1;
    root.total_value += value;
    let mut current = root;
    for &idx in path.iter().skip(1) {
        current = &mut current.children[idx];
        current.visits += 1;
        current.total_value += value;
    }
}

/// Random playout from current state. Returns game result.
fn random_playout(state: &mut GameState, db: &[CardDef], max_depth: u32) -> GameResult {
    let mut depth = 0;
    while !state.is_terminal() && depth < max_depth {
        let actions = state.legal_actions(db);
        if actions.is_empty() {
            break;
        }
        // Simple heuristic: prefer non-pass actions, but use first available
        // A real implementation would use a smarter rollout policy
        let action = if actions.len() > 1 {
            // Prefer pass priority to avoid infinite loops in random play
            &actions[0]
        } else {
            &actions[0]
        };
        state.apply_action(action, db);
        depth += 1;
    }
    state.result
}

/// Alpha-beta search with iterative deepening.
pub fn alphabeta_search(
    state: &GameState,
    db: &[CardDef],
    evaluator: &dyn Evaluator,
    max_depth: u32,
) -> Option<Action> {
    let player = state.priority_player;
    let actions = state.legal_actions(db);
    if actions.is_empty() {
        return None;
    }
    if actions.len() == 1 {
        return Some(actions[0].clone());
    }

    let mut best_action = None;
    let mut best_value = f64::NEG_INFINITY;

    for action in &actions {
        let mut child_state = state.clone();
        child_state.apply_action(action, db);
        let value = -alphabeta(
            &child_state,
            db,
            evaluator,
            max_depth - 1,
            f64::NEG_INFINITY,
            f64::INFINITY,
            state.opponent(player),
        );
        if value > best_value {
            best_value = value;
            best_action = Some(action.clone());
        }
    }

    best_action
}

fn alphabeta(
    state: &GameState,
    db: &[CardDef],
    evaluator: &dyn Evaluator,
    depth: u32,
    mut alpha: f64,
    beta: f64,
    maximizing_player: PlayerId,
) -> f64 {
    if depth == 0 || state.is_terminal() {
        return evaluator.evaluate(state, maximizing_player, db);
    }

    let actions = state.legal_actions(db);
    if actions.is_empty() {
        return evaluator.evaluate(state, maximizing_player, db);
    }

    let mut value = f64::NEG_INFINITY;
    for action in &actions {
        let mut child = state.clone();
        child.apply_action(action, db);
        let child_value = -alphabeta(
            &child,
            db,
            evaluator,
            depth - 1,
            -beta,
            -alpha,
            state.opponent(maximizing_player),
        );
        value = value.max(child_value);
        alpha = alpha.max(value);
        if alpha >= beta {
            break; // Beta cutoff
        }
    }
    value
}

/// Performance benchmarking utilities
pub mod bench {
    use super::*;

    /// Count legal actions from a given state (for move generation benchmarks).
    pub fn count_legal_actions(state: &GameState, db: &[CardDef]) -> usize {
        state.legal_actions(db).len()
    }

    /// Measure state cloning performance (returns clone).
    pub fn clone_state(state: &GameState) -> GameState {
        state.clone()
    }

    /// Perft-style test: count total leaf nodes at given depth.
    pub fn perft(state: &GameState, db: &[CardDef], depth: u32) -> u64 {
        if depth == 0 || state.is_terminal() {
            return 1;
        }
        let actions = state.legal_actions(db);
        let mut count = 0u64;
        for action in &actions {
            let mut child = state.clone();
            child.apply_action(action, db);
            count += perft(&child, db, depth - 1);
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::build_card_db;

    #[test]
    fn test_material_evaluator_terminal() {
        let mut state = GameState::new_two_player();
        state.result = GameResult::Win(0);
        let eval = MaterialEvaluator;
        let db = build_card_db();
        assert_eq!(eval.evaluate(&state, 0, &db), 1.0);
        assert_eq!(eval.evaluate(&state, 1, &db), -1.0);
    }

    #[test]
    fn test_material_evaluator_in_progress() {
        let state = GameState::new_two_player();
        let eval = MaterialEvaluator;
        let db = build_card_db();
        let score = eval.evaluate(&state, 0, &db);
        // Symmetric starting position should evaluate to ~0
        assert!(score.abs() < 0.1, "Expected ~0, got {}", score);
    }
}
