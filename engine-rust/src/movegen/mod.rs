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
            ChoiceKind::ChooseFromList { options, reason } => {
                let mut actions: Vec<Action> = options.iter().map(|&id| Action::ChooseCard(id)).collect();
                // Show and Tell: players may also pass (choose nothing). Use ChooseCard(0) as sentinel.
                if matches!(reason, ChoiceReason::ShowAndTellChoose { .. }) {
                    actions.push(Action::ChooseCard(0));
                }
                actions
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
                    let is_artifact = def.card_types.contains(&CardType::Artifact);

                    // For X spells, determine the range of valid X values
                    let x_values: Vec<u8> = if def.has_x_cost {
                        // Check if we can afford at least the base cost (X=0)
                        let can_afford_base = if is_artifact {
                            player.mana_pool.can_pay_for_artifact(&effective_cost)
                        } else {
                            player.mana_pool.can_pay(&effective_cost)
                        };
                        if !can_afford_base {
                            continue;
                        }
                        // Compute remaining mana after paying the base cost
                        let mut temp_pool = player.mana_pool;
                        if is_artifact {
                            temp_pool.pay_for_artifact(&effective_cost);
                            let remaining = temp_pool.total_for_artifact() as u8;
                            let max_x = if def.x_multiplier > 0 {
                                remaining / def.x_multiplier
                            } else {
                                remaining
                            };
                            let cap = max_x.min(10);
                            (0..=cap).collect()
                        } else {
                            temp_pool.pay(&effective_cost);
                            let remaining = temp_pool.total() as u8;
                            // Max X is limited by remaining mana divided by x_multiplier
                            let max_x = if def.x_multiplier > 0 {
                                remaining / def.x_multiplier
                            } else {
                                remaining
                            };
                            // Generate X values from 0 to max_x (inclusive)
                            // For search efficiency, cap at a reasonable limit
                            let cap = max_x.min(10);
                            (0..=cap).collect()
                        }
                    } else {
                        let can_afford = if is_artifact {
                            player.mana_pool.can_pay_for_artifact(&effective_cost)
                        } else {
                            player.mana_pool.can_pay(&effective_cost)
                        };
                        if !can_afford {
                            // Can't pay - but first check if we could tap lands to get mana
                            // For search, we generate mana ability actions separately
                            continue;
                        }
                        vec![0u8]
                    };

                    // Modal spells: generate actions per valid mode combination
                    if is_modal_spell(card_name) {
                        let modal_actions = self.generate_modal_actions(card_id, card_name, player_id, db);
                        actions.extend(modal_actions);
                        continue;
                    }

                    // Generate target permutations
                    let target_sets = self.generate_targets(card_name, player_id, db);
                    for x_value in x_values {
                        if target_sets.is_empty() {
                            // If this spell requires a sacrifice as an additional cost,
                            // it cannot be cast without a valid sacrifice target.
                            if requires_sacrifice_cost(card_name) {
                                continue;
                            }
                            actions.push(Action::CastSpell {
                                card_id,
                                targets: vec![],
                                x_value,
                                from_graveyard: false,
                from_library_top: false,
                                alt_cost: None,
                                modes: vec![],
                            });
                        } else {
                            for targets in &target_sets {
                                actions.push(Action::CastSpell {
                                    card_id,
                                    targets: targets.clone(),
                                    x_value,
                                    from_graveyard: false,
                from_library_top: false,
                                    alt_cost: None,
                                modes: vec![],
                                });
                            }
                        }
                    }
                }
            }
        }

        // --- Cast spells from graveyard (flashback / Yawgmoth's Will / Snapcaster Mage) ---
        // Check if any graveyard spell-casting effect is active.
        let yawgmoth_active = self.players[player_id as usize].yawgmoth_will_active;
        // Grafdigger's Cage: players can't cast spells from graveyards or libraries.
        let cage_blocks_graveyard = self.grafdiggers_cage_active();
        {
            let graveyard: Vec<ObjectId> = self.players[player_id as usize].graveyard.clone();
            for card_id in graveyard {
                if let Some(card_name) = self.card_name_for_id(card_id) {
                    if let Some(def) = find_card(db, card_name) {
                        // Skip lands (Yawgmoth's Will lets you play lands, handled separately for now)
                        if def.card_types.contains(&CardType::Land) {
                            continue;
                        }
                        // Grafdigger's Cage prevents casting from graveyard entirely.
                        if cage_blocks_graveyard {
                            continue;
                        }
                        // Determine if this card can be cast from graveyard and what cost to use.
                        // Priority: own flashback_cost > snapcaster grant > yawgmoth (normal cost)
                        let has_own_flashback = def.flashback_cost.is_some();
                        let has_snapcaster_flashback = self.snapcaster_flashback_cards.contains(&card_id);
                        // Wrenn and Six emblem: instant and sorcery cards in your graveyard have retrace.
                        // Retrace: you may cast this card from your graveyard by discarding a land.
                        let has_wrenn_retrace = self.has_emblem(player_id, crate::game::Emblem::WrennAndSix)
                            && (def.card_types.contains(&CardType::Instant)
                                || def.card_types.contains(&CardType::Sorcery));
                        // Check if the player has a land to discard (retrace cost)
                        let can_retrace = has_wrenn_retrace && self.players[player_id as usize].hand.iter().any(|&id| {
                            self.card_name_for_id(id)
                                .and_then(|cn| find_card(db, cn))
                                .map(|d| d.card_types.contains(&CardType::Land))
                                .unwrap_or(false)
                        });
                        let can_cast_from_gyd = has_own_flashback || has_snapcaster_flashback || yawgmoth_active || can_retrace;

                        if !can_cast_from_gyd {
                            continue;
                        }

                        // Determine the flashback cost:
                        // 1. Snapcaster grants flashback using the card's own mana cost.
                        // 2. Cards with their own flashback_cost use that cost.
                        // 3. Yawgmoth's Will uses the card's normal mana cost.
                        let flashback_base_cost = if has_own_flashback {
                            def.flashback_cost.unwrap()
                        } else {
                            // Snapcaster or Yawgmoth: use card's normal mana cost
                            def.mana_cost
                        };

                        // Check timing (flashback follows same timing as original spell type)
                        let can_cast_at_instant_speed = def.card_types.contains(&CardType::Instant)
                            || def.keywords.has(Keyword::Flash);
                        let can_cast = if can_cast_at_instant_speed {
                            true
                        } else {
                            sorcery_speed
                        };

                        if !can_cast {
                            continue;
                        }

                        // Apply cast-restriction statics
                        let archon_active = self.battlefield.iter().any(|p| {
                            p.card_name == CardName::ArchonOfEmeria
                        });
                        if archon_active && player.spells_cast_this_turn >= 1 {
                            continue;
                        }

                        let canonist_active = self.battlefield.iter().any(|p| {
                            p.card_name == CardName::EtherswornCanonist
                        });
                        if canonist_active
                            && !def.card_types.contains(&CardType::Artifact)
                            && player.nonartifact_spells_cast_this_turn >= 1
                        {
                            continue;
                        }

                        let deafening_silence_active = self.battlefield.iter().any(|p| {
                            p.card_name == CardName::DeafeningSilence
                        });
                        if deafening_silence_active
                            && !def.card_types.contains(&CardType::Creature)
                            && player.noncreature_spells_cast_this_turn >= 1
                        {
                            continue;
                        }

                        // Check mana affordability using the flashback cost (with taxes applied)
                        let effective_flashback_cost = self.effective_cost_with_base(def, player_id, flashback_base_cost);

                        let can_afford_flashback = if def.card_types.contains(&CardType::Artifact) {
                            player.mana_pool.can_pay_for_artifact(&effective_flashback_cost)
                        } else {
                            player.mana_pool.can_pay(&effective_flashback_cost)
                        };
                        if !can_afford_flashback {
                            continue;
                        }

                        // Generate target permutations
                        let target_sets = self.generate_targets(card_name, player_id, db);
                        if target_sets.is_empty() {
                            if requires_sacrifice_cost(card_name) {
                                continue;
                            }
                            actions.push(Action::CastSpell {
                                card_id,
                                targets: vec![],
                                x_value: 0,
                                from_graveyard: true,
                from_library_top: false,
                                alt_cost: None,
                                modes: vec![],
                            });
                        } else {
                            for targets in &target_sets {
                                actions.push(Action::CastSpell {
                                    card_id,
                                    targets: targets.clone(),
                                    x_value: 0,
                                    from_graveyard: true,
                from_library_top: false,
                                    alt_cost: None,
                                modes: vec![],
                                });
                            }
                        }
                    }
                }
            }
        }

        // --- Cast spells / play lands from top of library ---
        // Bolas's Citadel: you may play lands and cast spells from the top of your library,
        //   paying life equal to their mana value instead of mana.
        // Future Sight: you may play the top card of your library.
        // Mystic Forge: you may cast artifact spells and colorless spells from the top of your library.
        // Experimental Frenzy: you may cast spells from the top of your library (not from hand).
        // Grafdigger's Cage also blocks casting from libraries.
        {
            let cage_blocks_library = self.grafdiggers_cage_active();
            let citadel_active = !cage_blocks_library && self.battlefield.iter().any(|p| {
                p.card_name == CardName::BolassCitadel && p.controller == player_id
            });
            let future_sight_active = !cage_blocks_library && self.battlefield.iter().any(|p| {
                p.card_name == CardName::FutureSight && p.controller == player_id
            });
            let mystic_forge_active = !cage_blocks_library && self.battlefield.iter().any(|p| {
                p.card_name == CardName::MysticForge && p.controller == player_id
            });
            let experimental_frenzy_active = !cage_blocks_library && self.battlefield.iter().any(|p| {
                p.card_name == CardName::ExperimentalFrenzy && p.controller == player_id
            });

            let any_library_top_active = citadel_active || future_sight_active
                || mystic_forge_active || experimental_frenzy_active;

            if any_library_top_active {
                if let Some(&top_card_id) = self.players[player_id as usize].library.last() {
                    if let Some(top_name) = self.card_name_for_id(top_card_id) {
                        if let Some(top_def) = find_card(db, top_name) {
                            let is_land = top_def.card_types.contains(&CardType::Land);
                            let is_artifact = top_def.card_types.contains(&CardType::Artifact);
                            // A spell is colorless if it has no colored mana symbols in its cost.
                            let mc = top_def.mana_cost;
                            let is_colorless = mc.white == 0 && mc.blue == 0 && mc.black == 0
                                && mc.red == 0 && mc.green == 0;

                            if is_land {
                                // Play a land from top of library (sorcery speed, land plays remaining)
                                if sorcery_speed && player.land_plays_remaining > 0 {
                                    // Citadel, Future Sight allow playing lands
                                    if citadel_active || future_sight_active {
                                        actions.push(Action::PlayLandFromTop(top_card_id));
                                    }
                                }
                            } else {
                                // Cast a spell from top of library
                                // Timing check
                                let can_cast_instant = top_def.card_types.contains(&CardType::Instant)
                                    || top_def.keywords.has(Keyword::Flash);
                                let can_cast = can_cast_instant || sorcery_speed;

                                if can_cast {
                                    // Which enablers allow casting this card?
                                    let can_cast_with_citadel = citadel_active;
                                    let can_cast_with_future_sight = future_sight_active;
                                    let can_cast_with_forge = mystic_forge_active
                                        && (is_artifact || is_colorless);
                                    let can_cast_with_frenzy = experimental_frenzy_active;
                                    let enabled = can_cast_with_citadel || can_cast_with_future_sight
                                        || can_cast_with_forge || can_cast_with_frenzy;

                                    if enabled {
                                        // Affordability check
                                        let can_afford = if citadel_active && !future_sight_active
                                            && !can_cast_with_forge && !can_cast_with_frenzy
                                        {
                                            // Citadel only: pay life equal to mana value
                                            let life_cost = top_def.mana_cost.cmc() as i32;
                                            player.life > life_cost
                                        } else if can_cast_with_citadel {
                                            // When both citadel and mana-based options available,
                                            // check if either payment is affordable
                                            let cost = self.effective_cost(top_def, player_id);
                                            let can_pay_mana = if is_artifact {
                                                player.mana_pool.can_pay_for_artifact(&cost)
                                            } else {
                                                player.mana_pool.can_pay(&cost)
                                            };
                                            player.life > top_def.mana_cost.cmc() as i32
                                                || can_pay_mana
                                        } else {
                                            // Future Sight / Mystic Forge / Frenzy: pay mana
                                            let cost = self.effective_cost(top_def, player_id);
                                            if is_artifact {
                                                player.mana_pool.can_pay_for_artifact(&cost)
                                            } else {
                                                player.mana_pool.can_pay(&cost)
                                            }
                                        };

                                        if can_afford {
                                            let target_sets = self.generate_targets(top_name, player_id, db);
                                            if target_sets.is_empty() {
                                                if !requires_sacrifice_cost(top_name) {
                                                    actions.push(Action::CastSpell {
                                                        card_id: top_card_id,
                                                        targets: vec![],
                                                        x_value: 0,
                                                        from_graveyard: false,
                                                        from_library_top: true,
                                                        alt_cost: None,
                                modes: vec![],
                                                    });
                                                }
                                            } else {
                                                for targets in &target_sets {
                                                    actions.push(Action::CastSpell {
                                                        card_id: top_card_id,
                                                        targets: targets.clone(),
                                                        x_value: 0,
                                                        from_graveyard: false,
                                                        from_library_top: true,
                                                        alt_cost: None,
                                modes: vec![],
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // --- Alternate-cost spells ---
        // Generate CastSpell actions for cards that can be cast by paying an alternate cost
        // instead of their normal mana cost (Force cycle, evoke creatures, etc.).
        self.generate_alt_cost_actions(player_id, db, &mut actions, sorcery_speed);

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

        // --- Companion: pay {3} to put companion into hand (instant speed) ---
        // The companion is a card that starts outside the game (sideboard).
        // Once per game, the player may pay {3} at any time they have priority to
        // put their companion from outside the game into their hand.
        if player.companion.is_some() {
            let companion_cost = crate::mana::ManaCost { generic: 3, ..crate::mana::ManaCost::ZERO };
            if player.mana_pool.can_pay(&companion_cost) {
                actions.push(Action::CompanionFromSideboard);
            }
        }

        // --- Cycling abilities (from hand, instant speed) ---
        // ability_index 0 = cycling, 1 = channel
        for &card_id in &player.hand {
            if let Some(card_name) = self.card_name_for_id(card_id) {
                // --- Cycling ---
                if let Some((cycling_cost, cycling_kind)) = crate::card::cycling_ability(card_name) {
                    match cycling_kind {
                        CyclingKind::Basic => {
                            // Street Wraith cycling costs 2 life (zero mana), check life total
                            let life_cost = if card_name == CardName::StreetWraith { 2i32 } else { 0 };
                            let can_pay_mana = player.mana_pool.can_pay(&cycling_cost);
                            let can_pay_life = player.life > life_cost;
                            if can_pay_mana && can_pay_life {
                                actions.push(Action::ActivateFromHand {
                                    card_id,
                                    ability_index: 0,
                                    targets: vec![],
                                    x_value: 0,
                                });
                            }
                        }
                        CyclingKind::SharkTyphoon => {
                            // Cycling cost {X}{U}: check if we can pay at least the {U} part
                            if player.mana_pool.can_pay(&cycling_cost) {
                                // Compute remaining mana after paying {U}
                                let mut temp_pool = player.mana_pool;
                                temp_pool.pay(&cycling_cost);
                                let max_x = temp_pool.total() as u8;
                                // Generate X=0 through max_x
                                let cap = max_x.min(10);
                                for x in 0..=cap {
                                    actions.push(Action::ActivateFromHand {
                                        card_id,
                                        ability_index: 0,
                                        targets: vec![],
                                        x_value: x,
                                    });
                                }
                            }
                        }
                    }
                }

                // --- Channel ---
                if let Some((channel_cost, channel_kind)) = crate::card::channel_ability(card_name) {
                    if player.mana_pool.can_pay(&channel_cost) {
                        let targets = self.generate_channel_targets(channel_kind, player_id);
                        for target in targets {
                            actions.push(Action::ActivateFromHand {
                                card_id,
                                ability_index: 1,
                                targets: vec![target],
                                x_value: 0,
                            });
                        }
                    }
                }
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
                    let can_block_flight = if attacker.keywords.has(Keyword::Flying) {
                        perm.can_block_flyer()
                    } else {
                        true
                    };
                    // Protection: the attacker can't be blocked by creatures with a protected quality.
                    // Check if the attacker has protection from the blocker's colors or from the blocking player.
                    let blocked_by_protection = attacker.is_protected_from(&perm.colors, perm.controller);
                    // Menace: must be blocked by 2+ creatures (simplified: allow single block)
                    if can_block_flight && !blocked_by_protection {
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

    /// Generate valid targets for a channel ability.
    fn generate_channel_targets(&self, channel_kind: ChannelKind, player_id: PlayerId) -> Vec<Target> {
        match channel_kind {
            ChannelKind::Boseiju => {
                // Destroy target artifact, enchantment, or nonbasic land controlled by an opponent
                let opponent = self.opponent(player_id);
                self.battlefield.iter()
                    .filter(|p| {
                        p.controller == opponent
                            && (p.is_artifact() || p.is_enchantment() || (p.is_land() && !self.is_basic_land(p)))
                    })
                    .map(|p| Target::Object(p.id))
                    .collect()
            }
            ChannelKind::Otawara => {
                // Return target artifact, creature, or planeswalker to owner's hand
                self.battlefield.iter()
                    .filter(|p| p.is_artifact() || p.is_creature() || p.is_planeswalker())
                    .map(|p| Target::Object(p.id))
                    .collect()
            }
        }
    }

    /// Get the effective mana cost of a card after tax effects (Thalia, Trinisphere, etc.)
    /// and cost reduction effects (Foundry Inspector, affinity, etc.).
    pub(crate) fn effective_cost(&self, def: &CardDef, controller: PlayerId) -> ManaCost {
        self.effective_cost_with_base(def, controller, def.mana_cost)
    }

    /// Like `effective_cost`, but uses `base_cost` instead of `def.mana_cost`.
    /// Used for flashback casting where the alternate cost is different from the normal cost.
    pub(crate) fn effective_cost_with_base(&self, def: &CardDef, _controller: PlayerId, base_cost: ManaCost) -> ManaCost {
        let mut cost = base_cost;

        // Accumulate total cost increase and decrease separately, then apply at the end.
        // This avoids order-dependence bugs when combining taxes and reductions.
        let mut generic_increase: u32 = 0;
        let mut generic_reduction: u32 = 0;

        // Count tax effects from the battlefield
        for p in &self.battlefield {
            match p.card_name {
                // Thalia: noncreature spells cost {1} more (opponent's)
                CardName::ThaliaGuardianOfThraben if p.controller != _controller => {
                    if !def.card_types.contains(&CardType::Creature) {
                        generic_increase += 1;
                    }
                }
                // Archon of Emeria: each player can cast only 1 spell per turn
                // (cast restriction handled elsewhere, but also nonbasic lands enter tapped)

                // Lodestone Golem: nonartifact spells cost {1} more
                CardName::LodestoneGolem if p.controller != _controller => {
                    if !def.card_types.contains(&CardType::Artifact) {
                        generic_increase += 1;
                    }
                }
                // Sphere of Resistance: each spell costs {1} more
                CardName::SphereOfResistance => {
                    generic_increase += 1;
                }
                // Thorn of Amethyst: noncreature spells cost {1} more
                CardName::ThornOfAmethyst => {
                    if !def.card_types.contains(&CardType::Creature) {
                        generic_increase += 1;
                    }
                }
                // Defense Grid: spells cast not during controller's turn cost {3} more
                CardName::DefenseGrid if self.active_player != _controller => {
                    generic_increase += 3;
                }
                // Damping Sphere: each spell after the first costs {1} more per spell
                CardName::DampingSphere => {
                    let spells_cast = self.players[_controller as usize].spells_cast_this_turn;
                    if spells_cast > 0 {
                        generic_increase += spells_cast as u32;
                    }
                }
                // Dovin, Hand of Control: artifacts/instants/sorceries cost {1} more (opponent's)
                CardName::DovinHandOfControl if p.controller != _controller => {
                    if def.card_types.contains(&CardType::Artifact)
                        || def.card_types.contains(&CardType::Instant)
                        || def.card_types.contains(&CardType::Sorcery) {
                        generic_increase += 1;
                    }
                }
                // Foundry Inspector: artifact spells cost {1} less to cast (own)
                CardName::FoundryInspector if p.controller == _controller => {
                    if def.card_types.contains(&CardType::Artifact) {
                        generic_reduction += 1;
                    }
                }
                _ => {}
            }
        }

        // Hollow One: costs {2} less for each card cycled or discarded this turn.
        if def.name == CardName::HollowOne {
            let discards = self.players[_controller as usize].cards_discarded_this_turn as u32;
            generic_reduction += discards * 2;
        }

        // Affinity for artifacts: reduce cost by the number of artifacts the controller controls.
        // Applies to ThoughtMonitor and Thoughtcast.
        let has_affinity_for_artifacts = matches!(
            def.name,
            CardName::ThoughtMonitor | CardName::Thoughtcast
        );
        if has_affinity_for_artifacts {
            let artifact_count = self.battlefield.iter()
                .filter(|p| p.controller == _controller && p.is_artifact())
                .count() as u32;
            generic_reduction += artifact_count;
        }

        // Apply increases then reductions to generic, keeping it non-negative.
        // Reductions can only reduce the generic portion (not colored mana requirements).
        let generic_after_increase = cost.generic as u32 + generic_increase;
        cost.generic = generic_after_increase.saturating_sub(generic_reduction).min(u8::MAX as u32) as u8;

        // Trinisphere: spells cost at least {3} (when untapped)
        let trinisphere_active = self.battlefield.iter().any(|p| {
            p.card_name == CardName::Trinisphere && !p.tapped
        });
        if trinisphere_active && cost.cmc() < 3 {
            cost.generic = 3 - (cost.cmc() - cost.generic);
        }

        cost
    }

    /// Check if Blood Moon is on the battlefield (makes nonbasic lands into Mountains).
    fn blood_moon_active(&self) -> bool {
        self.battlefield.iter().any(|p| p.card_name == CardName::BloodMoon)
    }

    /// Check if Urborg, Tomb of Yawgmoth is on the battlefield (all lands also tap for B).
    fn urborg_active(&self) -> bool {
        self.battlefield.iter().any(|p| p.card_name == CardName::UrborgTombOfYawgmoth)
    }

    /// Check if Yavimaya, Cradle of Growth is on the battlefield (all lands also tap for G).
    fn yavimaya_active(&self) -> bool {
        self.battlefield.iter().any(|p| p.card_name == CardName::YavimayaCradleOfGrowth)
    }

    /// Apply land-type static effects (Blood Moon, Urborg, Yavimaya) to a land's mana options.
    /// - Blood Moon: nonbasic lands lose all abilities and become Mountains (only tap for R).
    /// - Urborg: all lands gain {T}: Add {B}.
    /// - Yavimaya: all lands gain {T}: Add {G}.
    fn apply_land_type_effects(
        &self,
        perm: &crate::permanent::Permanent,
        mut options: Vec<Option<Color>>,
    ) -> Vec<Option<Color>> {
        if !perm.is_land() {
            return options;
        }
        let is_basic = self.is_basic_land(perm);

        // Blood Moon: nonbasic lands lose their abilities and become Mountains (only tap for R).
        // This overrides all other effects for nonbasic lands.
        if !is_basic && self.blood_moon_active() {
            return vec![Some(Color::Red)];
        }

        // Urborg: all lands also tap for B (deduplicate if already present).
        if self.urborg_active() && !options.contains(&Some(Color::Black)) {
            options.push(Some(Color::Black));
        }

        // Yavimaya: all lands also tap for G (deduplicate if already present).
        if self.yavimaya_active() && !options.contains(&Some(Color::Green)) {
            options.push(Some(Color::Green));
        }

        options
    }

    /// What mana can a permanent produce?
    fn mana_ability_options(&self, perm: &crate::permanent::Permanent) -> Vec<Option<Color>> {
        let base = self.mana_ability_options_base(perm);
        self.apply_land_type_effects(perm, base)
    }

    /// Public version of mana_ability_options for testing.
    #[cfg(test)]
    pub fn mana_ability_options_pub(&self, perm: &crate::permanent::Permanent) -> Vec<Option<Color>> {
        self.mana_ability_options(perm)
    }

    /// Base mana options before land-type static effects.
    fn mana_ability_options_base(&self, perm: &crate::permanent::Permanent) -> Vec<Option<Color>> {
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

            // Cavern of Souls: produces colorless, or any color for spells of the named type.
            // Both abilities are always available; the colored-mana-only-for-named-type
            // restriction is enforced at spell casting, not here.
            CardName::CavernOfSouls => vec![
                None, // Colorless {C}
                Some(Color::White),
                Some(Color::Blue),
                Some(Color::Black),
                Some(Color::Red),
                Some(Color::Green),
            ],

            // Bazaar of Baghdad: doesn't produce mana, only draw/discard (activated ability)
            // Dryad Arbor: it's a forest, taps for green
            CardName::DryadArbor => vec![Some(Color::Green)],

            // Gleemox: any color
            CardName::Gleemox => vec![
                Some(Color::White), Some(Color::Blue), Some(Color::Black),
                Some(Color::Red), Some(Color::Green),
            ],

            // Chrome Mox: produces mana matching the imprinted card's colors.
            // If nothing is imprinted, produces no mana.
            CardName::ChromeMox => {
                let imprinted_card_id = self.imprinted.iter()
                    .find(|(perm_id, _)| *perm_id == perm.id)
                    .map(|(_, card_id)| *card_id);
                if let Some(card_id) = imprinted_card_id {
                    // Look up the exiled card's colors from the exile zone.
                    let exiled_name = self.exile.iter()
                        .find(|(id, _, _)| *id == card_id)
                        .map(|(_, name, _)| *name);
                    if let Some(_cn) = exiled_name {
                        // Collect the colors from the imprinted card via the card_registry
                        // We stored the imprinted card's colors when it was in hand.
                        // Since we don't have DB access here, we infer color from the card name
                        // stored in exile. We enumerate possible colors from the exile entry.
                        // For simplicity, return all 5 colors filtered to what the card could be.
                        // The actual color check happens at activation time where we have context.
                        vec![
                            Some(Color::White), Some(Color::Blue), Some(Color::Black),
                            Some(Color::Red), Some(Color::Green),
                        ]
                    } else {
                        vec![]
                    }
                } else {
                    // No imprint: Chrome Mox produces no mana
                    vec![]
                }
            }
            // Mox Diamond: any color
            CardName::MoxDiamond => vec![
                Some(Color::White), Some(Color::Blue), Some(Color::Black),
                Some(Color::Red), Some(Color::Green),
            ],

            // Mox Opal: Metalcraft — tap for any color only if controller has 3+ artifacts
            CardName::MoxOpal => {
                if self.metalcraft(perm.controller) {
                    vec![
                        Some(Color::White), Some(Color::Blue), Some(Color::Black),
                        Some(Color::Red), Some(Color::Green),
                    ]
                } else {
                    vec![]
                }
            }

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

        // Blood Moon: nonbasic lands lose all abilities and become Mountains (tap for R only).
        // This must be checked before the per-card match so that hardcoded colorless producers
        // (AncientTomb, StripMine, etc.) are overridden.
        let is_basic = self.is_basic_land(self.find_permanent(permanent_id).unwrap());
        if !is_basic && self.find_permanent(permanent_id).unwrap().is_land() && self.blood_moon_active() {
            if let Some(perm_mut) = self.find_permanent_mut(permanent_id) {
                perm_mut.tapped = true;
            }
            self.players[controller as usize].mana_pool.add(Some(Color::Red), 1);
            return true;
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

            // Mox Opal: Metalcraft — only active if controller has 3+ artifacts
            CardName::MoxOpal => {
                if !self.metalcraft(controller) {
                    return false;
                }
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.tapped = true;
                }
                self.players[controller as usize]
                    .mana_pool
                    .add(color_choice, 1);
                true
            }

            // Cavern of Souls: tap for colorless {C} or any color (for named creature type).
            // Colored mana from Cavern is tracked as "cavern mana" for uncounterable purposes,
            // but since we simplify mana tracking we just add it to the pool.
            CardName::CavernOfSouls => {
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

            // Mishra's Workshop: {T} for {C}{C}{C} restricted to artifact spells.
            // The mana is tracked in the separate `workshop` pool so the engine only
            // offers it when generating actions for artifact spells.
            CardName::MishrasWorkshop => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.tapped = true;
                }
                self.players[controller as usize].mana_pool.add_workshop(3);
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

        // The One Ring: {T}: Put a burden counter on The One Ring, then draw a card for each burden counter.
        if perm.card_name == CardName::TheOneRing && !perm.tapped {
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

        // Isochron Scepter: {2},{T} — copy and cast the imprinted instant for free.
        // Can only activate if there is an imprinted card.
        if perm.card_name == CardName::IsochronScepter && !perm.tapped {
            let has_imprint = self.imprinted.iter().any(|(perm_id, _)| *perm_id == perm.id);
            if has_imprint {
                abilities.push((0, vec![]));
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
                    // -6: Create emblem
                    if perm.loyalty >= 6 {
                        abilities.push((2, vec![]));
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
                    // +0 emblem: create Gideon emblem (no loyalty cost, can always activate)
                    abilities.push((2, vec![]));
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
                    // -7: Create emblem
                    if perm.loyalty >= 7 {
                        abilities.push((2, vec![]));
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
                CardName::TezzeretCruelCaptain => {
                    // +1: Draw a card if you control an artifact
                    abilities.push((0, vec![]));
                    // -2: Create a 1/1 colorless Thopter artifact creature token with flying
                    if perm.loyalty >= 2 {
                        abilities.push((1, vec![]));
                    }
                    // -7: Create emblem
                    if perm.loyalty >= 7 {
                        abilities.push((2, vec![]));
                    }
                }
                _ => {}
            }
        }

        // --- Equip abilities (sorcery speed) ---
        // Equipment can equip to any creature the controller controls.
        if sorcery_speed {
            if let Some(equip_generic) = crate::card::equip_cost(perm.card_name) {
                // Can pay the equip cost?
                let can_afford = u32::from(self.players[perm.controller as usize].mana_pool.colorless) >= equip_generic as u32
                    || u32::from(self.players[perm.controller as usize].mana_pool.total()) >= equip_generic as u32;
                let _ = can_afford; // We generate the action; payment is enforced at apply time
                // Generate one action per legal creature target
                let controller = perm.controller;
                let perm_id = perm.id;
                let creature_targets: Vec<ObjectId> = self.battlefield.iter()
                    .filter(|p| p.is_creature() && p.controller == controller && p.id != perm_id)
                    .map(|p| p.id)
                    .collect();
                for creature_id in creature_targets {
                    abilities.push((20, vec![Target::Object(creature_id)]));
                }
            }
        }

        // --- Batterskull bounce ability ---
        // {3}: Return Batterskull to owner's hand (at any time with priority)
        if perm.card_name == CardName::Batterskull {
            let controller = perm.controller;
            let can_afford = self.players[controller as usize].mana_pool.total() >= 3;
            let _ = can_afford;
            abilities.push((21, vec![]));
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

    /// Check whether a permanent can be targeted by a spell/ability with the given
    /// source colors cast by `caster`. Returns false if the permanent has hexproof,
    /// shroud, or protection from the source's color or from the caster.
    fn can_be_targeted(
        &self,
        target: &crate::permanent::Permanent,
        caster: PlayerId,
        source_colors: &[Color],
    ) -> bool {
        // Hexproof: can't be targeted by opponents' spells/abilities.
        if target.keywords.has(Keyword::Hexproof) && target.controller != caster {
            return false;
        }
        // Shroud: can't be targeted by any spells/abilities.
        if target.keywords.has(Keyword::Shroud) {
            return false;
        }
        // Protection: can't be targeted by sources with a protected quality.
        if target.is_protected_from(source_colors, caster) {
            return false;
        }
        true
    }

    /// Generate valid target sets for a spell.
    fn generate_targets(
        &self,
        card_name: CardName,
        controller: PlayerId,
        db: &[CardDef],
    ) -> Vec<Vec<Target>> {
        // Look up the spell's color identity for protection checks.
        let spell_colors: Vec<Color> = find_card(db, card_name)
            .map(|def| def.color_identity.to_vec())
            .unwrap_or_default();
        match card_name {
            // Target any player or creature (damage spells)
            CardName::LightningBolt | CardName::ChainLightning => {
                let mut targets = Vec::new();
                for pid in 0..self.num_players {
                    targets.push(vec![Target::Player(pid)]);
                }
                for perm in &self.battlefield {
                    if perm.is_creature() && self.can_be_targeted(perm, controller, &spell_colors) {
                        targets.push(vec![Target::Object(perm.id)]);
                    }
                }
                targets
            }

            // Shrapnel Blast: sacrifice an artifact as additional cost, deal 5 to any target.
            // targets[0] = artifact to sacrifice (controlled by caster), targets[1] = damage target.
            CardName::ShrapnelBlast => {
                let artifacts: Vec<ObjectId> = self.battlefield.iter()
                    .filter(|p| p.controller == controller && p.is_artifact())
                    .map(|p| p.id)
                    .collect();
                if artifacts.is_empty() {
                    return vec![];
                }
                let mut target_sets = Vec::new();
                for &art_id in &artifacts {
                    // Damage can go to any player or creature
                    for pid in 0..self.num_players {
                        target_sets.push(vec![Target::Object(art_id), Target::Player(pid)]);
                    }
                    for perm in &self.battlefield {
                        if perm.is_creature() {
                            target_sets.push(vec![Target::Object(art_id), Target::Object(perm.id)]);
                        }
                    }
                }
                target_sets
            }

            // Village Rites: sacrifice a creature as additional cost, draw 2.
            // targets[0] = creature to sacrifice (controlled by caster).
            CardName::VillageRites => {
                self.battlefield.iter()
                    .filter(|p| p.controller == controller && p.is_creature())
                    .map(|p| vec![Target::Object(p.id)])
                    .collect()
            }

            // Deadly Dispute: sacrifice an artifact or creature as additional cost, draw 2 + Treasure.
            // targets[0] = artifact or creature to sacrifice (controlled by caster).
            CardName::DeadlyDispute => {
                self.battlefield.iter()
                    .filter(|p| p.controller == controller && (p.is_artifact() || p.is_creature()))
                    .map(|p| vec![Target::Object(p.id)])
                    .collect()
            }

            // Natural Order: sacrifice a green creature as additional cost, tutor a green creature.
            // targets[0] = green creature to sacrifice (controlled by caster).
            CardName::NaturalOrder => {
                self.battlefield.iter()
                    .filter(|p| p.controller == controller && p.is_creature())
                    .map(|p| vec![Target::Object(p.id)])
                    .collect()
            }

            // Crop Rotation: sacrifice a land as additional cost, tutor any land.
            // targets[0] = land to sacrifice (controlled by caster).
            CardName::CropRotation => {
                self.battlefield.iter()
                    .filter(|p| p.controller == controller && p.is_land())
                    .map(|p| vec![Target::Object(p.id)])
                    .collect()
            }

            // Target creature or planeswalker (damage-based removal)
            CardName::Abrade | CardName::RedirectLightning => {
                let mut targets = Vec::new();
                for perm in &self.battlefield {
                    if (perm.is_creature() || perm.is_planeswalker() || perm.is_artifact())
                        && self.can_be_targeted(perm, controller, &spell_colors)
                    {
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
                    .filter(|p| p.is_creature() && self.can_be_targeted(p, controller, &spell_colors))
                    .map(|p| vec![Target::Object(p.id)])
                    .collect()
            }

            // Target creature or planeswalker
            CardName::BitterTriumph | CardName::MoltenCollapse | CardName::PrismaticEnding => {
                self.battlefield
                    .iter()
                    .filter(|p| (p.is_creature() || p.is_planeswalker()) && self.can_be_targeted(p, controller, &spell_colors))
                    .map(|p| vec![Target::Object(p.id)])
                    .collect()
            }

            // Target nonland permanent
            CardName::CouncilsJudgment | CardName::MarchOfOtherworldlyLight
            | CardName::ChainOfVapor | CardName::IntoTheFloodMaw => {
                self.battlefield
                    .iter()
                    .filter(|p| !p.is_land() && self.can_be_targeted(p, controller, &spell_colors))
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

            // Snapcaster Mage: ETB targets an instant or sorcery in the controller's graveyard.
            CardName::SnapcasterMage => {
                self.players[controller as usize]
                    .graveyard
                    .iter()
                    .filter(|&&id| {
                        self.card_name_for_id(id)
                            .and_then(|cn| find_card(db, cn))
                            .map(|def| {
                                def.card_types.contains(&CardType::Instant)
                                    || def.card_types.contains(&CardType::Sorcery)
                            })
                            .unwrap_or(false)
                    })
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

            // Surgical Extraction: target a card in any graveyard
            CardName::SurgicalExtraction => {
                let mut targets = Vec::new();
                for pid in 0..self.num_players as usize {
                    for &card_id in &self.players[pid].graveyard {
                        targets.push(vec![Target::Object(card_id)]);
                    }
                }
                targets
            }

            // No targets needed (tutors, cantrips, board wipes, etc.)
            _ => vec![],
        }
    }

    /// Generate CastSpell actions for alternate-cost spells in hand.
    /// These cards can be cast by exiling a card of a specific color from hand
    /// (and paying life in the case of Force of Will), bypassing the normal mana cost.
    fn generate_alt_cost_actions(
        &self,
        player_id: PlayerId,
        db: &[CardDef],
        actions: &mut Vec<Action>,
        sorcery_speed: bool,
    ) {
        let player = &self.players[player_id as usize];
        let is_opponent_turn = player_id != self.active_player;

        for &card_id in &player.hand {
            let card_name = match self.card_name_for_id(card_id) {
                Some(cn) => cn,
                None => continue,
            };

            match card_name {
                // --- Force of Will: exile blue card + pay 1 life, instant speed, stack must be non-empty ---
                CardName::ForceOfWill => {
                    if self.stack.is_empty() {
                        continue;
                    }
                    if player.life <= 1 {
                        continue;
                    }
                    // Find all blue cards that can be exiled (must be different card from FoW itself)
                    let blue_exile_candidates: Vec<ObjectId> = player.hand.iter()
                        .copied()
                        .filter(|&other_id| {
                            other_id != card_id
                                && self.card_name_for_id(other_id)
                                    .and_then(|cn| find_card(db, cn))
                                    .map(|d| d.color_identity.contains(&Color::Blue))
                                    .unwrap_or(false)
                        })
                        .collect();
                    for exile_id in blue_exile_candidates {
                        let target_sets = self.generate_targets(card_name, player_id, db);
                        if target_sets.is_empty() {
                            // FoW targets spells on stack; generate one action per stack spell
                            for item in self.stack.items() {
                                actions.push(Action::CastSpell {
                                    card_id,
                                    targets: vec![Target::Object(item.id)],
                                    x_value: 0,
                                    from_graveyard: false,
                from_library_top: false,
                                    alt_cost: Some(AltCost::ForceOfWill { exile_id }),
                                modes: vec![],
                                });
                            }
                        } else {
                            for targets in &target_sets {
                                actions.push(Action::CastSpell {
                                    card_id,
                                    targets: targets.clone(),
                                    x_value: 0,
                                    from_graveyard: false,
                from_library_top: false,
                                    alt_cost: Some(AltCost::ForceOfWill { exile_id }),
                                modes: vec![],
                                });
                            }
                        }
                    }
                }

                // --- Force of Negation: exile blue card, opponent's turn only ---
                CardName::ForceOfNegation => {
                    if !is_opponent_turn || self.stack.is_empty() {
                        continue;
                    }
                    let blue_exile_candidates: Vec<ObjectId> = player.hand.iter()
                        .copied()
                        .filter(|&other_id| {
                            other_id != card_id
                                && self.card_name_for_id(other_id)
                                    .and_then(|cn| find_card(db, cn))
                                    .map(|d| d.color_identity.contains(&Color::Blue))
                                    .unwrap_or(false)
                        })
                        .collect();
                    for exile_id in blue_exile_candidates {
                        let target_sets = self.generate_targets(card_name, player_id, db);
                        if target_sets.is_empty() {
                            for item in self.stack.items() {
                                actions.push(Action::CastSpell {
                                    card_id,
                                    targets: vec![Target::Object(item.id)],
                                    x_value: 0,
                                    from_graveyard: false,
                from_library_top: false,
                                    alt_cost: Some(AltCost::ForceOfNegation { exile_id }),
                                modes: vec![],
                                });
                            }
                        } else {
                            for targets in &target_sets {
                                actions.push(Action::CastSpell {
                                    card_id,
                                    targets: targets.clone(),
                                    x_value: 0,
                                    from_graveyard: false,
                from_library_top: false,
                                    alt_cost: Some(AltCost::ForceOfNegation { exile_id }),
                                modes: vec![],
                                });
                            }
                        }
                    }
                }

                // --- Misdirection: exile blue card, instant speed ---
                CardName::Misdirection => {
                    if self.stack.is_empty() {
                        continue;
                    }
                    let blue_exile_candidates: Vec<ObjectId> = player.hand.iter()
                        .copied()
                        .filter(|&other_id| {
                            other_id != card_id
                                && self.card_name_for_id(other_id)
                                    .and_then(|cn| find_card(db, cn))
                                    .map(|d| d.color_identity.contains(&Color::Blue))
                                    .unwrap_or(false)
                        })
                        .collect();
                    for exile_id in blue_exile_candidates {
                        let target_sets = self.generate_targets(card_name, player_id, db);
                        if target_sets.is_empty() {
                            for item in self.stack.items() {
                                // Misdirection targets spells with a single target
                                actions.push(Action::CastSpell {
                                    card_id,
                                    targets: vec![Target::Object(item.id)],
                                    x_value: 0,
                                    from_graveyard: false,
                from_library_top: false,
                                    alt_cost: Some(AltCost::Misdirection { exile_id }),
                                modes: vec![],
                                });
                            }
                        } else {
                            for targets in &target_sets {
                                actions.push(Action::CastSpell {
                                    card_id,
                                    targets: targets.clone(),
                                    x_value: 0,
                                    from_graveyard: false,
                from_library_top: false,
                                    alt_cost: Some(AltCost::Misdirection { exile_id }),
                                modes: vec![],
                                });
                            }
                        }
                    }
                }

                // --- Commandeer: exile two blue cards, instant speed ---
                CardName::Commandeer => {
                    if self.stack.is_empty() {
                        continue;
                    }
                    // Collect all pairs of distinct blue cards in hand
                    let blue_cards: Vec<ObjectId> = player.hand.iter()
                        .copied()
                        .filter(|&other_id| {
                            other_id != card_id
                                && self.card_name_for_id(other_id)
                                    .and_then(|cn| find_card(db, cn))
                                    .map(|d| d.color_identity.contains(&Color::Blue))
                                    .unwrap_or(false)
                        })
                        .collect();
                    if blue_cards.len() < 2 {
                        continue;
                    }
                    // Generate one representative pair (first two blue cards) to avoid
                    // action space explosion. In practice, which pair is chosen rarely matters.
                    let exile_id1 = blue_cards[0];
                    let exile_id2 = blue_cards[1];
                    let target_sets = self.generate_targets(card_name, player_id, db);
                    if target_sets.is_empty() {
                        for item in self.stack.items() {
                            actions.push(Action::CastSpell {
                                card_id,
                                targets: vec![Target::Object(item.id)],
                                x_value: 0,
                                from_graveyard: false,
                from_library_top: false,
                                alt_cost: Some(AltCost::Commandeer { exile_id1, exile_id2 }),
                                modes: vec![],
                            });
                        }
                    } else {
                        for targets in &target_sets {
                            actions.push(Action::CastSpell {
                                card_id,
                                targets: targets.clone(),
                                x_value: 0,
                                from_graveyard: false,
                from_library_top: false,
                                alt_cost: Some(AltCost::Commandeer { exile_id1, exile_id2 }),
                                modes: vec![],
                            });
                        }
                    }
                }

                // --- Evoke creatures: exile a card of matching color from hand ---
                // Solitude: exile a white card
                CardName::Solitude => {
                    self.generate_evoke_actions(card_id, card_name, Color::White, player_id, db, actions, sorcery_speed);
                }
                // Grief: exile a black card
                CardName::Grief => {
                    self.generate_evoke_actions(card_id, card_name, Color::Black, player_id, db, actions, sorcery_speed);
                }
                // Fury: exile a red card
                CardName::Fury => {
                    self.generate_evoke_actions(card_id, card_name, Color::Red, player_id, db, actions, sorcery_speed);
                }
                // Endurance: exile a green card
                CardName::Endurance => {
                    self.generate_evoke_actions(card_id, card_name, Color::Green, player_id, db, actions, sorcery_speed);
                }

                // --- Phyrexian mana cards ---
                // GitaxianProbe {U/P}: can pay 2 life instead of {U}
                CardName::GitaxianProbe => {
                    // Sorcery speed (GitaxianProbe is a sorcery)
                    if !sorcery_speed {
                        continue;
                    }
                    // Life payment option: 2 life instead of {U}
                    if player.life > 2 {
                        let target_sets = self.generate_targets(card_name, player_id, db);
                        let normal_cost = ManaCost::ZERO; // entire mana cost paid with life
                        let alt = AltCost::PhyrexianMana { life_paid: 2, normal_cost };
                        if target_sets.is_empty() {
                            actions.push(Action::CastSpell {
                                card_id,
                                targets: vec![],
                                x_value: 0,
                                from_graveyard: false,
                from_library_top: false,
                                alt_cost: Some(alt),
                                modes: vec![],
                            });
                        } else {
                            for targets in &target_sets {
                                actions.push(Action::CastSpell {
                                    card_id,
                                    targets: targets.clone(),
                                    x_value: 0,
                                    from_graveyard: false,
                from_library_top: false,
                                    alt_cost: Some(alt.clone()),
                                    modes: vec![],
                                });
                            }
                        }
                    }
                }

                // MentalMisstep {U/P}: can pay 2 life instead of {U}, instant speed
                // MentalMisstep requires a spell on the stack as target; skip if stack empty.
                CardName::MentalMisstep => {
                    if player.life > 2 {
                        let target_sets = self.generate_targets(card_name, player_id, db);
                        if target_sets.is_empty() {
                            // No spells on stack to target; cannot cast
                            continue;
                        }
                        let normal_cost = ManaCost::ZERO;
                        let alt = AltCost::PhyrexianMana { life_paid: 2, normal_cost };
                        for targets in &target_sets {
                            actions.push(Action::CastSpell {
                                card_id,
                                targets: targets.clone(),
                                x_value: 0,
                                from_graveyard: false,
                from_library_top: false,
                                alt_cost: Some(alt.clone()),
                                modes: vec![],
                            });
                        }
                    }
                }

                // SurgicalExtraction {B/P}: can pay 2 life instead of {B}, instant speed
                // SurgicalExtraction requires a graveyard target; no target → cannot be cast
                CardName::SurgicalExtraction => {
                    if player.life > 2 {
                        let target_sets = self.generate_targets(card_name, player_id, db);
                        if target_sets.is_empty() {
                            // No graveyard targets available; cannot cast
                            continue;
                        }
                        let normal_cost = ManaCost::ZERO;
                        let alt = AltCost::PhyrexianMana { life_paid: 2, normal_cost };
                        for targets in &target_sets {
                            actions.push(Action::CastSpell {
                                card_id,
                                targets: targets.clone(),
                                x_value: 0,
                                from_graveyard: false,
                from_library_top: false,
                                alt_cost: Some(alt.clone()),
                                modes: vec![],
                            });
                        }
                    }
                }

                // Dismember {1}{B/P}{B/P}: each B/P can be paid with 2 life each
                // Normal cost (stored as all-mana): {1}{B}{B}
                // Phyrexian options:
                //   life_paid=2: pay {1}{B} + 2 life (one pip replaced)
                //   life_paid=4: pay {1} + 4 life (both pips replaced)
                // Dismember requires a creature target; skip if none available.
                CardName::Dismember => {
                    let target_sets = self.generate_targets(card_name, player_id, db);
                    if target_sets.is_empty() {
                        continue;
                    }

                    // Variant 1: pay one B/P with life → {1}{B} + 2 life
                    if player.life > 2 {
                        let normal_cost = ManaCost { generic: 1, black: 1, ..ManaCost::ZERO };
                        if player.mana_pool.can_pay(&normal_cost) {
                            let alt = AltCost::PhyrexianMana { life_paid: 2, normal_cost };
                            for targets in &target_sets {
                                actions.push(Action::CastSpell {
                                    card_id,
                                    targets: targets.clone(),
                                    x_value: 0,
                                    from_graveyard: false,
                from_library_top: false,
                                    alt_cost: Some(alt.clone()),
                                    modes: vec![],
                                });
                            }
                        }
                    }

                    // Variant 2: pay both B/P with life → {1} + 4 life
                    if player.life > 4 {
                        let normal_cost = ManaCost { generic: 1, ..ManaCost::ZERO };
                        if player.mana_pool.can_pay(&normal_cost) {
                            let alt = AltCost::PhyrexianMana { life_paid: 4, normal_cost };
                            for targets in &target_sets {
                                actions.push(Action::CastSpell {
                                    card_id,
                                    targets: targets.clone(),
                                    x_value: 0,
                                    from_graveyard: false,
                from_library_top: false,
                                    alt_cost: Some(alt.clone()),
                                    modes: vec![],
                                });
                            }
                        }
                    }
                }

                _ => {}
            }
        }
    }

    /// Generate evoke CastSpell actions for a creature with evoke (exile matching color card).
    /// Evoke creatures can be cast at their normal timing (respecting instant/sorcery speed).
    fn generate_evoke_actions(
        &self,
        card_id: ObjectId,
        card_name: CardName,
        evoke_color: Color,
        player_id: PlayerId,
        db: &[CardDef],
        actions: &mut Vec<Action>,
        sorcery_speed: bool,
    ) {
        let player = &self.players[player_id as usize];

        // Evoke creatures are cast at sorcery speed (they're creatures) unless they have Flash
        let def = match find_card(db, card_name) {
            Some(d) => d,
            None => return,
        };
        let can_cast_at_instant_speed = def.card_types.contains(&CardType::Instant)
            || def.keywords.has(Keyword::Flash);
        if !can_cast_at_instant_speed && !sorcery_speed {
            return;
        }

        // Find exile candidates of the required color (must be different from the evoke creature itself)
        let exile_candidates: Vec<ObjectId> = player.hand.iter()
            .copied()
            .filter(|&other_id| {
                other_id != card_id
                    && self.card_name_for_id(other_id)
                        .and_then(|cn| find_card(db, cn))
                        .map(|d| d.color_identity.contains(&evoke_color))
                        .unwrap_or(false)
            })
            .collect();

        for exile_id in exile_candidates {
            let target_sets = self.generate_targets(card_name, player_id, db);
            if target_sets.is_empty() {
                actions.push(Action::CastSpell {
                    card_id,
                    targets: vec![],
                    x_value: 0,
                    from_graveyard: false,
                from_library_top: false,
                    alt_cost: Some(AltCost::Evoke { exile_id }),
                    modes: vec![],
                });
            } else {
                for targets in &target_sets {
                    actions.push(Action::CastSpell {
                        card_id,
                        targets: targets.clone(),
                        x_value: 0,
                        from_graveyard: false,
                from_library_top: false,
                        alt_cost: Some(AltCost::Evoke { exile_id }),
                        modes: vec![],
                    });
                }
            }
        }
    }

    /// Generate all valid CastSpell actions for a modal spell, covering every valid
    /// combination of modes and the required targets for each combination.
    fn generate_modal_actions(
        &self,
        card_id: ObjectId,
        card_name: CardName,
        player_id: PlayerId,
        db: &[CardDef],
    ) -> Vec<Action> {
        match card_name {
            CardName::KolaghanCommand => {
                // Choose exactly 2 of 4 modes.
                // Mode targets:
                //   0: graveyard creature card owned by controller
                //   1: any player (discard)
                //   2: any artifact on battlefield
                //   3: any creature or player (deal 2 damage)
                let spell_colors = find_card(db, card_name)
                    .map(|d| d.color_identity.to_vec())
                    .unwrap_or_default();
                let gyd_creatures: Vec<ObjectId> = self.players[player_id as usize]
                    .graveyard
                    .iter()
                    .filter(|&&id| {
                        self.card_name_for_id(id)
                            .and_then(|cn| find_card(db, cn))
                            .map(|def| def.card_types.contains(&CardType::Creature))
                            .unwrap_or(false)
                    })
                    .copied()
                    .collect();
                let players: Vec<PlayerId> = (0..self.num_players).collect();
                let artifacts: Vec<ObjectId> = self.battlefield.iter()
                    .filter(|p| p.is_artifact() && self.can_be_targeted(p, player_id, &spell_colors))
                    .map(|p| p.id)
                    .collect();
                // Damage targets: any player or creature
                let damage_targets: Vec<Target> = {
                    let mut v: Vec<Target> = players.iter().map(|&p| Target::Player(p)).collect();
                    for perm in &self.battlefield {
                        if perm.is_creature() && self.can_be_targeted(perm, player_id, &spell_colors) {
                            v.push(Target::Object(perm.id));
                        }
                    }
                    v
                };

                // Generate targets for each mode
                let mode_targets: [Vec<Target>; 4] = [
                    gyd_creatures.iter().map(|&id| Target::Object(id)).collect(),
                    players.iter().map(|&p| Target::Player(p)).collect(),
                    artifacts.iter().map(|&id| Target::Object(id)).collect(),
                    damage_targets.clone(),
                ];

                let mut result = Vec::new();
                // All C(4,2) = 6 combinations
                for mode_a in 0u8..4 {
                    for mode_b in (mode_a + 1)..4 {
                        let modes = vec![mode_a, mode_b];
                        // Mode 1 (discard) has no explicit target constraint check needed
                        // (targets a player, always valid)
                        // Some modes may have no valid targets
                        let targets_a = &mode_targets[mode_a as usize];
                        let targets_b = &mode_targets[mode_b as usize];
                        // modes 0 and 2 require a valid target; modes 1 and 3 always have valid targets
                        let a_needs_target = mode_a == 0 || mode_a == 2;
                        let b_needs_target = mode_b == 0 || mode_b == 2;
                        if a_needs_target && targets_a.is_empty() {
                            continue;
                        }
                        if b_needs_target && targets_b.is_empty() {
                            continue;
                        }
                        // Generate Cartesian product of valid targets for the two modes
                        let effective_a: Vec<Target> = if targets_a.is_empty() {
                            // Default target when mode doesn't require one (e.g., mode 1 player)
                            vec![Target::Player(self.opponent(player_id))]
                        } else {
                            targets_a.clone()
                        };
                        let effective_b: Vec<Target> = if targets_b.is_empty() {
                            vec![Target::Player(self.opponent(player_id))]
                        } else {
                            targets_b.clone()
                        };
                        for &ta in &effective_a {
                            for &tb in &effective_b {
                                result.push(Action::CastSpell {
                                    card_id,
                                    targets: vec![ta, tb],
                                    x_value: 0,
                                    from_graveyard: false,
                                    from_library_top: false,
                                    alt_cost: None,
                                    modes: modes.clone(),
                                });
                            }
                        }
                    }
                }
                result
            }

            CardName::KozileksCommand => {
                // Choose exactly 2 of 4 modes.
                // Mode targets:
                //   0: any player (draws 2, loses 2 life)
                //   1: no target (creates Eldrazi Spawn token)
                //   2: any artifact or enchantment
                //   3: any creature (-3/-3)
                let spell_colors = find_card(db, card_name)
                    .map(|d| d.color_identity.to_vec())
                    .unwrap_or_default();
                let players: Vec<PlayerId> = (0..self.num_players).collect();
                let art_ench: Vec<ObjectId> = self.battlefield.iter()
                    .filter(|p| (p.is_artifact() || p.is_enchantment()) && self.can_be_targeted(p, player_id, &spell_colors))
                    .map(|p| p.id)
                    .collect();
                let creatures: Vec<ObjectId> = self.battlefield.iter()
                    .filter(|p| p.is_creature() && self.can_be_targeted(p, player_id, &spell_colors))
                    .map(|p| p.id)
                    .collect();

                let mode_targets: [Vec<Target>; 4] = [
                    players.iter().map(|&p| Target::Player(p)).collect(),
                    vec![], // mode 1 has no target
                    art_ench.iter().map(|&id| Target::Object(id)).collect(),
                    creatures.iter().map(|&id| Target::Object(id)).collect(),
                ];
                // modes that need a valid target: 0 (always has players), 2, 3
                let needs_target = [false, false, true, true];

                let mut result = Vec::new();
                for mode_a in 0u8..4 {
                    for mode_b in (mode_a + 1)..4 {
                        let modes = vec![mode_a, mode_b];
                        let targets_a = &mode_targets[mode_a as usize];
                        let targets_b = &mode_targets[mode_b as usize];
                        if needs_target[mode_a as usize] && targets_a.is_empty() {
                            continue;
                        }
                        if needs_target[mode_b as usize] && targets_b.is_empty() {
                            continue;
                        }
                        // Enumerate targets
                        let effective_a: Vec<Option<Target>> = if targets_a.is_empty() {
                            vec![None] // mode 1: no target
                        } else {
                            targets_a.iter().map(|&t| Some(t)).collect()
                        };
                        let effective_b: Vec<Option<Target>> = if targets_b.is_empty() {
                            vec![None]
                        } else {
                            targets_b.iter().map(|&t| Some(t)).collect()
                        };
                        for ta_opt in &effective_a {
                            for tb_opt in &effective_b {
                                let mut targets = Vec::new();
                                if let Some(ta) = ta_opt { targets.push(*ta); }
                                if let Some(tb) = tb_opt { targets.push(*tb); }
                                result.push(Action::CastSpell {
                                    card_id,
                                    targets,
                                    x_value: 0,
                                    from_graveyard: false,
                                    from_library_top: false,
                                    alt_cost: None,
                                    modes: modes.clone(),
                                });
                            }
                        }
                    }
                }
                result
            }

            _ => vec![],
        }
    }
}

impl GameState {
    /// Check if a Cavern of Souls controlled by `player_id` makes a creature spell uncounterable.
    /// Returns true if the player controls a Cavern whose named creature type matches
    /// any creature type of the spell's card definition, or if the card is a changeling.
    /// Check if a Cavern of Souls controlled by `player_id` makes a creature spell uncounterable.
    /// Simplified: if the player controls a Cavern with the matching named type, and the spell
    /// has that creature type (or is a changeling), the spell is uncounterable.
    /// Note: In actual MTG rules, Cavern's colored mana must be spent on the spell; here we
    /// approximate this for the game tree search.
    pub fn cavern_makes_uncounterable(&self, player_id: PlayerId, def: &crate::card::CardDef, _card_name: CardName) -> bool {
        for perm in &self.battlefield {
            if perm.card_name == CardName::CavernOfSouls && perm.controller == player_id {
                if let Some(cavern_type) = perm.cavern_creature_type {
                    if def.is_changeling || def.creature_types.contains(&cavern_type) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

pub fn is_uncounterable(name: CardName) -> bool {
    matches!(
        name,
        CardName::AbruptDecay
    )
}

/// Returns true if a spell requires sacrificing a permanent as an additional cost.
/// Such spells cannot be cast if generate_targets returns no valid targets.
pub fn requires_sacrifice_cost(name: CardName) -> bool {
    matches!(
        name,
        CardName::VillageRites
            | CardName::DeadlyDispute
            | CardName::ShrapnelBlast
            | CardName::NaturalOrder
            | CardName::CropRotation
    )
}

/// Returns true if this card is a modal spell (choose N of M modes).
pub fn is_modal_spell(name: CardName) -> bool {
    matches!(name, CardName::KolaghanCommand | CardName::KozileksCommand)
}
