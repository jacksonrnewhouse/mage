/// Move generation: enumerate all legal actions for the priority player.
/// This is the primary interface between the game engine and search algorithms.

use crate::action::*;
use crate::card::*;
use crate::game::*;
use crate::mana::*;
use crate::stack::*;
use crate::types::*;

impl GameState {
    /// Generate all legal actions for the current priority player.
    /// This is the key method for search algorithms.
    pub fn legal_actions(&self, db: &[CardDef]) -> Vec<Action> {
        if self.is_terminal() {
            return vec![];
        }

        // If there's a pending choice, generate choice actions
        if let Some(ref choice) = self.pending_choice {
            return self.generate_choice_actions(choice);
        }

        match self.action_context {
            ActionContext::Priority => self.generate_priority_actions(db),
            ActionContext::DeclareAttackers => self.generate_attacker_actions(),
            ActionContext::DeclareBlockers => self.generate_blocker_actions(),
            ActionContext::MakingChoice => vec![Action::PassPriority],
        }
    }

    fn generate_choice_actions(&self, choice: &PendingChoice) -> Vec<Action> {
        match &choice.kind {
            ChoiceKind::ChooseFromList { options, .. } => {
                options.iter().map(|&id| Action::ChooseCard(id)).collect()
            }
            ChoiceKind::ChooseColor { .. } => {
                Color::ALL.iter().map(|&c| Action::ChooseColor(c)).collect()
            }
            ChoiceKind::ChooseNumber { min, max, .. } => {
                // For search, limit the number of options to avoid explosion
                let step = if *max - *min > 20 { (*max - *min) / 10 } else { 1 };
                let mut actions = Vec::new();
                let mut n = *min;
                while n <= *max {
                    actions.push(Action::ChooseNumber(n));
                    n += step;
                }
                if actions.last().map(|a| matches!(a, Action::ChooseNumber(x) if *x != *max)).unwrap_or(true) {
                    actions.push(Action::ChooseNumber(*max));
                }
                actions
            }
        }
    }

    fn generate_priority_actions(&self, db: &[CardDef]) -> Vec<Action> {
        let mut actions = Vec::with_capacity(16);
        let player_id = self.priority_player;
        let player = &self.players[player_id as usize];

        // Can always pass priority
        actions.push(Action::PassPriority);

        // Can always concede
        actions.push(Action::Concede);

        let is_main_phase = matches!(self.phase, Phase::PreCombatMain | Phase::PostCombatMain);
        let stack_empty = self.stack.is_empty();
        let is_active = player_id == self.active_player;
        let sorcery_speed = is_main_phase && stack_empty && is_active;

        // --- Play a land (sorcery speed, one per turn) ---
        if sorcery_speed && player.land_plays_remaining > 0 {
            for &card_id in &player.hand {
                if let Some(card_name) = self.card_name_for_id(card_id) {
                    if let Some(def) = find_card(db, card_name) {
                        if def.card_types.contains(&CardType::Land) {
                            actions.push(Action::PlayLand(card_id));
                        }
                    }
                }
            }
        }

        // --- Cast spells from hand ---
        for &card_id in &player.hand {
            if let Some(card_name) = self.card_name_for_id(card_id) {
                if let Some(def) = find_card(db, card_name) {
                    // Skip lands (handled above)
                    if def.card_types.contains(&CardType::Land) {
                        continue;
                    }

                    // Check timing
                    let can_cast_at_instant_speed = def.card_types.contains(&CardType::Instant)
                        || def.keywords.has(Keyword::Flash);
                    let can_cast = if can_cast_at_instant_speed {
                        true // Can cast instants anytime with priority
                    } else {
                        sorcery_speed
                    };

                    if !can_cast {
                        continue;
                    }

                    // Check mana cost (including Thalia tax, etc.)
                    let effective_cost = self.effective_cost(def, player_id);
                    if !player.mana_pool.can_pay(&effective_cost) {
                        // Can't pay - but first check if we could tap lands to get mana
                        // For search, we generate mana ability actions separately
                        continue;
                    }

                    // Generate target permutations
                    let target_sets = self.generate_targets(card_name, player_id, db);
                    if target_sets.is_empty() {
                        actions.push(Action::CastSpell {
                            card_id,
                            targets: vec![],
                        });
                    } else {
                        for targets in target_sets {
                            actions.push(Action::CastSpell {
                                card_id,
                                targets,
                            });
                        }
                    }
                }
            }
        }

        // --- Force of Will alternate cost ---
        for &card_id in &player.hand {
            if let Some(card_name) = self.card_name_for_id(card_id) {
                if card_name == CardName::ForceOfWill && !self.stack.is_empty() {
                    // Check if player has another blue card in hand and 1 life
                    let has_blue_card = player.hand.iter().any(|&other_id| {
                        other_id != card_id
                            && self
                                .card_name_for_id(other_id)
                                .and_then(|cn| find_card(db, cn))
                                .map(|d| d.color_identity.contains(&Color::Blue))
                                .unwrap_or(false)
                    });
                    if has_blue_card && player.life > 1 {
                        // Target each spell on the stack
                        for item in self.stack.items() {
                            actions.push(Action::CastSpell {
                                card_id,
                                targets: vec![Target::Object(item.id)],
                            });
                        }
                    }
                }
            }
        }

        // --- Activate mana abilities (tap lands/moxen for mana) ---
        for perm in self.permanents_controlled_by(player_id) {
            if perm.tapped {
                continue;
            }
            let mana_options = self.mana_ability_options(perm);
            for color in mana_options {
                actions.push(Action::ActivateManaAbility {
                    permanent_id: perm.id,
                    color_choice: color,
                });
            }
        }

        // --- Activate non-mana abilities ---
        for perm in self.permanents_controlled_by(player_id) {
            let abilities = self.activatable_abilities(perm, sorcery_speed, db);
            for (idx, targets) in abilities {
                actions.push(Action::ActivateAbility {
                    permanent_id: perm.id,
                    ability_index: idx,
                    targets,
                });
            }
        }

        actions
    }

