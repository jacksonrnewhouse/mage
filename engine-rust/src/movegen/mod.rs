/// Move generation: enumerate all legal actions for the priority player.
/// This is the primary interface between the game engine and search algorithms.

mod casting;
mod choices;

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

                    // --- Cast-restriction statics ---
                    // Archon of Emeria: each player can cast only one spell per turn
                    let archon_active = self.battlefield.iter().any(|p| {
                        p.card_name == CardName::ArchonOfEmeria
                    });
                    if archon_active && player.spells_cast_this_turn >= 1 {
                        continue;
                    }

                    // Ethersworn Canonist: each player can cast only one nonartifact spell per turn
                    let canonist_active = self.battlefield.iter().any(|p| {
                        p.card_name == CardName::EtherswornCanonist
                    });
                    if canonist_active
                        && !def.card_types.contains(&CardType::Artifact)
                        && player.nonartifact_spells_cast_this_turn >= 1
                    {
                        continue;
                    }

                    // Deafening Silence: each player can cast only one noncreature spell per turn
                    let deafening_silence_active = self.battlefield.iter().any(|p| {
                        p.card_name == CardName::DeafeningSilence
                    });
                    if deafening_silence_active
                        && !def.card_types.contains(&CardType::Creature)
                        && player.noncreature_spells_cast_this_turn >= 1
                    {
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
        let artifact_lockdown = self.battlefield.iter().any(|p| {
            matches!(p.card_name, CardName::CollectorOuphe | CardName::NullRod | CardName::StonySilence)
        });
        for perm in self.permanents_controlled_by(player_id) {
            if perm.tapped {
                continue;
            }
            // Collector Ouphe / Null Rod / Stony Silence: artifact activated abilities can't be activated
            if artifact_lockdown && perm.is_artifact() {
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
            // Collector Ouphe / Null Rod / Stony Silence: artifact activated abilities can't be activated
            if artifact_lockdown && perm.is_artifact() {
                continue;
            }
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

        // Count tax effects from the battlefield
        for p in &self.battlefield {
            match p.card_name {
                // Thalia: noncreature spells cost {1} more (opponent's)
                CardName::ThaliaGuardianOfThraben if p.controller != _controller => {
                    if !def.card_types.contains(&CardType::Creature) {
                        cost.generic += 1;
                    }
                }
                // Archon of Emeria: each player can cast only 1 spell per turn
                // (cast restriction handled elsewhere, but also nonbasic lands enter tapped)

                // Lodestone Golem: nonartifact spells cost {1} more
                CardName::LodestoneGolem if p.controller != _controller => {
                    if !def.card_types.contains(&CardType::Artifact) {
                        cost.generic += 1;
                    }
                }
                // Sphere of Resistance: each spell costs {1} more
                CardName::SphereOfResistance => {
                    cost.generic += 1;
                }
                // Thorn of Amethyst: noncreature spells cost {1} more
                CardName::ThornOfAmethyst => {
                    if !def.card_types.contains(&CardType::Creature) {
                        cost.generic += 1;
                    }
                }
                // Defense Grid: spells cast not during controller's turn cost {3} more
                CardName::DefenseGrid if self.active_player != _controller => {
                    cost.generic += 3;
                }
                // Damping Sphere: each spell after the first costs {1} more per spell
                CardName::DampingSphere => {
                    let spells_cast = self.players[_controller as usize].spells_cast_this_turn;
                    if spells_cast > 0 {
                        cost.generic += spells_cast as u8;
                    }
                }
                // Dovin, Hand of Control: artifacts/instants/sorceries cost {1} more (opponent's)
                CardName::DovinHandOfControl if p.controller != _controller => {
                    if def.card_types.contains(&CardType::Artifact)
                        || def.card_types.contains(&CardType::Instant)
                        || def.card_types.contains(&CardType::Sorcery) {
                        cost.generic += 1;
                    }
                }
                // Foundry Inspector: artifact spells cost {1} less (own)
                CardName::FoundryInspector if p.controller == _controller => {
                    if def.card_types.contains(&CardType::Artifact) && cost.generic > 0 {
                        cost.generic -= 1;
                    }
                }
                _ => {}
            }
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

            // Shock lands (two options each)
            CardName::HallowedFountain => vec![Some(Color::White), Some(Color::Blue)],
            CardName::WateryGrave => vec![Some(Color::Blue), Some(Color::Black)],
            CardName::BloodCrypt => vec![Some(Color::Black), Some(Color::Red)],
            CardName::StompingGround => vec![Some(Color::Red), Some(Color::Green)],
            CardName::TempleGarden => vec![Some(Color::Green), Some(Color::White)],
            CardName::GodlessShrine => vec![Some(Color::White), Some(Color::Black)],
            CardName::SteamVents => vec![Some(Color::Blue), Some(Color::Red)],
            CardName::OvergrownTomb => vec![Some(Color::Black), Some(Color::Green)],
            CardName::SacredFoundry => vec![Some(Color::Red), Some(Color::White)],
            CardName::BreedingPool => vec![Some(Color::Green), Some(Color::Blue)],

            // Survey/Misc dual lands
            CardName::MeticulousArchive => vec![Some(Color::White), Some(Color::Blue)],
            CardName::UndercitySewers => vec![Some(Color::Blue), Some(Color::Black)],
            CardName::ThunderingFalls => vec![Some(Color::Red), Some(Color::Green)],
            CardName::HedgeMaze => vec![Some(Color::Green), Some(Color::White)],

            // Other utility lands producing colored mana
            CardName::Karakas => vec![Some(Color::White)],
            CardName::OtawaraSoaringCity => vec![Some(Color::Blue)],
            CardName::BoseijuWhoEndures => vec![Some(Color::Green)],
            CardName::GaeasCradle => {
                let creature_count = self.creatures_controlled_by(perm.controller).count();
                if creature_count > 0 {
                    vec![Some(Color::Green)]
                } else {
                    vec![]
                }
            }

            // Lands producing colorless
            CardName::CityOfTraitors | CardName::GhostQuarter
            | CardName::SpireOfIndustry | CardName::TheMycoSynthGardens
            | CardName::UrzasSaga | CardName::TalonGatesOfMadara => vec![None],

            // Lands producing any color
            CardName::ForbiddenOrchard | CardName::StartingTown => vec![
                Some(Color::White), Some(Color::Blue), Some(Color::Black),
                Some(Color::Red), Some(Color::Green),
            ],

            // Urborg makes all lands Swamps (they tap for black)
            // Yavimaya makes all lands Forests (they tap for green)
            // These are handled as static effects on the lands themselves
            CardName::UrborgTombOfYawgmoth => vec![Some(Color::Black)],
            CardName::YavimayaCradleOfGrowth => vec![Some(Color::Green)],

            // Bazaar of Baghdad: doesn't produce mana, only draw/discard (activated ability)
            // Dryad Arbor: it's a forest, taps for green
            CardName::DryadArbor => vec![Some(Color::Green)],

            // Gleemox: any color
            CardName::Gleemox => vec![
                Some(Color::White), Some(Color::Blue), Some(Color::Black),
                Some(Color::Red), Some(Color::Green),
            ],

            // Chrome Mox, Mox Diamond, Mox Opal: any color (simplified)
            CardName::ChromeMox | CardName::MoxDiamond | CardName::MoxOpal => vec![
                Some(Color::White), Some(Color::Blue), Some(Color::Black),
                Some(Color::Red), Some(Color::Green),
            ],

            // Chromatic Star: any color
            CardName::ChromaticStar => vec![
                Some(Color::White), Some(Color::Blue), Some(Color::Black),
                Some(Color::Red), Some(Color::Green),
            ],

            // Delighted Halfling: colorless, or any color for legendaries (simplified as any)
            CardName::DelightedHalfling => vec![
                None,
                Some(Color::White), Some(Color::Blue), Some(Color::Black),
                Some(Color::Red), Some(Color::Green),
            ],

            // Deathrite Shaman: mana from exiling land cards
            CardName::DeathriteShaman => {
                // Check if any graveyard has land cards
                let has_land_in_gy = self.players.iter().any(|p| {
                    p.graveyard.iter().any(|&id| {
                        if let Some(name) = self.card_name_for_id(id) {
                            if let Some(def) = find_card(&[], name) { // would need db
                                return def.card_types.contains(&CardType::Land);
                            }
                        }
                        false
                    })
                });
                if has_land_in_gy {
                    vec![Some(Color::White), Some(Color::Blue), Some(Color::Black),
                         Some(Color::Red), Some(Color::Green)]
                } else {
                    vec![]
                }
            }

            // Undermountain Adventurer: any color
            CardName::UndermountainAdventurer => vec![
                Some(Color::White), Some(Color::Blue), Some(Color::Black),
                Some(Color::Red), Some(Color::Green),
            ],

            // The Mightstone and Weakstone: {T} for CC
            CardName::TheMightstoneAndWeakstone => vec![None],

            // Coveted Jewel: 3 mana of one color
            CardName::CovetedJewel => vec![
                Some(Color::White), Some(Color::Blue), Some(Color::Black),
                Some(Color::Red), Some(Color::Green),
            ],

            // KCI: sacrifice artifact (activated ability, not mana ability for options)
            // Voltaic Key, Manifold Key: untap abilities (not mana producers)

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

        // Collector Ouphe / Null Rod / Stony Silence: activated abilities of artifacts can't be activated
        if perm.is_artifact() {
            let artifact_lockdown = self.battlefield.iter().any(|p| {
                matches!(p.card_name, CardName::CollectorOuphe | CardName::NullRod | CardName::StonySilence)
            });
            if artifact_lockdown {
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
            | CardName::BirdsOfParadise
            // Shock lands
            | CardName::HallowedFountain
            | CardName::WateryGrave
            | CardName::BloodCrypt
            | CardName::StompingGround
            | CardName::TempleGarden
            | CardName::GodlessShrine
            | CardName::SteamVents
            | CardName::OvergrownTomb
            | CardName::SacredFoundry
            | CardName::BreedingPool
            // Survey dual lands
            | CardName::MeticulousArchive
            | CardName::UndercitySewers
            | CardName::ThunderingFalls
            | CardName::HedgeMaze
            // Other colored-producing lands
            | CardName::Karakas
            | CardName::OtawaraSoaringCity
            | CardName::BoseijuWhoEndures
            | CardName::UrborgTombOfYawgmoth
            | CardName::YavimayaCradleOfGrowth
            | CardName::DryadArbor
            // Any-color mana producers
            | CardName::ForbiddenOrchard
            | CardName::StartingTown
            | CardName::Gleemox
            | CardName::ChromeMox
            | CardName::MoxDiamond
            | CardName::MoxOpal
            | CardName::ChromaticStar
            | CardName::DelightedHalfling
            | CardName::UndermountainAdventurer => {
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

            // Strip Mine / Wasteland / other colorless-producing lands: {T} for {C}
            CardName::StripMine | CardName::Wasteland | CardName::LibraryOfAlexandria
            | CardName::GhostQuarter | CardName::SpireOfIndustry
            | CardName::TheMycoSynthGardens | CardName::UrzasSaga
            | CardName::TalonGatesOfMadara => {
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

            // City of Traitors: {T} for {C}{C}
            CardName::CityOfTraitors => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.tapped = true;
                }
                self.players[controller as usize].mana_pool.colorless += 2;
                true
            }

            // Gaea's Cradle: {T} for {G} per creature
            CardName::GaeasCradle => {
                let creature_count = self.creatures_controlled_by(controller).count() as u8;
                if creature_count == 0 {
                    return false;
                }
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.tapped = true;
                }
                self.players[controller as usize]
                    .mana_pool
                    .add(Some(Color::Green), creature_count);
                true
            }

            // The Mightstone and Weakstone: {T} for {C}{C}
            CardName::TheMightstoneAndWeakstone => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.tapped = true;
                }
                self.players[controller as usize].mana_pool.colorless += 2;
                true
            }

            // Coveted Jewel: {T} for 3 of one color
            CardName::CovetedJewel => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.tapped = true;
                }
                self.players[controller as usize]
                    .mana_pool
                    .add(color_choice, 3);
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

            // KCI: sacrifice for {C}{C} - handled as activated ability
            CardName::KrarkClanIronworks => false, // Not a tap ability

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

        // Sacrifice abilities (Black Lotus, Lotus Petal, Lion's Eye Diamond, Treasure tokens)
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
            // Treasure token: Sacrifice to add one mana of any color
            CardName::TreasureToken => {
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

        // Karakas: bounce legendary creature
        if perm.card_name == CardName::Karakas && !perm.tapped {
            for target in &self.battlefield {
                if target.is_creature() && self.is_legendary(target) {
                    abilities.push((1, vec![Target::Object(target.id)]));
                }
            }
        }

        // GhostQuarter: destroy target land
        if perm.card_name == CardName::GhostQuarter && !perm.tapped {
            for target in &self.battlefield {
                if target.is_land() && target.id != perm.id {
                    abilities.push((1, vec![Target::Object(target.id)]));
                }
            }
        }

        // Bazaar of Baghdad: draw 2, discard 3
        if perm.card_name == CardName::BazaarOfBaghdad && !perm.tapped {
            abilities.push((0, vec![]));
        }

        // Sensei's Divining Top: {T} draw + put on top
        if perm.card_name == CardName::SenseisDiviningTop && !perm.tapped {
            abilities.push((0, vec![])); // Look at top 3
            abilities.push((1, vec![])); // Draw + put on top
        }

        // Voltaic Key: untap another artifact
        if perm.card_name == CardName::VoltaicKey && !perm.tapped {
            for target in &self.battlefield {
                if target.is_artifact() && target.id != perm.id && target.tapped {
                    abilities.push((0, vec![Target::Object(target.id)]));
                }
            }
        }

        // Manifold Key: untap another artifact
        if perm.card_name == CardName::ManifoldKey && !perm.tapped {
            for target in &self.battlefield {
                if target.is_artifact() && target.id != perm.id && target.tapped {
                    abilities.push((0, vec![Target::Object(target.id)]));
                }
            }
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
                CardName::NarsetParterOfVeils => {
                    // -2: Look at top 4, take noncreature nonland
                    if perm.loyalty >= 2 {
                        abilities.push((0, vec![]));
                    }
                }
                CardName::GideonOfTheTrials => {
                    // +1: Prevent damage from target permanent
                    for target in &self.battlefield {
                        abilities.push((0, vec![Target::Object(target.id)]));
                    }
                    // 0: Become 4/4 creature
                    abilities.push((1, vec![]));
                }
                CardName::WrennAndSix => {
                    // +1: Return land from graveyard to hand
                    abilities.push((0, vec![]));
                    // -1: Deal 1 damage to any target
                    if perm.loyalty >= 1 {
                        for pid in 0..self.num_players {
                            abilities.push((1, vec![Target::Player(pid)]));
                        }
                        for target in &self.battlefield {
                            if target.is_creature() {
                                abilities.push((1, vec![Target::Object(target.id)]));
                            }
                        }
                    }
                }
                CardName::OkoThiefOfCrowns => {
                    // +2: Create Food token
                    abilities.push((0, vec![]));
                    // +1: Target artifact/creature becomes 3/3 Elk
                    for target in &self.battlefield {
                        if target.is_artifact() || target.is_creature() {
                            abilities.push((1, vec![Target::Object(target.id)]));
                        }
                    }
                }
                CardName::KarnTheGreatCreator => {
                    // +1: Target noncreature artifact becomes creature
                    for target in &self.battlefield {
                        if target.is_artifact() && !target.is_creature() {
                            abilities.push((0, vec![Target::Object(target.id)]));
                        }
                    }
                    // -2: Get artifact from sideboard/exile
                    if perm.loyalty >= 2 {
                        abilities.push((1, vec![]));
                    }
                }
                CardName::KayaOrzhovUsurper => {
                    // +1: Exile cards from graveyard
                    abilities.push((0, vec![]));
                    // -1: Exile nonland permanent MV 1 or less
                    if perm.loyalty >= 1 {
                        for target in &self.battlefield {
                            if !target.is_land() {
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
            // Target any player or creature (damage spells)
            CardName::LightningBolt | CardName::ChainLightning | CardName::ShrapnelBlast => {
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

            // Target creature or planeswalker (damage-based removal)
            CardName::Abrade | CardName::RedirectLightning => {
                let mut targets = Vec::new();
                for perm in &self.battlefield {
                    if perm.is_creature() || perm.is_planeswalker() || perm.is_artifact() {
                        targets.push(vec![Target::Object(perm.id)]);
                    }
                }
                targets
            }

            // Target creature
            CardName::SwordsToPlowshares | CardName::PathToExile | CardName::Dismember
            | CardName::FatalPush | CardName::SnuffOut => {
                self.battlefield
                    .iter()
                    .filter(|p| p.is_creature())
                    .map(|p| vec![Target::Object(p.id)])
                    .collect()
            }

            // Target creature or planeswalker
            CardName::BitterTriumph | CardName::MoltenCollapse | CardName::PrismaticEnding => {
                self.battlefield
                    .iter()
                    .filter(|p| p.is_creature() || p.is_planeswalker())
                    .map(|p| vec![Target::Object(p.id)])
                    .collect()
            }

            // Target nonland permanent
            CardName::CouncilsJudgment | CardName::MarchOfOtherworldlyLight
            | CardName::ChainOfVapor | CardName::IntoTheFloodMaw => {
                self.battlefield
                    .iter()
                    .filter(|p| !p.is_land())
                    .map(|p| vec![Target::Object(p.id)])
                    .collect()
            }

            // Target spell on stack
            CardName::Counterspell | CardName::ManaDrain | CardName::MentalMisstep
            | CardName::ForceOfWill | CardName::ForceOfNegation | CardName::Flusterstorm
            | CardName::Daze | CardName::ManaLeak | CardName::MemoryLapse | CardName::Remand
            | CardName::SpellPierce | CardName::MysticalDispute | CardName::MindbreakTrap
            | CardName::SinkIntoStupor => {
                self.stack
                    .items()
                    .iter()
                    .map(|item| vec![Target::Object(item.id)])
                    .collect()
            }

            // Target activated or triggered ability on stack
            CardName::Stifle | CardName::ConsignToMemory => {
                self.stack
                    .items()
                    .iter()
                    .filter(|item| !matches!(item.kind, StackItemKind::Spell { .. }))
                    .map(|item| vec![Target::Object(item.id)])
                    .collect()
            }

            // Target player (for draw/recall)
            CardName::AncestralRecall => {
                (0..self.num_players)
                    .map(|pid| vec![Target::Player(pid)])
                    .collect()
            }

            // Target opponent (discard spells)
            CardName::Thoughtseize | CardName::Duress | CardName::InquisitionOfKozilek
            | CardName::Unmask | CardName::HymnToTourach | CardName::MindTwist
            | CardName::SheoldredsEdict => {
                vec![vec![Target::Player(self.opponent(controller))]]
            }

            // Target opponent (for damage/drain)
            CardName::TendrillsOfAgony | CardName::BrainFreeze => {
                vec![vec![Target::Player(self.opponent(controller))]]
            }

            // Target artifact or enchantment
            CardName::Disenchant | CardName::NaturesClaim | CardName::Fragmentize
            | CardName::AncientGrudge | CardName::ShatteringSpree | CardName::Vandalblast
            | CardName::Suplex | CardName::UntimellyMalfunction | CardName::Crash
            | CardName::SunderingEruption | CardName::AbruptDecay | CardName::PestControl => {
                self.battlefield
                    .iter()
                    .filter(|p| p.is_artifact() || p.is_enchantment())
                    .map(|p| vec![Target::Object(p.id)])
                    .collect()
            }

            // Target artifact/enchantment opponent controls
            CardName::ForceOfVigor => {
                self.battlefield
                    .iter()
                    .filter(|p| (p.is_artifact() || p.is_enchantment()) && p.controller != controller)
                    .map(|p| vec![Target::Object(p.id)])
                    .collect()
            }

            // Target creature in any graveyard
            CardName::Reanimate | CardName::Exhume => {
                let mut targets = Vec::new();
                for pid in 0..self.num_players as usize {
                    for &id in &self.players[pid].graveyard {
                        targets.push(vec![Target::Object(id)]);
                    }
                }
                targets
            }

            // Target card in own graveyard
            CardName::Regrowth | CardName::NoxiousRevival | CardName::MemorysJourney => {
                self.players[controller as usize]
                    .graveyard
                    .iter()
                    .map(|&id| vec![Target::Object(id)])
                    .collect()
            }

            // Blue/red hosers
            CardName::Pyroblast | CardName::RedElementalBlast => {
                let mut targets = Vec::new();
                for perm in &self.battlefield {
                    targets.push(vec![Target::Object(perm.id)]);
                }
                for item in self.stack.items() {
                    targets.push(vec![Target::Object(item.id)]);
                }
                targets
            }

            // No targets needed (tutors, cantrips, board wipes, etc.)
            _ => vec![],
        }
    }
}

pub fn is_uncounterable(name: CardName) -> bool {
    matches!(
        name,
        CardName::AbruptDecay
    )
}
