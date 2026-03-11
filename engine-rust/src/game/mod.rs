/// Core game state and turn management.
/// This is the central data structure that search algorithms clone and mutate.
/// Every field is owned data (no references) for cheap Clone.

mod resolution;
mod sba;
mod triggers;

use crate::action::*;
use crate::card::*;
use crate::permanent::*;
use crate::player::*;
use crate::stack::*;
use crate::types::*;

/// Emblem types. Emblems are permanent game objects owned by a player that can't be removed.
/// They provide continuous effects or triggered abilities for the rest of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Emblem {
    /// Dack Fayden -6: "Whenever you cast a spell that targets one or more permanents,
    /// gain control of those permanents."
    DackFayden,
    /// Wrenn and Six -7: "Instant and sorcery cards in your graveyard have retrace."
    WrennAndSix,
    /// Tezzeret, Cruel Captain -7: "Whenever you cast an artifact spell, search your library
    /// for an artifact card, put it onto the battlefield, then shuffle."
    TezzeretCruelCaptain,
    /// Gideon of the Trials +0 emblem: "As long as you control a Gideon planeswalker,
    /// you can't lose the game and your opponents can't win the game."
    GideonOfTheTrials,
}

/// Where a permanent goes when it leaves the battlefield.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestinationZone {
    Graveyard,
    Exile,
    Hand,
    Library,
}

/// Complete game state. Clone this for search tree exploration.
#[derive(Debug, Clone)]
pub struct GameState {
    // --- Players ---
    pub players: Vec<Player>,
    pub active_player: PlayerId,
    pub priority_player: PlayerId,
    pub num_players: u8,

    // --- Turn structure ---
    pub turn_number: u32,
    pub phase: Phase,
    pub step: Option<Step>,

    // --- Zones ---
    pub battlefield: Vec<Permanent>,
    pub exile: Vec<(ObjectId, CardName, PlayerId)>, // (id, card, owner)
    pub stack: GameStack,

    // --- Combat ---
    pub attackers: Vec<(ObjectId, PlayerId)>, // (creature_id, defending_player)
    pub blockers: Vec<(ObjectId, ObjectId)>,  // (blocker_id, attacker_id)
    pub combat_damage_dealt: bool,

    // --- Game flow ---
    pub action_context: ActionContext,
    pub result: GameResult,
    pub passed_priority: Vec<bool>, // indexed by player id
    pub storm_count: u16,

    // --- Object ID counter ---
    next_object_id: ObjectId,

    // --- Card database reference (shared, not cloned) ---
    // In practice this is an Arc or &'static, but for simplicity
    // we'll pass it externally to methods that need it.

    // --- Pending choices ---
    pub pending_choice: Option<PendingChoice>,

    // --- Card registry: maps ObjectId -> CardName ---
    pub card_registry: Vec<(ObjectId, CardName)>,

    // --- Temporary until-end-of-turn effects ---
    pub temporary_effects: Vec<TemporaryEffect>,

    // --- Graveyard casting ---
    /// Cards in graveyards that have been granted flashback by Snapcaster Mage (cleared at end of turn).
    pub snapcaster_flashback_cards: Vec<ObjectId>,

    // --- Madness ---
    /// Cards currently exiled due to madness (waiting for the player to decide to cast or not).
    /// Each entry is (card_id, owner). When the pending choice resolves, the card is either
    /// cast from exile or moved to the graveyard.
    pub madness_exiled: Vec<(ObjectId, PlayerId)>,

    // --- Exile-linked permanents ---
    /// Maps (exiling_permanent_id, exiled_card_id) for "exile until leaves" effects.
    /// When the exiling permanent leaves the battlefield, the exiled card returns.
    pub exile_linked: Vec<(ObjectId, ObjectId)>,

    // --- Imprint ---
    /// Maps (permanent_id, imprinted_card_id) for imprint effects (Chrome Mox, Isochron Scepter).
    /// The imprinted card is exiled and referenced by the permanent for future abilities.
    pub imprinted: Vec<(ObjectId, ObjectId)>,

    /// Maps (exiling_permanent_id, token_mv) for Skyclave Apparition.
    /// When Skyclave Apparition leaves, the opponent gets a token with MV equal to the exiled card's MV.
    pub skyclave_token_mv: Vec<(ObjectId, u32)>,

    // --- Hideaway ---
    /// Maps (land_permanent_id, exiled_card_id) for hideaway lands (Shelldock Isle, Mosswort Bridge, etc.).
    /// The exiled card is face-down and can be cast for free when the hideaway condition is met.
    pub hideaway_exiled: Vec<(ObjectId, ObjectId)>,

    // --- Monarch ---
    /// The player who is currently the monarch, if any. The monarch draws a card
    /// at the beginning of their end step. When a creature deals combat damage to
    /// the monarch, that creature's controller becomes the new monarch.
    pub monarch: Option<PlayerId>,

    // --- Emblems ---
    /// Emblems created by planeswalker ultimates (and similar). Each entry is
    /// (owner, emblem_kind). Emblems can't be removed and persist for the rest of the game.
    pub emblems: Vec<(PlayerId, Emblem)>,

    // --- Delayed triggers ---
    /// Delayed triggered abilities that will fire at a future step/phase.
    /// Examples: "at the beginning of the next end step, sacrifice this" (Sneak Attack evoke-like),
    ///           "at the beginning of your next upkeep, draw a card".
    pub delayed_triggers: Vec<DelayedTrigger>,
}

/// When the game needs a player to make a choice (tutor, discard, etc.)
#[derive(Debug, Clone)]
pub struct PendingChoice {
    pub player: PlayerId,
    pub kind: ChoiceKind,
}

#[derive(Debug, Clone)]
pub enum ChoiceKind {
    /// Choose a card from a list (hand, library search result, etc.)
    ChooseFromList {
        options: Vec<ObjectId>,
        reason: ChoiceReason,
    },
    /// Choose a color (Black Lotus, etc.)
    ChooseColor { reason: ChoiceReason },
    /// Choose a number (X costs, Toxic Deluge, etc.)
    ChooseNumber {
        min: u32,
        max: u32,
        reason: ChoiceReason,
    },
}