    fn generate_attacker_actions(&self) -> Vec<Action> {
        let mut actions = Vec::new();

        // Can always confirm (done declaring attackers, even with 0)
        actions.push(Action::ConfirmAttackers);

        // Can declare each eligible creature as attacker
        let defending_player = self.opponent(self.active_player);
        for perm in self.permanents_controlled_by(self.active_player) {
            if perm.can_attack() {
                // Check if already declared as attacker
                if !self.attackers.iter().any(|(id, _)| *id == perm.id) {
                    actions.push(Action::DeclareAttacker {
                        creature_id: perm.id,
                    });
                }
            }
        }

        // For search optimization: also allow declaring all attackers at once
        // by treating each DeclareAttacker as additive.
        let _ = defending_player;

        actions
    }

    fn generate_blocker_actions(&self) -> Vec<Action> {
        let mut actions = Vec::new();

        // Can always confirm (done declaring blockers)
        actions.push(Action::ConfirmBlockers);

        let blocking_player = self.opponent(self.active_player);
        for perm in self.permanents_controlled_by(blocking_player) {
            if !perm.can_block() {
                continue;
            }
            // Already blocking something?
            if self.blockers.iter().any(|(bid, _)| *bid == perm.id) {
                continue;
            }
            // Can block each attacker
            for &(attacker_id, _) in &self.attackers {
                if let Some(attacker) = self.find_permanent(attacker_id) {
                    let can_block = if attacker.keywords.has(Keyword::Flying) {
                        perm.can_block_flyer()
                    } else {
                        true
                    };
                    // Menace: must be blocked by 2+ creatures (simplified: allow single block)
                    if can_block {
                        actions.push(Action::DeclareBlocker {
                            blocker_id: perm.id,
                            attacker_id,
                        });
                    }
                }
            }
        }

        actions
    }

    /// Get the effective mana cost of a card after tax effects (Thalia, Trinisphere, etc.)
    fn effective_cost(&self, def: &CardDef, _controller: PlayerId) -> ManaCost {
        let mut cost = def.mana_cost;

        // Thalia tax: noncreature spells cost {1} more
        let thalia_active = self.battlefield.iter().any(|p| {
            p.card_name == CardName::ThaliaGuardianOfThraben
                && p.controller != _controller
        });
        if thalia_active && !def.card_types.contains(&CardType::Creature) {
            cost.generic += 1;
        }

        // Lodestone Golem: nonartifact spells cost {1} more
        let lodestone_active = self.battlefield.iter().any(|p| {
            p.card_name == CardName::LodestoneGolem
                && p.controller != _controller
        });
        if lodestone_active && !def.card_types.contains(&CardType::Artifact) {
            cost.generic += 1;
        }

        // Trinisphere: spells cost at least {3} (when untapped)
        let trinisphere_active = self.battlefield.iter().any(|p| {
            p.card_name == CardName::Trinisphere && !p.tapped
        });
        if trinisphere_active && cost.cmc() < 3 {
            cost.generic = 3 - (cost.cmc() - cost.generic);
        }

        cost
    }

