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
    /// Snuff Out: pay 4 life (must control a Swamp).
    SnuffOut,
    /// Daze: return an Island you control to its owner's hand.
    /// `island_id` is the ObjectId of the Island permanent being bounced.
    Daze { island_id: ObjectId },
    /// Gush: return two Islands you control to their owner's hand.
    /// `island_id1` and `island_id2` are the ObjectIds of the two Island permanents.
    Gush { island_id1: ObjectId, island_id2: ObjectId },
    /// Force of Vigor: exile a green card from hand (not your turn only).
    /// `exile_id` is the ObjectId of the green card being exiled from hand.
    ForceOfVigor { exile_id: ObjectId },
    /// Once Upon a Time: free cast if it's the first spell you've cast this game.
    OnceUponATime,
    /// Unmask: exile a black card from hand rather than pay mana cost.
    /// `exile_id` is the ObjectId of the black card being exiled from hand.
    Unmask { exile_id: ObjectId },
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
    /// Pay {3} to put the player's companion from outside the game into their hand.
    /// The companion card is identified by its ObjectId (registered in the card_registry).
    /// Only legal when the player has an unrevealed/unused companion (player.companion is Some).
    CompanionFromSideboard,
    /// Cast the adventure half of an adventure card from hand.
    /// `card_id` is the ObjectId of the card in hand (the full card, e.g., Bonecrusher Giant).
    /// After the adventure resolves, the card goes to exile, from which the creature half can be cast.
    CastAdventure {
        card_id: ObjectId,
        targets: Vec<Target>,
    },
    /// Cast the creature half of an adventure card from exile (after its adventure resolved).
    /// `card_id` is the ObjectId of the card in exile.
    CastCreatureFromAdventureExile {
        card_id: ObjectId,
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