#[derive(Debug, Clone)]
pub enum ChoiceReason {
    BlackLotusColor,
    LotusPetalColor,
    DemonicTutorSearch,
    VampiricTutorSearch,
    MysticalTutorSearch,
    EntombSearch,
    BrainstormPutBack,
    ThoughtseizeDiscard,
    HymnToTourachDiscard,
    ToxicDelugeLife,
    WheelOfFortuneDiscard,
    TimeTwisterShuffle,
    GenericDiscard,
    GenericSearch,
    /// Shock land entering the battlefield: 0 = enter tapped, 1 = pay 2 life (enter untapped)
    ShockLandETB { card_id: ObjectId },
    /// Myr Retriever: return an artifact from graveyard to hand
    MyrRetrieverReturn,
    /// Edict effect: the affected player must sacrifice a creature they control
    EdictSacrifice,
    /// Annihilator N: the defending player must sacrifice a permanent.
    /// `remaining` is the number of additional sacrifices still required after this one resolves.
    AnnihilatorSacrifice { remaining: u8 },
    /// Treasure token sacrifice: choose a color to add 1 mana of
    TreasureSacrificeColor,
    /// Cavern of Souls ETB: choose a creature type (encoded as index into CreatureType::ALL)
    CavernOfSoulsETB { cavern_id: ObjectId },
    /// Surveil N: for each card seen, choose 0 = keep on top, 1 = put in graveyard.
    /// The card_id passed to resolve_choice is the top-of-library card being surveilled.
    SurveilCard { draw_after: bool },
    /// Surveil land ETB: combined shock-land + surveil.
    /// The player first chooses 0 (enter tapped) or 1 (pay 2 life, enter untapped),
    /// then surveil 1 is triggered for the same player.
    SurveilLandShock { card_id: ObjectId },
    /// True-Name Nemesis ETB: choose a player (by player id).
    /// The permanent_id is the True-Name Nemesis's ObjectId so we can grant protection.
    TrueNameNemesisETB { permanent_id: ObjectId },
    /// Clone ETB: choose a permanent to copy.
    /// `clone_id` is the clone permanent's ObjectId.
    /// `is_metamorph` is true for Phyrexian Metamorph (always keeps Artifact type).
    CloneTarget { clone_id: ObjectId, is_metamorph: bool },
    /// Show and Tell: the choosing player may put an artifact, creature, enchantment,
    /// or planeswalker from their hand onto the battlefield.
    /// `next_player` is the player who will choose next (after this player resolves),
    /// or None if this is the last player to choose.
    /// Passing (choosing no card) is represented by ChooseCard(0) — the engine
    /// uses object ID 0 as a sentinel for "no card".
    ShowAndTellChoose { next_player: Option<PlayerId> },
    /// Chrome Mox imprint ETB: choose a nonartifact, nonland card from hand to exile.
    /// mox_id is the Chrome Mox's ObjectId so we can record the imprint link.
    /// Passing (choosing no card) is represented by ChooseCard(0).
    ChromeMoxImprint { mox_id: ObjectId },
    /// Isochron Scepter imprint ETB: choose an instant with MV <= 2 from hand to exile.
    /// scepter_id is the Isochron Scepter's ObjectId so we can record the imprint link.
    /// Passing (choosing no card) is represented by ChooseCard(0).
    IsochronScepterImprint { scepter_id: ObjectId },
    /// Hideaway ETB: look at top N cards, choose one to exile face-down, put the rest on bottom.
    /// land_id is the hideaway land's ObjectId so we can record the hideaway link.
    HideawayExile { land_id: ObjectId },
    /// Dredge replacement: before a draw, the player may dredge a card instead.
    /// `dredge_card_id` is the ObjectId of the dredge card in the graveyard.
    /// `dredge_n` is the dredge value (number of cards to mill).
    /// `remaining_draws` is how many more draws remain after this one.
    /// Choose 0 = draw normally, 1 = dredge.
    DredgeChoice {
        dredge_card_id: ObjectId,
        dredge_n: u8,
        remaining_draws: usize,
    },
    /// Coin flip for random effects (e.g. Mana Crypt upkeep trigger).
    /// For game tree search, the "chance player" chooses the outcome:
    ///   0 = heads (win the flip — no negative consequence)
    ///   1 = tails (lose the flip — negative consequence applies)
    /// This models randomness as a two-branch decision node so search can
    /// explore both outcomes (MCTS handles variance correctly this way).
    CoinFlip,
    /// Madness: this card was discarded and exiled due to madness.
    /// The player may cast it for `madness_cost` (0) or put it in the graveyard (1).
    /// `card_id` is the ObjectId of the exiled card.
    /// `madness_cost` is the alternate cost to cast it.
    MadnessCast {
        card_id: ObjectId,
        madness_cost: crate::mana::ManaCost,
    },
}
impl GameState {
    /// Create a new two-player game.
    pub fn new_two_player() -> Self {
        GameState {
            players: vec![Player::new(0), Player::new(1)],
            active_player: 0,
            priority_player: 0,
            num_players: 2,
            turn_number: 0,
            phase: Phase::Beginning,
            step: Some(Step::Untap),
            battlefield: Vec::with_capacity(32),
            exile: Vec::new(),
            stack: GameStack::new(10000), // Start stack IDs high to avoid collision
            attackers: Vec::new(),
            blockers: Vec::new(),
            combat_damage_dealt: false,
            action_context: ActionContext::Priority,
            result: GameResult::InProgress,
            passed_priority: vec![false, false],
            storm_count: 0,
            next_object_id: 1000, // Reserve 0-999 for card IDs
            pending_choice: None,
            card_registry: Vec::with_capacity(120),
            temporary_effects: Vec::new(),
            snapcaster_flashback_cards: Vec::new(),
            madness_exiled: Vec::new(),
            exile_linked: Vec::new(),
            imprinted: Vec::new(),
            skyclave_token_mv: Vec::new(),
            hideaway_exiled: Vec::new(),
            monarch: None,
            emblems: Vec::new(),
            delayed_triggers: Vec::new(),
        }
    }