    /// What mana can a permanent produce?
    fn mana_ability_options(&self, perm: &crate::permanent::Permanent) -> Vec<Option<Color>> {
        match perm.card_name {
            // Basic lands
            CardName::Plains => vec![Some(Color::White)],
            CardName::Island => vec![Some(Color::Blue)],
            CardName::Swamp => vec![Some(Color::Black)],
            CardName::Mountain => vec![Some(Color::Red)],
            CardName::Forest => vec![Some(Color::Green)],

            // Dual lands (two options)
            CardName::UndergroundSea => vec![Some(Color::Blue), Some(Color::Black)],
            CardName::VolcanicIsland => vec![Some(Color::Blue), Some(Color::Red)],
            CardName::Tundra => vec![Some(Color::White), Some(Color::Blue)],
            CardName::TropicalIsland => vec![Some(Color::Blue), Some(Color::Green)],
            CardName::Badlands => vec![Some(Color::Black), Some(Color::Red)],
            CardName::Bayou => vec![Some(Color::Black), Some(Color::Green)],
            CardName::Plateau => vec![Some(Color::Red), Some(Color::White)],
            CardName::Savannah => vec![Some(Color::Green), Some(Color::White)],
            CardName::Scrubland => vec![Some(Color::White), Some(Color::Black)],
            CardName::Taiga => vec![Some(Color::Red), Some(Color::Green)],

            // Moxen
            CardName::MoxPearl => vec![Some(Color::White)],
            CardName::MoxSapphire => vec![Some(Color::Blue)],
            CardName::MoxJet => vec![Some(Color::Black)],
            CardName::MoxRuby => vec![Some(Color::Red)],
            CardName::MoxEmerald => vec![Some(Color::Green)],

            // Sol Ring, Mana Crypt, Ancient Tomb: produce colorless
            CardName::SolRing | CardName::ManaCrypt | CardName::AncientTomb => vec![None],

            // Mana Vault, Grim Monolith: produce colorless (but may not untap)
            CardName::ManaVault | CardName::GrimMonolith => vec![None],

            // Library of Alexandria: colorless
            CardName::LibraryOfAlexandria => vec![None],

            // Strip Mine, Wasteland: colorless
            CardName::StripMine | CardName::Wasteland => vec![None],

            // Birds of Paradise: any color
            CardName::BirdsOfParadise => vec![
                Some(Color::White),
                Some(Color::Blue),
                Some(Color::Black),
                Some(Color::Red),
                Some(Color::Green),
            ],

            // Mishra's Workshop: colorless (3, but only for artifacts)
            CardName::MishrasWorkshop => vec![None],

            // Tolarian Academy: blue per artifact
            CardName::TolarianAcademy => {
                if self.artifacts_controlled_by(perm.controller).count() > 0 {
                    vec![Some(Color::Blue)]
                } else {
                    vec![]
                }
            }

            _ => vec![],
        }
    }

    /// Apply a mana ability (tap for mana). Returns true if successful.
    pub fn activate_mana_ability(&mut self, permanent_id: ObjectId, color_choice: Option<Color>) -> bool {
        let perm = match self.find_permanent(permanent_id) {
            Some(p) => p,
            None => return false,
        };
        if perm.tapped {
            return false;
        }
        let controller = perm.controller;
        let card_name = perm.card_name;

        // Collector Ouphe: activated abilities of artifacts can't be activated
        if perm.is_artifact() {
            let ouphe_active = self.battlefield.iter().any(|p| {
                p.card_name == CardName::CollectorOuphe
            });
            if ouphe_active {
                return false;
            }
        }

        match card_name {
            // Basics and duals: tap for 1 of the chosen color
            CardName::Plains
            | CardName::Island
            | CardName::Swamp
            | CardName::Mountain
            | CardName::Forest
            | CardName::UndergroundSea
            | CardName::VolcanicIsland
            | CardName::Tundra
            | CardName::TropicalIsland
            | CardName::Badlands
            | CardName::Bayou
            | CardName::Plateau
            | CardName::Savannah
            | CardName::Scrubland
            | CardName::Taiga
            | CardName::MoxPearl
            | CardName::MoxSapphire
            | CardName::MoxJet
            | CardName::MoxRuby
            | CardName::MoxEmerald
            | CardName::BirdsOfParadise => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.tapped = true;
                }
                self.players[controller as usize]
                    .mana_pool
                    .add(color_choice, 1);
                true
            }

