/// Core game state and turn management.
/// This is the central data structure that search algorithms clone and mutate.
/// Every field is owned data (no references) for cheap Clone.

use crate::action::*;
use crate::card::*;
use crate::permanent::*;
use crate::player::*;
use crate::stack::*;
use crate::types::*;

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
        // Clear damage from all creatures
        for perm in &mut self.battlefield {
            perm.end_of_turn_cleanup();
        }
        // Clear temporary power/toughness modifications
        for perm in &mut self.battlefield {
            perm.power_mod = 0;
            perm.toughness_mod = 0;
        }
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
        for perm in &mut self.battlefield {
            if perm.controller == self.active_player {
                // TODO: handle "doesn't untap" (Mana Vault, Grim Monolith)
                perm.tapped = false;
            }
        }

        self.priority_player = self.active_player;
        self.action_context = ActionContext::Priority;
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

    pub fn remove_permanent(&mut self, id: ObjectId) -> Option<Permanent> {
        if let Some(pos) = self.battlefield.iter().position(|p| p.id == id) {
            Some(self.battlefield.swap_remove(pos))
        } else {
            None
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

    pub fn check_state_based_actions(&mut self) {
        let mut changes = true;
        while changes {
            changes = false;

            // Player loses if life <= 0
            for i in 0..self.num_players as usize {
                if self.players[i].life <= 0 && !self.players[i].has_lost {
                    self.players[i].has_lost = true;
                    changes = true;
                }
            }

            // Player loses if they tried to draw from empty library
            // (Handled when draw happens)

            // Creatures with 0 or less toughness die
            let mut to_die = Vec::new();
            for perm in &self.battlefield {
                if perm.is_creature() && (perm.toughness() <= 0 || perm.has_lethal_damage()) {
                    to_die.push(perm.id);
                }
            }
            for id in to_die {
                if let Some(perm) = self.remove_permanent(id) {
                    self.players[perm.owner as usize].graveyard.push(perm.id);
                    changes = true;
                }
            }

            // Planeswalkers with 0 or less loyalty die
            let mut pw_to_die = Vec::new();
            for perm in &self.battlefield {
                if perm.is_planeswalker() && perm.loyalty <= 0 {
                    pw_to_die.push(perm.id);
                }
            }
            for id in pw_to_die {
                if let Some(perm) = self.remove_permanent(id) {
                    self.players[perm.owner as usize].graveyard.push(perm.id);
                    changes = true;
                }
            }

            // Legend rule: if a player controls 2+ legendaries with the same name,
            // they choose one to keep (for simplicity, keep the newer one)
            let mut legend_names: Vec<(CardName, PlayerId, ObjectId)> = Vec::new();
            let mut legend_to_remove = Vec::new();
            for perm in &self.battlefield {
                if perm.card_types.contains(&CardType::Planeswalker)
                    || self.is_legendary(perm)
                {
                    if let Some(existing) = legend_names.iter().find(|(n, c, _)| {
                        *n == perm.card_name && *c == perm.controller
                    }) {
                        legend_to_remove.push(existing.2); // Remove older one
                    }
                    legend_names.push((perm.card_name, perm.controller, perm.id));
                }
            }
            for id in legend_to_remove {
                if let Some(perm) = self.remove_permanent(id) {
                    self.players[perm.owner as usize].graveyard.push(perm.id);
                    changes = true;
                }
            }
        }

        // Check for game over
        let alive_count = self.players.iter().filter(|p| !p.has_lost).count();
        if alive_count <= 1 {
            if let Some(winner) = self.players.iter().find(|p| !p.has_lost) {
                self.result = GameResult::Win(winner.id);
            } else {
                self.result = GameResult::Draw;
            }
        }
    }

    pub fn is_legendary(&self, perm: &Permanent) -> bool {
        // Check the card database for supertypes
        // For now, check card_name for known legendaries
        matches!(
            perm.card_name,
            CardName::MoxOpal
                | CardName::SheoldredTheApocalypse
                | CardName::ThaliaGuardianOfThraben
                | CardName::RagavanNimblePilferer
                | CardName::JaceTheMindSculptor
                | CardName::TeferiTimeRaveler
                | CardName::LeovoldEmissaryOfTrest
                | CardName::DackFayden
                | CardName::TolarianAcademy
        )
    }

    // --- Spell Resolution ---

    pub fn resolve_top(&mut self, db: &[CardDef]) {
        if let Some(item) = self.stack.pop() {
            match item.kind {
                StackItemKind::Spell { card_name, card_id } => {
                    self.resolve_spell(card_name, card_id, item.controller, &item.targets, db);
                }
                StackItemKind::TriggeredAbility { effect, .. } => {
                    self.resolve_triggered(effect, item.controller, &item.targets);
                }
                StackItemKind::ActivatedAbility { effect, .. } => {
                    self.resolve_activated(effect, item.controller, &item.targets);
                }
            }
            self.check_state_based_actions();
        }
    }

    fn resolve_spell(
        &mut self,
        card_name: CardName,
        card_id: ObjectId,
        controller: PlayerId,
        targets: &[Target],
        db: &[CardDef],
    ) {
        let card_def = find_card(db, card_name);
        let is_permanent = card_def
            .map(|c| {
                c.card_types.iter().any(|t| matches!(t,
                    CardType::Creature | CardType::Artifact | CardType::Enchantment
                    | CardType::Planeswalker | CardType::Land
                ))
            })
            .unwrap_or(false);

        if is_permanent {
            // Put permanent onto battlefield
            if let Some(def) = card_def {
                let perm = Permanent::new(
                    card_id,
                    card_name,
                    controller,
                    controller,
                    def.power,
                    def.toughness,
                    def.loyalty,
                    def.keywords,
                    def.card_types,
                );
                self.battlefield.push(perm);
                // ETB triggers would go here
                self.handle_etb(card_name, card_id, controller);
            }
        } else {
            // Instant/sorcery: resolve effect, then put in graveyard
            self.resolve_card_effect(card_name, controller, targets, db);
            self.players[controller as usize].graveyard.push(card_id);
        }
    }

    fn resolve_card_effect(
        &mut self,
        card_name: CardName,
        controller: PlayerId,
        targets: &[Target],
        _db: &[CardDef],
    ) {
        match card_name {
            // === Draw spells ===
            CardName::AncestralRecall => {
                let target_player = match targets.first() {
                    Some(Target::Player(p)) => *p,
                    _ => controller,
                };
                self.draw_cards(target_player, 3);
            }

            // === Counterspells ===
            CardName::Counterspell | CardName::ForceOfWill | CardName::ManaDrain => {
                if let Some(Target::Object(spell_id)) = targets.first() {
                    self.stack.remove(*spell_id);
                    // Mana Drain: would add triggered ability for next main phase
                }
            }
            CardName::MentalMisstep => {
                if let Some(Target::Object(spell_id)) = targets.first() {
                    // Should check CMC == 1, but for engine purposes just counter it
                    self.stack.remove(*spell_id);
                }
            }
            CardName::SpellPierce => {
                // Counter unless controller pays {2} - simplified: just counter
                if let Some(Target::Object(spell_id)) = targets.first() {
                    self.stack.remove(*spell_id);
                }
            }

            // === Damage spells ===
            CardName::LightningBolt | CardName::ChainLightning => {
                if let Some(target) = targets.first() {
                    self.deal_damage_to_target(*target, 3, controller);
                }
            }

            // === Removal ===
            CardName::SwordsToPlowshares => {
                if let Some(Target::Object(creature_id)) = targets.first() {
                    if let Some(perm) = self.remove_permanent(*creature_id) {
                        let power = perm.power();
                        self.players[perm.controller as usize].life += power as i32;
                        self.exile.push((perm.id, perm.card_name, perm.owner));
                    }
                }
            }
            CardName::PathToExile => {
                if let Some(Target::Object(creature_id)) = targets.first() {
                    if let Some(perm) = self.remove_permanent(*creature_id) {
                        self.exile.push((perm.id, perm.card_name, perm.owner));
                        // Opponent may search for basic land - simplified: skip
                    }
                }
            }

            // === Mana generation ===
            CardName::DarkRitual => {
                self.players[controller as usize].mana_pool.add(Some(Color::Black), 3);
            }

            // === Extra turns ===
            CardName::TimeWalk => {
                self.players[controller as usize].extra_turns += 1;
            }

            // === Tutors ===
            CardName::DemonicTutor => {
                // In a real implementation, this would present a choice.
                // For search engine: the choice is an action the AI makes.
                let options: Vec<ObjectId> = self.players[controller as usize]
                    .library
                    .clone();
                if !options.is_empty() {
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseFromList {
                            options,
                            reason: ChoiceReason::DemonicTutorSearch,
                        },
                    });
                }
            }
            CardName::VampiricTutor => {
                self.players[controller as usize].life -= 2;
                let options: Vec<ObjectId> = self.players[controller as usize]
                    .library
                    .clone();
                if !options.is_empty() {
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseFromList {
                            options,
                            reason: ChoiceReason::VampiricTutorSearch,
                        },
                    });
                }
            }
            CardName::MysticalTutor => {
                let options: Vec<ObjectId> = self.players[controller as usize]
                    .library
                    .clone();
                if !options.is_empty() {
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseFromList {
                            options,
                            reason: ChoiceReason::MysticalTutorSearch,
                        },
                    });
                }
            }
            CardName::Entomb => {
                let options: Vec<ObjectId> = self.players[controller as usize]
                    .library
                    .clone();
                if !options.is_empty() {
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseFromList {
                            options,
                            reason: ChoiceReason::EntombSearch,
                        },
                    });
                }
            }

            // === Discard ===
            CardName::Thoughtseize => {
                self.players[controller as usize].life -= 2;
                if let Some(Target::Player(target_player)) = targets.first() {
                    let options: Vec<ObjectId> = self.players[*target_player as usize]
                        .hand
                        .clone();
                    if !options.is_empty() {
                        self.pending_choice = Some(PendingChoice {
                            player: controller,
                            kind: ChoiceKind::ChooseFromList {
                                options,
                                reason: ChoiceReason::ThoughtseizeDiscard,
                            },
                        });
                    }
                }
            }
            CardName::HymnToTourach => {
                if let Some(Target::Player(target_player)) = targets.first() {
                    let pid = *target_player as usize;
                    // Discard 2 at random - for deterministic search, pick last 2
                    let count = 2.min(self.players[pid].hand.len());
                    for _ in 0..count {
                        let id = self.players[pid].hand.pop().unwrap();
                        self.players[pid].graveyard.push(id);
                    }
                }
            }

            // === Wheel effects ===
            CardName::WheelOfFortune | CardName::Timetwister => {
                for pid in 0..self.num_players as usize {
                    // Discard hand
                    let hand = std::mem::take(&mut self.players[pid].hand);
                    if card_name == CardName::Timetwister {
                        // Shuffle hand + graveyard into library
                        self.players[pid].library.extend(hand);
                        let gy = std::mem::take(&mut self.players[pid].graveyard);
                        self.players[pid].library.extend(gy);
                    } else {
                        self.players[pid].graveyard.extend(hand);
                    }
                    // Draw 7
                    self.draw_cards(pid as PlayerId, 7);
                }
            }

            // === Cantrips ===
            CardName::Brainstorm => {
                self.draw_cards(controller, 3);
                // Need to put 2 back - this becomes a pending choice
                // Simplified: put last 2 drawn back
                let hand = &mut self.players[controller as usize].hand;
                if hand.len() >= 2 {
                    let c1 = hand.pop().unwrap();
                    let c2 = hand.pop().unwrap();
                    self.players[controller as usize].library.push(c2);
                    self.players[controller as usize].library.push(c1);
                }
            }
            CardName::Ponder => {
                // Simplified: draw a card
                self.draw_cards(controller, 1);
            }
            CardName::Preordain => {
                // Simplified: draw a card
                self.draw_cards(controller, 1);
            }

            // === Board wipes ===
            CardName::Balance => {
                // Find minimum land/creature/hand counts
                let min_lands = (0..self.num_players as usize)
                    .map(|p| self.lands_controlled_by(p as PlayerId).count())
                    .min()
                    .unwrap_or(0);
                let min_creatures = (0..self.num_players as usize)
                    .map(|p| self.creatures_controlled_by(p as PlayerId).count())
                    .min()
                    .unwrap_or(0);
                let min_hand = (0..self.num_players as usize)
                    .map(|p| self.players[p].hand.len())
                    .min()
                    .unwrap_or(0);

                // Sacrifice excess lands
                for pid in 0..self.num_players as usize {
                    let current = self.lands_controlled_by(pid as PlayerId).count();
                    if current > min_lands {
                        let to_sac: Vec<ObjectId> = self
                            .lands_controlled_by(pid as PlayerId)
                            .take(current - min_lands)
                            .map(|p| p.id)
                            .collect();
                        for id in to_sac {
                            if let Some(perm) = self.remove_permanent(id) {
                                self.players[perm.owner as usize].graveyard.push(perm.id);
                            }
                        }
                    }
                }

                // Sacrifice excess creatures
                for pid in 0..self.num_players as usize {
                    let current = self.creatures_controlled_by(pid as PlayerId).count();
                    if current > min_creatures {
                        let to_sac: Vec<ObjectId> = self
                            .creatures_controlled_by(pid as PlayerId)
                            .take(current - min_creatures)
                            .map(|p| p.id)
                            .collect();
                        for id in to_sac {
                            if let Some(perm) = self.remove_permanent(id) {
                                self.players[perm.owner as usize].graveyard.push(perm.id);
                            }
                        }
                    }
                }

                // Discard to min hand size
                for pid in 0..self.num_players as usize {
                    while self.players[pid].hand.len() > min_hand {
                        if let Some(id) = self.players[pid].hand.pop() {
                            self.players[pid].graveyard.push(id);
                        }
                    }
                }
            }

            CardName::Armageddon => {
                let lands: Vec<ObjectId> = self
                    .battlefield
                    .iter()
                    .filter(|p| p.is_land())
                    .map(|p| p.id)
                    .collect();
                for id in lands {
                    if let Some(perm) = self.remove_permanent(id) {
                        self.players[perm.owner as usize].graveyard.push(perm.id);
                    }
                }
            }

            CardName::ToxicDeluge => {
                // Need X life payment - simplified version
                // In real implementation, X is chosen as part of casting
                let x = 3i16; // Default to -3/-3 for now
                for perm in &mut self.battlefield {
                    if perm.is_creature() {
                        perm.toughness_mod -= x;
                        perm.power_mod -= x;
                    }
                }
            }

            CardName::Disenchant => {
                if let Some(Target::Object(target_id)) = targets.first() {
                    if let Some(perm) = self.remove_permanent(*target_id) {
                        self.players[perm.owner as usize].graveyard.push(perm.id);
                    }
                }
            }

            // === Color hosers ===
            CardName::Pyroblast | CardName::RedElementalBlast => {
                if let Some(Target::Object(target_id)) = targets.first() {
                    // Counter if on stack, destroy if permanent - simplified
                    if self.stack.remove(*target_id).is_none() {
                        if let Some(perm) = self.remove_permanent(*target_id) {
                            self.players[perm.owner as usize].graveyard.push(perm.id);
                        }
                    }
                }
            }

            // === Reanimation ===
            CardName::Reanimate => {
                if let Some(Target::Object(target_id)) = targets.first() {
                    // Find card in any graveyard
                    for pid in 0..self.num_players as usize {
                        if let Some(pos) = self.players[pid].graveyard.iter().position(|&id| id == *target_id) {
                            let card_id = self.players[pid].graveyard.remove(pos);
                            let card_name = self.card_name_for_id(card_id);
                            if let Some(cn) = card_name {
                                // TODO: look up proper stats from db
                                let perm = Permanent::new(
                                    card_id, cn, controller, pid as PlayerId,
                                    Some(0), Some(0), None, Keywords::empty(), &[CardType::Creature],
                                );
                                self.battlefield.push(perm);
                                // Lose life equal to CMC - simplified
                                self.players[controller as usize].life -= 5;
                            }
                            break;
                        }
                    }
                }
            }

            // === Yawgmoth's Will ===
            CardName::YawgmothsWill => {
                // This is extremely complex to implement fully.
                // Simplified: let the controller cast one spell from graveyard this turn.
                // Full implementation would need a continuous effect tracking.
            }

            // === Storm ===
            CardName::TendrillsOfAgony => {
                // Base effect: target loses 2, you gain 2
                if let Some(target) = targets.first() {
                    match target {
                        Target::Player(p) => {
                            self.players[*p as usize].life -= 2;
                            self.players[controller as usize].life += 2;
                        }
                        _ => {}
                    }
                }
                // Storm copies
                let storm = self.storm_count;
                for _ in 0..storm {
                    if let Some(target) = targets.first() {
                        match target {
                            Target::Player(p) => {
                                self.players[*p as usize].life -= 2;
                                self.players[controller as usize].life += 2;
                            }
                            _ => {}
                        }
                    }
                }
            }

            // === Channel ===
            CardName::Channel => {
                // Until end of turn, pay 1 life to add {C}.
                // Simplified: convert all life except 1 to colorless mana
                let life = self.players[controller as usize].life;
                if life > 1 {
                    let mana_to_add = (life - 1) as u8;
                    self.players[controller as usize].mana_pool.colorless += mana_to_add;
                    self.players[controller as usize].life = 1;
                }
            }

            // === Regrowth ===
            CardName::Regrowth => {
                if let Some(Target::Object(target_id)) = targets.first() {
                    let gy = &mut self.players[controller as usize].graveyard;
                    if let Some(pos) = gy.iter().position(|&id| id == *target_id) {
                        let card = gy.remove(pos);
                        self.players[controller as usize].hand.push(card);
                    }
                }
            }

            _ => {
                // Unimplemented card effect - no-op
            }
        }
    }

    fn handle_etb(&mut self, card_name: CardName, _card_id: ObjectId, _controller: PlayerId) {
        match card_name {
            // ETB triggers would be queued here
            _ => {}
        }
    }

    fn resolve_triggered(&mut self, effect: TriggeredEffect, controller: PlayerId, targets: &[Target]) {
        match effect {
            TriggeredEffect::ManaCryptUpkeep => {
                // Flip coin - simplified: 50% chance of 3 damage
                // For deterministic search: always deal damage (worst case)
                self.players[controller as usize].life -= 3;
            }
            TriggeredEffect::DealDamage(amount) => {
                if let Some(target) = targets.first() {
                    self.deal_damage_to_target(*target, amount, controller);
                }
            }
            TriggeredEffect::DrawCards(n) => {
                self.draw_cards(controller, n as usize);
            }
            TriggeredEffect::GainLife(amount) => {
                self.players[controller as usize].life += amount;
            }
            TriggeredEffect::LoseLife(amount) => {
                self.players[controller as usize].life -= amount;
            }
            TriggeredEffect::SheoldredDraw => {
                self.players[controller as usize].life += 2;
            }
            TriggeredEffect::SheoldredOpponentDraw => {
                let opp = self.opponent(controller);
                self.players[opp as usize].life -= 2;
            }
            TriggeredEffect::DarkConfidantUpkeep => {
                // Reveal top card, lose life equal to CMC
                if let Some(id) = self.players[controller as usize].library.pop() {
                    self.players[controller as usize].hand.push(id);
                    // Lose life equal to CMC - simplified: lose 2
                    self.players[controller as usize].life -= 2;
                }
            }
            TriggeredEffect::YoungPyromancerCast | TriggeredEffect::MonasteryMentorCast => {
                // Create 1/1 token
                let token_id = self.new_object_id();
                let mut kws = Keywords::empty();
                if matches!(effect, TriggeredEffect::MonasteryMentorCast) {
                    kws.add(Keyword::Prowess);
                }
                let mut token = Permanent::new(
                    token_id,
                    card_name_for_token(),
                    controller,
                    controller,
                    Some(1),
                    Some(1),
                    None,
                    kws,
                    &[CardType::Creature],
                );
                token.is_token = true;
                self.battlefield.push(token);
            }
            TriggeredEffect::CreateTokens { power, toughness, count } => {
                for _ in 0..count {
                    let token_id = self.new_object_id();
                    let mut token = Permanent::new(
                        token_id,
                        card_name_for_token(),
                        controller,
                        controller,
                        Some(power),
                        Some(toughness),
                        None,
                        Keywords::empty(),
                        &[CardType::Creature],
                    );
                    token.is_token = true;
                    self.battlefield.push(token);
                }
            }
            _ => {}
        }
    }

    fn resolve_activated(&mut self, effect: ActivatedEffect, controller: PlayerId, targets: &[Target]) {
        match effect {
            ActivatedEffect::SacrificeForMana { amount: _ } => {
                // Handled at activation time (mana already added, permanent already sacrificed)
            }
            ActivatedEffect::JaceBrainstorm => {
                self.draw_cards(controller, 3);
                // Put 2 back - simplified
                let hand = &mut self.players[controller as usize].hand;
                if hand.len() >= 2 {
                    let c1 = hand.pop().unwrap();
                    let c2 = hand.pop().unwrap();
                    self.players[controller as usize].library.push(c2);
                    self.players[controller as usize].library.push(c1);
                }
            }
            ActivatedEffect::JaceBounce => {
                if let Some(Target::Object(creature_id)) = targets.first() {
                    if let Some(perm) = self.remove_permanent(*creature_id) {
                        self.players[perm.owner as usize].hand.push(perm.id);
                    }
                }
            }
            ActivatedEffect::JaceFateseal => {
                // +2: Look at top of target player's library, may put on bottom
                // Simplified: no-op for now (hidden info)
            }
            ActivatedEffect::TeferiBounce => {
                if let Some(Target::Object(target_id)) = targets.first() {
                    if let Some(perm) = self.remove_permanent(*target_id) {
                        self.players[perm.owner as usize].hand.push(perm.id);
                    }
                }
                self.draw_cards(controller, 1);
            }
            ActivatedEffect::DrawCards(n) => {
                self.draw_cards(controller, n as usize);
            }
            _ => {}
        }
    }

    // --- Utility ---

    pub fn draw_cards(&mut self, player: PlayerId, count: usize) {
        let pid = player as usize;
        for _ in 0..count {
            if let Some(id) = self.players[pid].library.pop() {
                self.players[pid].hand.push(id);
            } else {
                // Can't draw from empty library - player loses
                self.players[pid].has_lost = true;
            }
        }
    }

    fn deal_damage_to_target(&mut self, target: Target, amount: u16, _source_controller: PlayerId) {
        match target {
            Target::Player(p) => {
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
}

/// Placeholder card name for tokens (they don't have real card names).
fn card_name_for_token() -> CardName {
    CardName::Plains // Placeholder - tokens would need their own system
}

// We need to add the card_registry field to GameState