    /// Allocate a new unique object ID.
    pub fn new_object_id(&mut self) -> ObjectId {
        let id = self.next_object_id;
        self.next_object_id += 1;
        id
    }

    /// Set up a player's library with a deck of card names.
    /// Returns object IDs assigned to each card.
    pub fn load_deck(&mut self, player_id: PlayerId, deck: &[CardName], _db: &[CardDef]) -> Vec<ObjectId> {
        let mut ids = Vec::with_capacity(deck.len());
        for &card_name in deck {
            let id = self.new_object_id();
            ids.push(id);
            self.players[player_id as usize].library.push(id);
            // Store the card-to-name mapping in the card registry
            self.card_registry.push((id, card_name));
        }
        ids
    }

    /// Register a companion card for a player (outside the game / sideboard).
    /// The companion card is given a new ObjectId and stored in `player.companion`.
    /// It is registered in the card_registry so the engine can look it up.
    /// Call this after `load_deck` but before `start_game`.
    /// Returns the ObjectId assigned to the companion.
    pub fn set_companion(&mut self, player_id: PlayerId, card_name: CardName) -> ObjectId {
        let id = self.new_object_id();
        self.card_registry.push((id, card_name));
        self.players[player_id as usize].companion = Some(id);
        id
    }

    // --- Turn Structure ---

    /// Start the game: each player draws 7 cards.
    pub fn start_game(&mut self) {
        self.turn_number = 1;
        self.phase = Phase::Beginning;
        self.step = Some(Step::Upkeep);
        // Draw opening hands
        for pid in 0..self.num_players {
            for _ in 0..7 {
                if let Some(id) = self.players[pid as usize].library.pop() {
                    self.players[pid as usize].hand.push(id);
                }
            }
        }
        self.active_player = 0;
        self.priority_player = 0;
    }

    /// Advance to the next phase/step.
    pub fn advance_phase(&mut self) {
        // Clear mana pools at phase change
        for p in &mut self.players {
            p.mana_pool.empty();
        }

        match (self.phase, self.step) {
            (Phase::Beginning, Some(Step::Untap)) => {
                self.step = Some(Step::Upkeep);
                self.check_delayed_triggers();
                self.give_priority_to_active();
            }
            (Phase::Beginning, Some(Step::Upkeep)) => {
                self.step = Some(Step::Draw);
                // Active player draws a card (skip on turn 1 for first player in 2-player)
                if self.turn_number > 1 || self.active_player != 0 {
                    let active = self.active_player as usize;
                    if let Some(id) = self.players[active].library.pop() {
                        self.players[active].hand.push(id);
                    }
                }
                self.give_priority_to_active();
            }
            (Phase::Beginning, Some(Step::Draw)) => {
                self.phase = Phase::PreCombatMain;
                self.step = None;
                self.give_priority_to_active();
            }
            (Phase::PreCombatMain, _) => {
                self.phase = Phase::Combat;
                self.step = Some(Step::BeginCombat);
                self.give_priority_to_active();
            }
            (Phase::Combat, Some(Step::BeginCombat)) => {
                self.step = Some(Step::DeclareAttackers);
                self.action_context = ActionContext::DeclareAttackers;
                self.attackers.clear();
                self.give_priority_to_active();
            }
            (Phase::Combat, Some(Step::DeclareAttackers)) => {
                if self.attackers.is_empty() {
                    // No attackers, skip to post-combat main
                    self.phase = Phase::PostCombatMain;
                    self.step = None;
                    self.action_context = ActionContext::Priority;
                    self.give_priority_to_active();
                } else {
                    self.step = Some(Step::DeclareBlockers);
                    self.action_context = ActionContext::DeclareBlockers;
                    // Non-active player declares blockers
                    self.priority_player = self.opponent(self.active_player);
                }
            }
            (Phase::Combat, Some(Step::DeclareBlockers)) => {
                self.action_context = ActionContext::Priority;
                // Check for first strike
                let has_first_strike = self.attackers.iter().any(|(id, _)| {
                    self.find_permanent(*id)
                        .map(|p| p.keywords.has(Keyword::FirstStrike) || p.keywords.has(Keyword::DoubleStrike))
                        .unwrap_or(false)
                });
                if has_first_strike {
                    self.step = Some(Step::FirstStrikeDamage);
                } else {
                    self.step = Some(Step::CombatDamage);
                }
                self.give_priority_to_active();
            }
            (Phase::Combat, Some(Step::FirstStrikeDamage)) => {
                self.step = Some(Step::CombatDamage);
                self.give_priority_to_active();
            }
            (Phase::Combat, Some(Step::CombatDamage)) => {
                self.step = Some(Step::EndOfCombat);
                self.give_priority_to_active();
            }
            (Phase::Combat, Some(Step::EndOfCombat)) => {
                self.attackers.clear();
                self.blockers.clear();
                self.combat_damage_dealt = false;
                self.phase = Phase::PostCombatMain;
                self.step = None;
                self.action_context = ActionContext::Priority;
                self.give_priority_to_active();
            }
            (Phase::PostCombatMain, _) => {
                self.phase = Phase::Ending;
                self.step = Some(Step::End);
                // Monarch draws a card at the beginning of their end step.
                if let Some(monarch_id) = self.monarch {
                    if monarch_id == self.active_player {
                        self.stack.push(
                            crate::stack::StackItemKind::TriggeredAbility {
                                source_id: 0,
                                source_name: crate::card::CardName::Plains, // placeholder
                                effect: crate::stack::TriggeredEffect::MonarchEndStep,
                            },
                            monarch_id,
                            vec![],
                        );
                    }
                }
                self.check_delayed_triggers();
                self.give_priority_to_active();
            }
            (Phase::Ending, Some(Step::End)) => {
                self.step = Some(Step::Cleanup);
                self.cleanup_step();
            }
            (Phase::Ending, Some(Step::Cleanup)) => {
                self.next_turn();
            }
            _ => {
                // Shouldn't happen, advance to next turn as safety
                self.next_turn();
            }
        }
        self.reset_priority_passes();
    }