            // Sol Ring: {T} for {C}{C}
            CardName::SolRing => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.tapped = true;
                }
                self.players[controller as usize].mana_pool.colorless += 2;
                true
            }

            // Mana Crypt: {T} for {C}{C}
            CardName::ManaCrypt => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.tapped = true;
                }
                self.players[controller as usize].mana_pool.colorless += 2;
                true
            }

            // Mana Vault: {T} for {C}{C}{C}
            CardName::ManaVault | CardName::GrimMonolith => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.tapped = true;
                }
                self.players[controller as usize].mana_pool.colorless += 3;
                true
            }

            // Ancient Tomb: {T} for {C}{C}, deals 2 to you
            CardName::AncientTomb => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.tapped = true;
                }
                self.players[controller as usize].mana_pool.colorless += 2;
                self.players[controller as usize].life -= 2;
                true
            }

            // Strip Mine / Wasteland: {T} for {C}
            CardName::StripMine | CardName::Wasteland | CardName::LibraryOfAlexandria => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.tapped = true;
                }
                self.players[controller as usize].mana_pool.colorless += 1;
                true
            }

            // Mishra's Workshop: {T} for {C}{C}{C} (only for artifacts - enforced at cast time)
            CardName::MishrasWorkshop => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.tapped = true;
                }
                self.players[controller as usize].mana_pool.colorless += 3;
                true
            }

            // Tolarian Academy: {T} for {U} per artifact
            CardName::TolarianAcademy => {
                let artifact_count = self.artifacts_controlled_by(controller).count() as u8;
                if artifact_count == 0 {
                    return false;
                }
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.tapped = true;
                }
                self.players[controller as usize]
                    .mana_pool
                    .add(Some(Color::Blue), artifact_count);
                true
            }

            _ => false,
        }
    }

    /// Non-mana activated abilities available on a permanent.
    fn activatable_abilities(
        &self,
        perm: &crate::permanent::Permanent,
        sorcery_speed: bool,
        _db: &[CardDef],
    ) -> Vec<(u8, Vec<Target>)> {
        let mut abilities = Vec::new();

        // Sacrifice abilities (Black Lotus, Lotus Petal, Lion's Eye Diamond)
        match perm.card_name {
            CardName::BlackLotus if !perm.tapped => {
                // Sacrifice for 3 mana of any color - generates color choice
                for &_color in &Color::ALL {
                    abilities.push((0, vec![Target::Player(perm.controller)]));
                }
            }
            CardName::LotusPetal if !perm.tapped => {
                abilities.push((0, vec![]));
            }
            CardName::LionEyeDiamond if !perm.tapped => {
                abilities.push((0, vec![]));
            }
            _ => {}
        }

        // Fetch lands
        match perm.card_name {
            CardName::FloodedStrand
            | CardName::PollutedDelta
            | CardName::BloodstainedMire
            | CardName::WoodedFoothills
            | CardName::WindsweptHeath
            | CardName::MistyRainforest
            | CardName::ScaldingTarn
            | CardName::VerdantCatacombs
            | CardName::AridMesa
            | CardName::MarshFlats
                if !perm.tapped =>
            {
                abilities.push((0, vec![]));
            }
            _ => {}
        }

        // Strip Mine / Wasteland: destroy target land
        match perm.card_name {
            CardName::StripMine if !perm.tapped => {
                for target_perm in &self.battlefield {
                    if target_perm.is_land() && target_perm.id != perm.id {
                        abilities.push((1, vec![Target::Object(target_perm.id)]));
                    }
                }
            }
            CardName::Wasteland if !perm.tapped => {
                for target_perm in &self.battlefield {
                    if target_perm.is_land() && !self.is_basic_land(target_perm) && target_perm.id != perm.id {
                        abilities.push((1, vec![Target::Object(target_perm.id)]));
                    }
                }
            }
            _ => {}
        }

        // Planeswalker abilities (sorcery speed only)
        if sorcery_speed && perm.is_planeswalker() && !perm.loyalty_activated_this_turn {
            match perm.card_name {
                CardName::JaceTheMindSculptor => {
                    // +2: Fateseal
                    if perm.loyalty >= 0 {
                        abilities.push((0, vec![Target::Player(self.opponent(perm.controller))]));
                    }
                    // 0: Brainstorm
                    abilities.push((1, vec![]));
                    // -1: Bounce creature
                    if perm.loyalty >= 1 {
                        for target in &self.battlefield {
                            if target.is_creature() {
                                abilities.push((2, vec![Target::Object(target.id)]));
                            }
                        }
                    }
                }
                CardName::TeferiTimeRaveler => {
                    // +1: Flash for sorceries
                    abilities.push((0, vec![]));
                    // -3: Bounce + draw
                    if perm.loyalty >= 3 {
                        for target in &self.battlefield {
                            if target.is_artifact() || target.is_creature() || target.is_enchantment() {
                                abilities.push((1, vec![Target::Object(target.id)]));
                            }
                        }
                    }
                }
                CardName::DackFayden => {
                    // +1: Target player draws 2, discards 2
                    abilities.push((0, vec![Target::Player(perm.controller)]));
                    // -2: Steal artifact
                    if perm.loyalty >= 2 {
                        for target in &self.battlefield {
                            if target.is_artifact() && target.controller != perm.controller {
                                abilities.push((1, vec![Target::Object(target.id)]));
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        abilities
    }

    fn is_basic_land(&self, perm: &crate::permanent::Permanent) -> bool {
        matches!(
            perm.card_name,
            CardName::Plains
                | CardName::Island
                | CardName::Swamp
                | CardName::Mountain
                | CardName::Forest
        )
    }

    /// Generate valid target sets for a spell.
    fn generate_targets(
        &self,
        card_name: CardName,
        controller: PlayerId,
        _db: &[CardDef],
    ) -> Vec<Vec<Target>> {
        match card_name {
            // Target any player or creature
            CardName::LightningBolt | CardName::ChainLightning => {
                let mut targets = Vec::new();
                for pid in 0..self.num_players {
                    targets.push(vec![Target::Player(pid)]);
                }
                for perm in &self.battlefield {
                    if perm.is_creature() {
                        targets.push(vec![Target::Object(perm.id)]);
                    }
                }
                targets
            }

            // Target creature
            CardName::SwordsToPlowshares | CardName::PathToExile => {
                self.battlefield
                    .iter()
                    .filter(|p| p.is_creature())
                    .map(|p| vec![Target::Object(p.id)])
                    .collect()
            }

            // Target spell on stack
            CardName::Counterspell | CardName::ManaDrain | CardName::MentalMisstep => {
                self.stack
                    .items()
                    .iter()
                    .map(|item| vec![Target::Object(item.id)])
                    .collect()
            }

            // Target player (for Ancestral Recall)
            CardName::AncestralRecall => {
                (0..self.num_players)
                    .map(|pid| vec![Target::Player(pid)])
                    .collect()
            }

            // Target player (for discard)
            CardName::Thoughtseize => {
                vec![vec![Target::Player(self.opponent(controller))]]
            }
            CardName::HymnToTourach => {
                vec![vec![Target::Player(self.opponent(controller))]]
            }

            // Target opponent (for Tendrils)
            CardName::TendrillsOfAgony => {
                vec![vec![Target::Player(self.opponent(controller))]]
            }

            // Target artifact or enchantment
            CardName::Disenchant => {
                self.battlefield
                    .iter()
                    .filter(|p| p.is_artifact() || p.is_enchantment())
                    .map(|p| vec![Target::Object(p.id)])
                    .collect()
            }

            // Target creature in any graveyard
            CardName::Reanimate => {
                let mut targets = Vec::new();
                for pid in 0..self.num_players as usize {
                    for &id in &self.players[pid].graveyard {
                        // Would need to check if it's a creature card
                        targets.push(vec![Target::Object(id)]);
                    }
                }
                targets
            }

            // Target card in own graveyard
            CardName::Regrowth => {
                self.players[controller as usize]
                    .graveyard
                    .iter()
                    .map(|&id| vec![Target::Object(id)])
                    .collect()
            }

            // Blue/red hosers
            CardName::Pyroblast | CardName::RedElementalBlast => {
                let mut targets = Vec::new();
                // Target blue permanent
                for perm in &self.battlefield {
                    // Simplified: would need to check color
                    targets.push(vec![Target::Object(perm.id)]);
                }
                // Target blue spell on stack
                for item in self.stack.items() {
                    targets.push(vec![Target::Object(item.id)]);
                }
                targets
            }

            // No targets needed
            _ => vec![],
        }
    }

    /// Apply an action to the game state. Mutates self.
    /// This is the other key method for search algorithms.
    pub fn apply_action(&mut self, action: &Action, db: &[CardDef]) {
        match action {
            Action::PassPriority => {
                self.pass_priority(db);
            }

            Action::Concede => {
                self.players[self.priority_player as usize].has_lost = true;
                self.check_state_based_actions();
            }

            Action::PlayLand(card_id) => {
                let player_id = self.priority_player;
                let card_name = self.card_name_for_id(*card_id);
                if self.players[player_id as usize].remove_from_hand(*card_id) {
                    self.players[player_id as usize].land_plays_remaining -= 1;
                    if let Some(cn) = card_name {
                        if let Some(def) = find_card(db, cn) {
                            let perm = Permanent::new(
                                *card_id,
                                cn,
                                player_id,
                                player_id,
                                def.power,
                                def.toughness,
                                def.loyalty,
                                def.keywords,
                                def.card_types,
                            );
                            self.battlefield.push(perm);
                        }
                    }
                }
            }

            Action::CastSpell { card_id, targets } => {
                let player_id = self.priority_player;
                let card_name = self.card_name_for_id(*card_id);
                if let Some(cn) = card_name {
                    if let Some(def) = find_card(db, cn) {
                        let cost = self.effective_cost(def, player_id);
                        if self.players[player_id as usize].mana_pool.pay(&cost) {
                            self.players[player_id as usize].remove_from_hand(*card_id);
                            self.stack.push(
                                StackItemKind::Spell {
                                    card_name: cn,
                                    card_id: *card_id,
                                },
                                player_id,
                                targets.clone(),
                            );
                            self.players[player_id as usize].spells_cast_this_turn += 1;
                            self.storm_count += 1;
                            self.reset_priority_passes();
                        }
                    }
                }
            }

            Action::ActivateManaAbility {
                permanent_id,
                color_choice,
            } => {
                self.activate_mana_ability(*permanent_id, *color_choice);
            }

            Action::ActivateAbility {
                permanent_id,
                ability_index,
                targets,
            } => {
                self.apply_activated_ability(*permanent_id, *ability_index, targets, db);
            }

            Action::DeclareAttacker { creature_id } => {
                let defending_player = self.opponent(self.active_player);
                self.attackers.push((*creature_id, defending_player));
                if let Some(perm) = self.find_permanent_mut(*creature_id) {
                    if !perm.keywords.has(Keyword::Vigilance) {
                        perm.tapped = true;
                    }
                    perm.attacked_this_turn = true;
                }
            }

            Action::ConfirmAttackers => {
                self.action_context = ActionContext::Priority;
                self.advance_phase(); // Move to declare blockers step
            }

            Action::DeclareBlocker {
                blocker_id,
                attacker_id,
            } => {
                self.blockers.push((*blocker_id, *attacker_id));
            }

            Action::ConfirmBlockers => {
                self.action_context = ActionContext::Priority;
                self.advance_phase(); // Move to combat damage
            }

            Action::ChooseCard(card_id) => {
                if let Some(choice) = self.pending_choice.take() {
                    self.resolve_choice(choice, *card_id, db);
                }
            }

            Action::ChooseColor(color) => {
                if let Some(choice) = self.pending_choice.take() {
                    self.resolve_color_choice(choice, *color);
                }
            }

            Action::ChooseNumber(n) => {
                if let Some(_choice) = self.pending_choice.take() {
                    // Handle number choice (e.g., Toxic Deluge X value)
                    let _ = n;
                }
            }
        }
    }

    fn apply_activated_ability(
        &mut self,
        permanent_id: ObjectId,
        ability_index: u8,
        targets: &[Target],
        _db: &[CardDef],
    ) {
        let perm = match self.find_permanent(permanent_id) {
            Some(p) => p,
            None => return,
        };
        let card_name = perm.card_name;
        let controller = perm.controller;

        match card_name {
            CardName::BlackLotus => {
                // Sacrifice + add 3 mana of any color
                // Color is chosen via follow-up ChooseColor action
                if let Some(perm) = self.remove_permanent(permanent_id) {
                    self.players[perm.owner as usize].graveyard.push(perm.id);
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseColor {
                            reason: ChoiceReason::BlackLotusColor,
                        },
                    });
                }
            }

            CardName::LotusPetal => {
                if let Some(perm) = self.remove_permanent(permanent_id) {
                    self.players[perm.owner as usize].graveyard.push(perm.id);
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseColor {
                            reason: ChoiceReason::LotusPetalColor,
                        },
                    });
                }
            }

            CardName::LionEyeDiamond => {
                // Discard hand, sacrifice, add 3 mana of any color
                let hand = std::mem::take(&mut self.players[controller as usize].hand);
                self.players[controller as usize].graveyard.extend(hand);
                if let Some(perm) = self.remove_permanent(permanent_id) {
                    self.players[perm.owner as usize].graveyard.push(perm.id);
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseColor {
                            reason: ChoiceReason::BlackLotusColor, // Same effect
                        },
                    });
                }
            }

            // Fetch lands
            CardName::FloodedStrand
            | CardName::PollutedDelta
            | CardName::BloodstainedMire
            | CardName::WoodedFoothills
            | CardName::WindsweptHeath
            | CardName::MistyRainforest
            | CardName::ScaldingTarn
            | CardName::VerdantCatacombs
            | CardName::AridMesa
            | CardName::MarshFlats => {
                self.players[controller as usize].life -= 1;
                if let Some(perm) = self.remove_permanent(permanent_id) {
                    self.players[perm.owner as usize].graveyard.push(perm.id);
                }
                // Search for appropriate land - present as choice
                let searchable: Vec<ObjectId> = self.players[controller as usize]
                    .library
                    .iter()
                    .filter(|&&id| {
                        self.card_name_for_id(id)
                            .map(|cn| self.is_fetchable(card_name, cn))
                            .unwrap_or(false)
                    })
                    .copied()
                    .collect();
                if !searchable.is_empty() {
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseFromList {
                            options: searchable,
                            reason: ChoiceReason::GenericSearch,
                        },
                    });
                }
            }

            // Strip Mine: destroy target land
            CardName::StripMine if ability_index == 1 => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.tapped = true;
                }
                // Sacrifice Strip Mine
                if let Some(perm) = self.remove_permanent(permanent_id) {
                    self.players[perm.owner as usize].graveyard.push(perm.id);
                }
                // Destroy target land
                if let Some(Target::Object(target_id)) = targets.first() {
                    if let Some(target) = self.remove_permanent(*target_id) {
                        self.players[target.owner as usize].graveyard.push(target.id);
                    }
                }
            }

            // Wasteland: destroy target nonbasic land
            CardName::Wasteland if ability_index == 1 => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.tapped = true;
                }
                if let Some(perm) = self.remove_permanent(permanent_id) {
                    self.players[perm.owner as usize].graveyard.push(perm.id);
                }
                if let Some(Target::Object(target_id)) = targets.first() {
                    if let Some(target) = self.remove_permanent(*target_id) {
                        self.players[target.owner as usize].graveyard.push(target.id);
                    }
                }
            }

            // Planeswalker abilities
            CardName::JaceTheMindSculptor => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.loyalty_activated_this_turn = true;
                    match ability_index {
                        0 => {
                            perm.loyalty += 2;
                            // Fateseal - simplified no-op
                        }
                        1 => {
                            // Brainstorm
                            let effect = ActivatedEffect::JaceBrainstorm;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        2 => {
                            perm.loyalty -= 1;
                            let effect = ActivatedEffect::JaceBounce;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        _ => {}
                    }
                }
            }

            CardName::TeferiTimeRaveler => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.loyalty_activated_this_turn = true;
                    match ability_index {
                        0 => {
                            perm.loyalty += 1;
                            // +1: sorceries have flash - would need continuous effect
                        }
                        1 => {
                            perm.loyalty -= 3;
                            let effect = ActivatedEffect::TeferiBounce;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        _ => {}
                    }
                }
            }

            _ => {}
        }
    }

    fn is_fetchable(&self, fetch: CardName, target: CardName) -> bool {
        match fetch {
            CardName::FloodedStrand => matches!(target, CardName::Plains | CardName::Island | CardName::Tundra | CardName::Savannah | CardName::Scrubland | CardName::UndergroundSea | CardName::VolcanicIsland | CardName::TropicalIsland),
            CardName::PollutedDelta => matches!(target, CardName::Island | CardName::Swamp | CardName::UndergroundSea | CardName::TropicalIsland | CardName::VolcanicIsland | CardName::Badlands | CardName::Bayou | CardName::Tundra),
            CardName::BloodstainedMire => matches!(target, CardName::Swamp | CardName::Mountain | CardName::Badlands | CardName::UndergroundSea | CardName::Bayou | CardName::VolcanicIsland | CardName::Plateau | CardName::Taiga),
            CardName::WoodedFoothills => matches!(target, CardName::Mountain | CardName::Forest | CardName::Taiga | CardName::VolcanicIsland | CardName::Plateau | CardName::Badlands | CardName::Bayou | CardName::Savannah | CardName::TropicalIsland),
            CardName::WindsweptHeath => matches!(target, CardName::Forest | CardName::Plains | CardName::Savannah | CardName::TropicalIsland | CardName::Bayou | CardName::Taiga | CardName::Tundra | CardName::Plateau | CardName::Scrubland),
            CardName::MistyRainforest => matches!(target, CardName::Forest | CardName::Island | CardName::TropicalIsland | CardName::Bayou | CardName::Savannah | CardName::Taiga | CardName::UndergroundSea | CardName::VolcanicIsland | CardName::Tundra),
            CardName::ScaldingTarn => matches!(target, CardName::Island | CardName::Mountain | CardName::VolcanicIsland | CardName::UndergroundSea | CardName::TropicalIsland | CardName::Tundra | CardName::Badlands | CardName::Plateau | CardName::Taiga),
            CardName::VerdantCatacombs => matches!(target, CardName::Swamp | CardName::Forest | CardName::Bayou | CardName::UndergroundSea | CardName::Badlands | CardName::TropicalIsland | CardName::Savannah | CardName::Taiga),
            CardName::AridMesa => matches!(target, CardName::Mountain | CardName::Plains | CardName::Plateau | CardName::VolcanicIsland | CardName::Badlands | CardName::Taiga | CardName::Tundra | CardName::Savannah | CardName::Scrubland),
            CardName::MarshFlats => matches!(target, CardName::Plains | CardName::Swamp | CardName::Scrubland | CardName::Tundra | CardName::Savannah | CardName::Plateau | CardName::UndergroundSea | CardName::Badlands | CardName::Bayou),
            _ => false,
        }
    }

    fn resolve_choice(&mut self, choice: PendingChoice, card_id: ObjectId, db: &[CardDef]) {
        match choice.kind {
            ChoiceKind::ChooseFromList { reason, .. } => {
                match reason {
                    ChoiceReason::DemonicTutorSearch => {
                        // Move chosen card from library to hand
                        let pid = choice.player as usize;
                        if let Some(pos) = self.players[pid].library.iter().position(|&id| id == card_id) {
                            self.players[pid].library.remove(pos);
                            self.players[pid].hand.push(card_id);
                        }
                    }
                    ChoiceReason::VampiricTutorSearch | ChoiceReason::MysticalTutorSearch => {
                        // Move chosen card to top of library
                        let pid = choice.player as usize;
                        if let Some(pos) = self.players[pid].library.iter().position(|&id| id == card_id) {
                            self.players[pid].library.remove(pos);
                            self.players[pid].library.push(card_id); // push = top of library
                        }
                    }
                    ChoiceReason::EntombSearch => {
                        // Move chosen card from library to graveyard
                        let pid = choice.player as usize;
                        if let Some(pos) = self.players[pid].library.iter().position(|&id| id == card_id) {
                            self.players[pid].library.remove(pos);
                            self.players[pid].graveyard.push(card_id);
                        }
                    }
                    ChoiceReason::ThoughtseizeDiscard => {
                        // Discard chosen card from opponent's hand
                        let target_player = self.opponent(choice.player) as usize;
                        if let Some(pos) = self.players[target_player].hand.iter().position(|&id| id == card_id) {
                            self.players[target_player].hand.remove(pos);
                            self.players[target_player].graveyard.push(card_id);
                        }
                    }
                    ChoiceReason::GenericSearch => {
                        // Fetch land: put onto battlefield
                        let pid = choice.player as usize;
                        if let Some(pos) = self.players[pid].library.iter().position(|&id| id == card_id) {
                            self.players[pid].library.remove(pos);
                            let card_name = self.card_name_for_id(card_id);
                            if let Some(cn) = card_name {
                                if let Some(def) = find_card(db, cn) {
                                    let perm = Permanent::new(
                                        card_id, cn, choice.player, choice.player,
                                        def.power, def.toughness, def.loyalty, def.keywords, def.card_types,
                                    );
                                    self.battlefield.push(perm);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn resolve_color_choice(&mut self, choice: PendingChoice, color: Color) {
        match choice.kind {
            ChoiceKind::ChooseColor { reason } => {
                match reason {
                    ChoiceReason::BlackLotusColor => {
                        self.players[choice.player as usize]
                            .mana_pool
                            .add(Some(color), 3);
                    }
                    ChoiceReason::LotusPetalColor => {
                        self.players[choice.player as usize]
                            .mana_pool
                            .add(Some(color), 1);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

use crate::permanent::Permanent;
