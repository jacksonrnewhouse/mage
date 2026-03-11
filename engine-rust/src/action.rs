/// Actions represent all legal moves a player can take.
/// The action space is the interface between the game engine and the search algorithm.

use crate::types::*;

/// An alternate cost that can be paid instead of a spell's normal mana cost.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AltCost {
    /// Force of Will: exile a blue card from hand + pay 1 life.
    /// `exile_id` is the ObjectId of the blue card being exiled from hand.
    ForceOfWill { exile_id: ObjectId },
    /// Force of Negation: exile a blue card from hand (opponent's turn only).
    /// `exile_id` is the ObjectId of the blue card being exiled from hand.
    ForceOfNegation { exile_id: ObjectId },
    /// Misdirection: exile a blue card from hand.
    Misdirection { exile_id: ObjectId },
    /// Commandeer: exile two blue cards from hand.
    Commandeer { exile_id1: ObjectId, exile_id2: ObjectId },
    /// Evoke cost: exile a card of the matching color from hand.
    /// `exile_id` is the ObjectId of the card being exiled.
    /// When cast via evoke, the creature enters, ETB triggers, then is sacrificed.
    Evoke { exile_id: ObjectId },
    /// Phyrexian mana cost: pay life instead of colored mana.
    /// `life_paid` is the total life paid (each Phyrexian pip costs 2 life).
    /// The remaining mana cost (normal_cost) must still be paid from the mana pool.
    /// `normal_cost` is the reduced mana cost after substituting some pips with life.
    PhyrexianMana { life_paid: u8, normal_cost: crate::mana::ManaCost },
}

/// A game action that can be taken by the active/priority player.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Action {
    /// Pass priority (or decline to act)
    PassPriority,
    /// Play a land from hand
    PlayLand(ObjectId),
    /// Play a land from the top of library (Future Sight, Bolas's Citadel).
    PlayLandFromTop(ObjectId),
    /// Cast a spell (from hand or graveyard). Includes target selection.
    /// For X spells, `x_value` is the chosen value of X (0 for non-X spells).
    /// `from_graveyard` is true when casting via flashback or Yawgmoth's Will.
    /// `from_library_top` is true when casting via Bolas's Citadel, Future Sight, Mystic Forge, etc.
    /// `alt_cost` is Some when paying an alternative cost instead of the normal mana cost.
    /// `modes` is the list of chosen mode indices for modal spells (e.g., Kolaghan's Command).
    /// Empty for non-modal spells.
    CastSpell {
        card_id: ObjectId,
        targets: Vec<Target>,
        x_value: u8,
        from_graveyard: bool,
        from_library_top: bool,
        alt_cost: Option<AltCost>,
        modes: Vec<u8>,
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
    /// Activate an ability from a card in hand (cycling, channel, etc.).
    /// `card_id` is the card in hand, `ability_index` identifies which ability.
    ActivateFromHand {
        card_id: ObjectId,
        ability_index: u8,
        targets: Vec<Target>,
        /// For X-based cycling (Shark Typhoon): the chosen value of X.
        x_value: u8,
    },
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