    fn cleanup_step(&mut self) {
        // Discard to hand size (7)
        let active = self.active_player as usize;
        while self.players[active].hand.len() > 7 {
            // For AI: this becomes a choice. For now, discard last card.
            if let Some(id) = self.players[active].hand.pop() {
                self.players[active].graveyard.push(id);
            }
        }
        // Clear damage and per-turn flags from all permanents
        for perm in &mut self.battlefield {
            perm.end_of_turn_cleanup();
        }
        // Reverse and clear all temporary until-end-of-turn effects
        self.end_of_turn_cleanup();
        // Clear Snapcaster Mage flashback grants
        self.snapcaster_flashback_cards.clear();
    }

    /// Apply a temporary effect immediately to the target permanent,
    /// and record it so it can be reversed at end of turn.
    pub fn add_temporary_effect(&mut self, effect: TemporaryEffect) {
        match &effect {
            TemporaryEffect::ModifyPT { target, power, toughness } => {
                let (target, power, toughness) = (*target, *power, *toughness);
                if let Some(perm) = self.find_permanent_mut(target) {
                    perm.power_mod += power;
                    perm.toughness_mod += toughness;
                }
            }
            TemporaryEffect::GrantKeyword { target, keyword } => {
                let (target, keyword) = (*target, *keyword);
                if let Some(perm) = self.find_permanent_mut(target) {
                    perm.keywords.add(keyword);
                }
            }
            TemporaryEffect::RemoveAllAbilities { target, .. } => {
                let target = *target;
                if let Some(perm) = self.find_permanent_mut(target) {
                    perm.keywords = Keywords::empty();
                }
            }
        }
        self.temporary_effects.push(effect);
    }

    /// Reverse all temporary until-end-of-turn effects and clear the list.
    /// Called during the cleanup step.
    pub fn end_of_turn_cleanup(&mut self) {
        let effects = std::mem::take(&mut self.temporary_effects);
        for effect in &effects {
            match effect {
                TemporaryEffect::ModifyPT { target, power, toughness } => {
                    if let Some(perm) = self.find_permanent_mut(*target) {
                        perm.power_mod -= power;
                        perm.toughness_mod -= toughness;
                    }
                }
                TemporaryEffect::GrantKeyword { target, keyword } => {
                    if let Some(perm) = self.find_permanent_mut(*target) {
                        perm.keywords.remove(*keyword);
                    }
                }
                TemporaryEffect::RemoveAllAbilities { target, saved_keywords } => {
                    if let Some(perm) = self.find_permanent_mut(*target) {
                        perm.keywords = *saved_keywords;
                    }
                }
            }
        }
        // temporary_effects already cleared by mem::take
    }

    fn next_turn(&mut self) {
        // Check for extra turns
        let active = self.active_player as usize;
        if self.players[active].extra_turns > 0 {
            self.players[active].extra_turns -= 1;
        } else {
            self.active_player = self.opponent(self.active_player);
        }

        self.turn_number += 1;
        self.phase = Phase::Beginning;
        self.step = Some(Step::Untap);
        self.storm_count = 0;

        let active = self.active_player as usize;
        self.players[active].reset_for_turn();

        // Untap permanents
        self.untap_step();

        self.priority_player = self.active_player;
        self.action_context = ActionContext::Priority;
    }

    /// Untap all permanents controlled by the active player, skipping those
    /// that have the `doesnt_untap` flag set (e.g. Mana Vault, Grim Monolith, Time Vault).
    pub fn untap_step(&mut self) {
        for perm in &mut self.battlefield {
            if perm.controller == self.active_player && !perm.doesnt_untap {
                perm.tapped = false;
            }
        }
    }

    fn give_priority_to_active(&mut self) {
        self.priority_player = self.active_player;
        self.action_context = ActionContext::Priority;
    }

    pub fn reset_priority_passes(&mut self) {
        for p in &mut self.passed_priority {
            *p = false;
        }
    }

    /// Get the opponent of a player (2-player only).
    pub fn opponent(&self, player: PlayerId) -> PlayerId {
        1 - player
    }

    // --- Battlefield queries ---

    pub fn find_permanent(&self, id: ObjectId) -> Option<&Permanent> {
        self.battlefield.iter().find(|p| p.id == id)
    }

    pub fn find_permanent_mut(&mut self, id: ObjectId) -> Option<&mut Permanent> {
        self.battlefield.iter_mut().find(|p| p.id == id)
    }

    pub fn permanents_controlled_by(&self, player: PlayerId) -> impl Iterator<Item = &Permanent> {
        self.battlefield.iter().filter(move |p| p.controller == player)
    }

    pub fn creatures_controlled_by(&self, player: PlayerId) -> impl Iterator<Item = &Permanent> {
        self.battlefield
            .iter()
            .filter(move |p| p.controller == player && p.is_creature())
    }

    pub fn lands_controlled_by(&self, player: PlayerId) -> impl Iterator<Item = &Permanent> {
        self.battlefield
            .iter()
            .filter(move |p| p.controller == player && p.is_land())
    }

    pub fn artifacts_controlled_by(&self, player: PlayerId) -> impl Iterator<Item = &Permanent> {
        self.battlefield
            .iter()
            .filter(move |p| p.controller == player && p.is_artifact())
    }

