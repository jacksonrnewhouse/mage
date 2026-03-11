/// Actions represent all legal moves a player can take.
/// The action space is the interface between the game engine and the search algorithm.

use crate::types::*;

/// A game action that can be taken by the active/priority player.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Action {
    /// Pass priority (or decline to act)
    PassPriority,
    /// Play a land from hand
    PlayLand(ObjectId),
    /// Cast a spell from hand. Includes target selection.
    /// For X spells, `x_value` is the chosen value of X (0 for non-X spells).
    CastSpell {
        card_id: ObjectId,
        targets: Vec<Target>,
        x_value: u8,
    },
    /// Activate an ability on a permanent.
    ActivateAbility {
        permanent_id: ObjectId,
        ability_index: u8,
        targets: Vec<Target>,
    },
    /// Tap a land/permanent for mana (mana ability, doesn't use stack).
    ActivateManaAbility {
        permanent_id: ObjectId,
        /// Which color to produce (for dual lands, Birds, etc.)
        color_choice: Option<Color>,
    },
    /// Declare a creature as an attacker.
    DeclareAttacker {
        creature_id: ObjectId,
    },
    /// Done declaring attackers.
    ConfirmAttackers,
    /// Declare a creature as a blocker for a specific attacker.
    DeclareBlocker {
        blocker_id: ObjectId,
        attacker_id: ObjectId,
    },
    /// Done declaring blockers.
    ConfirmBlockers,
    /// Choose a card for an effect (e.g., Demonic Tutor search, discard)
    ChooseCard(ObjectId),
    /// Choose a number (e.g., for X costs, Toxic Deluge life payment)
    ChooseNumber(u32),
    /// Choose a color (e.g., for Black Lotus)
    ChooseColor(Color),
    /// Concede the game.
    Concede,
}

/// Categories of game situations where different action types are available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionContext {
    /// Normal priority - can cast, activate, play lands (if main phase + empty stack)
    Priority,
    /// Declaring attackers
    DeclareAttackers,
    /// Declaring blockers
    DeclareBlockers,
    /// Making a choice for an effect
    MakingChoice,
}