    /// Count the number of colored mana symbols of a given color among permanents controlled by
    /// player. This implements the "devotion to <color>" mechanic (e.g. Thassa's Oracle, Nykthos).
    /// Lands are excluded (they don't have mana costs that contribute to devotion).
    pub fn devotion_to(&self, player: PlayerId, color: Color, db: &[CardDef]) -> u32 {
        let mut count: u32 = 0;
        for perm in self.permanents_controlled_by(player) {
            // Lands don't contribute to devotion
            if perm.is_land() {
                continue;
            }
            if let Some(def) = find_card(db, perm.card_name) {
                count += match color {
                    Color::White => def.mana_cost.white as u32,
                    Color::Blue => def.mana_cost.blue as u32,
                    Color::Black => def.mana_cost.black as u32,
                    Color::Red => def.mana_cost.red as u32,
                    Color::Green => def.mana_cost.green as u32,
                };
            }
        }
        count
    }

    /// Returns true if the player controls three or more artifacts (metalcraft condition).
    pub fn metalcraft(&self, player: PlayerId) -> bool {
        self.artifacts_controlled_by(player).count() >= 3
    }

    /// Low-level removal: removes a permanent from the battlefield without firing triggers.
    /// Prefer `remove_permanent_to_zone` for game-logic removal.
    pub fn remove_permanent(&mut self, id: ObjectId) -> Option<Permanent> {
        if let Some(pos) = self.battlefield.iter().position(|p| p.id == id) {
            Some(self.battlefield.swap_remove(pos))
        } else {
            None
        }
    }

    /// Centralized permanent removal: removes from battlefield, places in destination zone,
    /// and fires dies/leaves-battlefield triggers as appropriate.
    pub fn remove_permanent_to_zone(&mut self, id: ObjectId, destination: DestinationZone) -> Option<Permanent> {
        // Before removing: collect attachment info for cleanup.
        // If this is a creature, detach all its equipment (equipment stays, becomes unattached).
        // If this is an equipment, remove its bonuses from its host.
        let attachments_to_unequip: Vec<ObjectId> = self.find_permanent(id)
            .map(|p| p.attachments.clone())
            .unwrap_or_default();
        let attached_to_host: Option<ObjectId> = self.find_permanent(id).and_then(|p| p.attached_to);

        // Collect Skullclamp trigger info before removing (equip_id and controller)
        // A Skullclamp trigger fires when its equipped creature dies.
        let skullclamp_trigger: Option<(ObjectId, PlayerId)> = {
            let mut result = None;
            if destination == DestinationZone::Graveyard {
                // Check if dying permanent is a creature equipped with Skullclamp
                if let Some(dying) = self.find_permanent(id) {
                    if dying.is_creature() {
                        for &att_id in &dying.attachments {
                            if let Some(att) = self.find_permanent(att_id) {
                                if att.card_name == CardName::SkullClamp {
                                    result = Some((att_id, att.controller));
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            result
        };

        // If the leaving permanent is an equipment attached to a creature, remove its bonuses.
        if let Some(host_id) = attached_to_host {
            self.remove_equipment_bonuses(id, host_id);
            if let Some(host) = self.find_permanent_mut(host_id) {
                host.attachments.retain(|&att_id| att_id != id);
            }
        }

        // If the leaving permanent is a creature, detach all equipment from it.
        for equip_id in &attachments_to_unequip {
            self.remove_equipment_bonuses(*equip_id, id);
            if let Some(equip) = self.find_permanent_mut(*equip_id) {
                equip.attached_to = None;
            }
        }

        let perm = self.remove_permanent(id)?;
        let perm_id = perm.id;
        let perm_name = perm.card_name;
        let controller = perm.controller;
        let owner = perm.owner;
        let is_artifact = perm.is_artifact();
        let is_token = perm.is_token;

        // Apply graveyard-replacement effects: if Rest in Peace is on the battlefield,
        // any card that would go to the graveyard goes to exile instead.
        let actual_destination = if destination == DestinationZone::Graveyard {
            self.graveyard_destination(owner)
        } else {
            destination
        };

        // Place in destination zone (tokens cease to exist, but we still fire triggers)
        if !is_token {
            match actual_destination {
                DestinationZone::Graveyard => {
                    self.players[owner as usize].graveyard.push(perm_id);
                }
                DestinationZone::Exile => {
                    self.exile.push((perm_id, perm_name, owner));
                }
                DestinationZone::Hand => {
                    self.players[owner as usize].hand.push(perm_id);
                }
                DestinationZone::Library => {
                    self.players[owner as usize].library.push(perm_id);
                }
            }
        }

        // Check dies triggers (only when actually going to graveyard)
        if actual_destination == DestinationZone::Graveyard {
            self.check_dies_triggers(perm_id, perm_name, controller, is_artifact);
        }

        // Fire Skullclamp trigger: when equipped creature dies (goes to graveyard), draw 2.
        // With Rest in Peace the creature goes to exile, so Skullclamp does NOT trigger.
        if actual_destination == DestinationZone::Graveyard {
            if let Some((skullclamp_id, skullclamp_controller)) = skullclamp_trigger {
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: skullclamp_id,
                        source_name: CardName::SkullClamp,
                        effect: TriggeredEffect::SkullclampDeath,
                    },
                    skullclamp_controller,
                    vec![],
                );
            }
        }

        // Check leaves-battlefield triggers (for all removals)
        self.check_leaves_triggers(perm_id, perm_name, controller);

        Some(perm)
    }

    /// Convenience: destroy a permanent (move to graveyard with triggers).
    /// If a graveyard-replacement effect (Rest in Peace) is active, the permanent goes to exile instead.
    pub fn destroy_permanent(&mut self, id: ObjectId) -> Option<Permanent> {
        let owner = self.find_permanent(id).map(|p| p.owner);
        let dest = owner
            .map(|o| self.graveyard_destination(o))
            .unwrap_or(DestinationZone::Graveyard);
        self.remove_permanent_to_zone(id, dest)
    }

    /// Check whether a graveyard-replacement effect is in play for the given card owner.
    /// Returns the actual destination zone (Exile when Rest in Peace is on the battlefield,
    /// Graveyard otherwise).
    pub fn graveyard_destination(&self, _owner: PlayerId) -> DestinationZone {
        let rest_in_peace_on_battlefield = self
            .battlefield
            .iter()
            .any(|p| p.card_name == CardName::RestInPeace);
        if rest_in_peace_on_battlefield {
            DestinationZone::Exile
        } else {
            DestinationZone::Graveyard
        }
    }

    /// Check whether Grafdigger's Cage is on the battlefield.
    /// When true, creature cards from graveyards and libraries can't enter the battlefield,
    /// and players can't cast spells from graveyards or libraries.
    pub fn grafdiggers_cage_active(&self) -> bool {
        self.battlefield
            .iter()
            .any(|p| p.card_name == CardName::GrafdiggersCage)
    }

    /// Check whether Containment Priest is on the battlefield.
    /// When true, nontoken creatures that weren't cast are exiled instead of entering.
    pub fn containment_priest_active(&self) -> bool {
        self.battlefield
            .iter()
            .any(|p| p.card_name == CardName::ContainmentPriest)
    }

    /// Send a card (by id and name) from any zone directly to the graveyard,
    /// applying graveyard-replacement effects (Rest in Peace → exile instead).
    /// Returns the actual destination zone used.
    pub fn send_to_graveyard(&mut self, card_id: ObjectId, card_name: CardName, owner: PlayerId) -> DestinationZone {
        let dest = self.graveyard_destination(owner);
        match dest {
            DestinationZone::Graveyard => {
                self.players[owner as usize].graveyard.push(card_id);
            }
            DestinationZone::Exile => {
                self.exile.push((card_id, card_name, owner));
            }
            _ => {
                self.players[owner as usize].graveyard.push(card_id);
            }
        }
        dest
    }

    /// Discard a card from a player's hand to the graveyard (or exile if madness applies).
    /// Handles the madness replacement effect: if the card has madness, it goes to exile instead
    /// and a pending choice is created (cast for madness cost or put in graveyard).
    /// Also increments `cards_discarded_this_turn` for the player.
    /// `db` is the card database for madness cost lookup.
    pub fn discard_card(&mut self, card_id: ObjectId, owner: PlayerId, db: &[crate::card::CardDef]) {
        // Track discard count for Hollow One cost reduction.
        self.players[owner as usize].cards_discarded_this_turn += 1;

        // Check for madness: look up the card's madness cost.
        let madness = if let Some(cn) = self.card_name_for_id(card_id) {
            crate::card::find_card(db, cn)
                .and_then(|def| def.madness_cost)
                .map(|mc| mc)
        } else {
            None
        };

        if let Some(madness_cost) = madness {
            // Madness replacement: card goes to exile instead of graveyard.
            let card_name = self.card_name_for_id(card_id).unwrap_or(CardName::Plains);
            self.exile.push((card_id, card_name, owner));
            self.madness_exiled.push((card_id, owner));

            // Queue a pending choice: 0 = cast for madness cost, 1 = put in graveyard.
            self.pending_choice = Some(PendingChoice {
                player: owner,
                kind: ChoiceKind::ChooseNumber {
                    min: 0,
                    max: 1,
                    reason: ChoiceReason::MadnessCast { card_id, madness_cost },
                },
            });
        } else {
            // Normal discard: apply graveyard replacement effects (Rest in Peace, etc.)
            let card_name = self.card_name_for_id(card_id).unwrap_or(CardName::Plains);
            self.send_to_graveyard(card_id, card_name, owner);
        }
    }

    /// Trigger annihilator N for the given defending player: they must sacrifice N permanents.
    /// Sets up N pending edict-style sacrifice choices, chained via AnnihilatorSacrifice { remaining }.
    pub fn trigger_annihilator(&mut self, defending_player: PlayerId, n: u8) {
        if n == 0 {
            return;
        }
        let permanents: Vec<ObjectId> = self.battlefield.iter()
            .filter(|p| p.controller == defending_player)
            .map(|p| p.id)
            .collect();
        if permanents.is_empty() {
            return;
        }
        self.pending_choice = Some(PendingChoice {
            player: defending_player,
            kind: ChoiceKind::ChooseFromList {
                options: permanents,
                reason: ChoiceReason::AnnihilatorSacrifice { remaining: n - 1 },
            },
        });
    }

    /// Make a player the monarch. The monarch draws a card at the beginning of their
    /// end step. If there's already a monarch, they lose the designation.
    pub fn become_monarch(&mut self, player_id: PlayerId) {
        self.monarch = Some(player_id);
    }

    /// Create an emblem for a player. Emblems can't be removed and persist for the rest of the game.
    pub fn create_emblem(&mut self, player_id: PlayerId, emblem: Emblem) {
        self.emblems.push((player_id, emblem));
    }

    /// Check whether a player has a specific emblem.
    pub fn has_emblem(&self, player_id: PlayerId, emblem: Emblem) -> bool {
        self.emblems.iter().any(|&(pid, e)| pid == player_id && e == emblem)
    }

    /// Check whether any player has a specific emblem.
    pub fn any_player_has_emblem(&self, emblem: Emblem) -> bool {
        self.emblems.iter().any(|&(_, e)| e == emblem)
    }

    /// Register a delayed trigger. It will fire at the specified condition
    /// and be removed after firing if `fires_once` is true.
    pub fn add_delayed_trigger(&mut self, trigger: DelayedTrigger) {
        self.delayed_triggers.push(trigger);
    }

    /// Check and fire delayed triggers for the current step.
    /// Called after entering each step that delayed triggers can target.
    /// Fires all matching triggers by pushing them onto the stack;
    /// removes one-shot triggers after firing.
    pub fn check_delayed_triggers(&mut self) {
        let active = self.active_player;
        let step = self.step;
        let phase = self.phase;

        // Collect indices of triggers that should fire
        let firing: Vec<usize> = self.delayed_triggers
            .iter()
            .enumerate()
            .filter(|(_, dt)| {
                match dt.condition {
                    DelayedTriggerCondition::AtBeginningOfEndStep { player } => {
                        phase == Phase::Ending && step == Some(Step::End) && active == player
                    }
                    DelayedTriggerCondition::AtBeginningOfUpkeep { player } => {
                        phase == Phase::Beginning && step == Some(Step::Upkeep) && active == player
                    }
                    DelayedTriggerCondition::AtBeginningOfNextEndStep => {
                        phase == Phase::Ending && step == Some(Step::End)
                    }
                    DelayedTriggerCondition::AtBeginningOfNextUpkeep => {
                        phase == Phase::Beginning && step == Some(Step::Upkeep)
                    }
                }
            })
            .map(|(i, _)| i)
            .collect();

        if firing.is_empty() {
            return;
        }

        // Collect triggers to fire (clone data needed for stack push)
        let to_fire: Vec<(TriggeredEffect, PlayerId, bool)> = firing
            .iter()
            .map(|&i| {
                let dt = &self.delayed_triggers[i];
                (dt.effect.clone(), dt.controller, dt.fires_once)
            })
            .collect();

        // Remove one-shot triggers (in reverse index order to preserve indices)
        let mut to_remove: Vec<usize> = firing
            .iter()
            .copied()
            .filter(|&i| self.delayed_triggers[i].fires_once)
            .collect();
        to_remove.sort_unstable_by(|a, b| b.cmp(a));
        for i in to_remove {
            self.delayed_triggers.swap_remove(i);
        }

        // Push triggered abilities onto the stack
        for (effect, controller, _) in to_fire {
            self.stack.push(
                StackItemKind::TriggeredAbility {
                    source_id: 0,
                    source_name: crate::card::CardName::Plains, // placeholder source
                    effect,
                },
                controller,
                vec![],
            );
        }
    }

    /// Change the controller of a permanent. Does not fire triggers.
    pub fn gain_control(&mut self, perm_id: ObjectId, new_controller: PlayerId) {
        if let Some(perm) = self.find_permanent_mut(perm_id) {
            perm.controller = new_controller;
        }
    }

    /// Exchange controllers between two permanents.
    pub fn exchange_control(&mut self, perm_a: ObjectId, perm_b: ObjectId) {
        let controller_a = self.find_permanent(perm_a).map(|p| p.controller);
        let controller_b = self.find_permanent(perm_b).map(|p| p.controller);
        if let (Some(ca), Some(cb)) = (controller_a, controller_b) {
            self.gain_control(perm_a, cb);
            self.gain_control(perm_b, ca);
        }
    }

    // --- Priority system ---

    /// Both players passed priority in succession on an empty stack.
    pub fn both_passed_on_empty_stack(&self) -> bool {
        self.stack.is_empty() && self.passed_priority.iter().all(|&p| p)
    }

    /// Pass priority to the next player, or resolve top of stack.
    pub fn pass_priority(&mut self, db: &[CardDef]) {
        self.passed_priority[self.priority_player as usize] = true;

        if self.passed_priority.iter().all(|&p| p) {
            // Both players passed
            if self.stack.is_empty() {
                // Advance the game
                self.advance_phase();
            } else {
                // Resolve top of stack
                self.resolve_top(db);
                self.reset_priority_passes();
                self.give_priority_to_active();
            }
        } else {
            // Pass to opponent
            self.priority_player = self.opponent(self.priority_player);
        }
    }

    // --- Game result ---

    pub fn is_terminal(&self) -> bool {
        self.result != GameResult::InProgress
    }

    // --- Utility ---

    /// Surveil N: look at the top N cards of a player's library.
    /// For AI search simplicity, we model this as a pending binary choice per card:
    /// the player either keeps each card on top (ChooseNumber 0) or puts it in the
    /// graveyard (ChooseNumber 1).  For N > 1 we queue only the first card; the
    /// choice handler re-queues for the remaining cards automatically.
    ///
    /// `draw_after`: when true, draw a card after the surveil completes (e.g. Consider).
    pub fn surveil(&mut self, player: PlayerId, count: u8, draw_after: bool) {
        if count == 0 {
            if draw_after {
                self.draw_cards(player, 1);
            }
            return;
        }
        // Only set a choice if there are cards to surveil.
        let pid = player as usize;
        if self.players[pid].library.is_empty() {
            if draw_after {
                self.draw_cards(player, 1);
            }
            return;
        }
        // Peek at the top card (last element = top of library in this engine).
        let top_id = *self.players[pid].library.last().unwrap();
        self.pending_choice = Some(PendingChoice {
            player,
            kind: ChoiceKind::ChooseNumber {
                min: 0,
                max: 1,
                reason: ChoiceReason::SurveilCard { draw_after },
            },
        });
        // We need the card_id to be retrievable; we store a reference via the top of library.
        // The choice resolver will pop the top card and handle it.
        // No extra storage needed: the resolver always uses the current library top.
        let _ = top_id; // suppress unused warning; resolver reads it directly
    }

    /// Returns the dredge value for a card name, or None if the card does not have dredge.
    pub fn dredge_value(card_name: CardName) -> Option<u8> {
        match card_name {
            CardName::GolgariGraveTroll => Some(6),
            CardName::StinkweedImp => Some(5),
            CardName::LifeFromTheLoam => Some(3),
            _ => None,
        }
    }

    /// Find the first dredge card in a player's graveyard that can be dredged
    /// (i.e., the player has at least `dredge_n` cards in their library).
    /// Returns (card_id, dredge_n) for the first eligible dredge card, or None.
    pub fn find_dredgeable(&self, player: PlayerId) -> Option<(ObjectId, u8)> {
        let pid = player as usize;
        let lib_size = self.players[pid].library.len();
        for &card_id in &self.players[pid].graveyard {
            if let Some(name) = self.card_name_for_id(card_id) {
                if let Some(n) = Self::dredge_value(name) {
                    if lib_size >= n as usize {
                        return Some((card_id, n));
                    }
                }
            }
        }
        None
    }

    pub fn draw_cards(&mut self, player: PlayerId, count: usize) {
        let pid = player as usize;
        for remaining in (0..count).rev() {
            // Check for draw-limiter statics before each individual draw.
            // Spirit of the Labyrinth: each player can't draw more than one card per turn.
            // Narset, Parter of Veils: opponents can't draw more than one card per turn.
            // Leovold, Emissary of Trest: opponents can't draw more than one card per turn.
            if self.players[pid].draws_this_turn >= 1 {
                let limited = self.battlefield.iter().any(|p| {
                    matches!(p.card_name, CardName::SpiritOfTheLabyrinth)
                        || (matches!(p.card_name, CardName::NarsetParterOfVeils)
                            && p.controller != player)
                        || (matches!(p.card_name, CardName::LeovoldEmissaryOfTrest)
                            && p.controller != player)
                });
                if limited {
                    break;
                }
            }
            // Check for a dredge replacement: if the player has a dredgeable card in their
            // graveyard, offer a pending choice before the draw.
            if let Some((dredge_card_id, dredge_n)) = self.find_dredgeable(player) {
                self.pending_choice = Some(PendingChoice {
                    player,
                    kind: ChoiceKind::ChooseNumber {
                        min: 0,
                        max: 1,
                        reason: ChoiceReason::DredgeChoice {
                            dredge_card_id,
                            dredge_n,
                            remaining_draws: remaining,
                        },
                    },
                });
                // Stop the draw loop; remaining draws will be continued after the choice is resolved.
                return;
            }
            if let Some(id) = self.players[pid].library.pop() {
                self.players[pid].hand.push(id);
                self.players[pid].draws_this_turn += 1;
                self.players[pid].has_drawn_this_turn = true;
            } else {
                // Can't draw from empty library - player loses
                self.players[pid].has_lost = true;
            }
        }
    }

    fn deal_damage_to_target(&mut self, target: Target, amount: u16, _source_controller: PlayerId) {
        match target {
            Target::Player(p) => {
                // Prevent all damage to a player with protection from everything
                // (e.g. The One Ring ETB effect until their next turn).
                if self.players[p as usize].protection_from_everything {
                    return;
                }
                self.players[p as usize].life -= amount as i32;
            }
            Target::Object(id) => {
                if let Some(perm) = self.find_permanent_mut(id) {
                    perm.damage += amount as i16;
                }
            }
            Target::None => {}
        }
    }

    pub fn card_name_for_id(&self, id: ObjectId) -> Option<CardName> {
        self.card_registry
            .iter()
            .find(|(obj_id, _)| *obj_id == id)
            .map(|(_, name)| *name)
    }

    // --- Dynamic P/T ---

    /// Count the number of distinct card types among all cards in all players' graveyards.
    /// Used for Tarmogoyf and similar lhurgoyf-style creatures.
    pub fn graveyard_card_type_count(&self, db: &[CardDef]) -> i16 {
        let mut seen_types: u8 = 0; // bitfield for CardType variants (7 types)
        for player in &self.players {
            for &obj_id in &player.graveyard {
                if let Some(card_name) = self.card_name_for_id(obj_id) {
                    if let Some(def) = find_card(db, card_name) {
                        for &ct in def.card_types {
                            let bit = match ct {
                                CardType::Land => 1 << 0,
                                CardType::Creature => 1 << 1,
                                CardType::Artifact => 1 << 2,
                                CardType::Enchantment => 1 << 3,
                                CardType::Instant => 1 << 4,
                                CardType::Sorcery => 1 << 5,
                                CardType::Planeswalker => 1 << 6,
                            };
                            seen_types |= bit;
                        }
                    }
                }
            }
        }
        seen_types.count_ones() as i16
    }

    /// Effective power of a permanent, accounting for dynamic P/T (e.g. Tarmogoyf).
    /// For most creatures this equals `perm.power()`. For lhurgoyf-style creatures,
    /// it is calculated from game state.
    pub fn effective_power(&self, perm_id: ObjectId, db: &[CardDef]) -> i16 {
        let perm = match self.find_permanent(perm_id) {
            Some(p) => p,
            None => return 0,
        };
        match perm.card_name {
            CardName::Tarmogoyf => {
                let count = self.graveyard_card_type_count(db);
                count + perm.power_mod
                    + perm.counters.get(CounterType::PlusOnePlusOne)
                    - perm.counters.get(CounterType::MinusOneMinusOne)
            }
            _ => perm.power(),
        }
    }

    /// Effective toughness of a permanent, accounting for dynamic P/T (e.g. Tarmogoyf).
    pub fn effective_toughness(&self, perm_id: ObjectId, db: &[CardDef]) -> i16 {
        let perm = match self.find_permanent(perm_id) {
            Some(p) => p,
            None => return 0,
        };
        match perm.card_name {
            CardName::Tarmogoyf => {
                let count = self.graveyard_card_type_count(db);
                count + 1 + perm.toughness_mod
                    + perm.counters.get(CounterType::PlusOnePlusOne)
                    - perm.counters.get(CounterType::MinusOneMinusOne)
            }
            _ => perm.toughness(),
        }
    }

    /// Create a Treasure token controlled by the given player and place it on the battlefield.
    /// Returns the ObjectId of the newly created token.
    pub fn create_treasure_token(&mut self, controller: PlayerId) -> ObjectId {
        let token_id = self.new_object_id();
        let mut token = Permanent::new(
            token_id,
            CardName::TreasureToken,
            controller,
            controller,
            None,
            None,
            None,
            Keywords::empty(),
            &[CardType::Artifact],
        );
        token.is_token = true;
        self.battlefield.push(token);
        token_id
    }
}

/// Placeholder card name for tokens (they don't have real card names).
pub(crate) fn card_name_for_token() -> CardName {
    CardName::Plains // Placeholder - tokens would need their own system
}
