/// Action application: casting spells, activating abilities, and related logic.

use super::is_uncounterable;
use crate::action::*;
use crate::card::*;
use crate::game::*;
use crate::mana::*;
use crate::permanent::Permanent;
use crate::stack::*;
use crate::types::*;

impl GameState {
    pub fn apply_action(&mut self, action: &Action, db: &[CardDef]) {
        match action {
            Action::PassPriority => {
                self.pass_priority(db);
            }

            Action::Concede => {
                self.players[self.priority_player as usize].has_lost = true;
                self.check_state_based_actions(db);
            }

            Action::PlayLand(card_id) => {
                let player_id = self.priority_player;
                let card_name = self.card_name_for_id(*card_id);
                if self.players[player_id as usize].remove_from_hand(*card_id) {
                    self.players[player_id as usize].land_plays_remaining -= 1;
                    if let Some(cn) = card_name {
                        if let Some(def) = find_card(db, cn) {
                            let mut perm = Permanent::new(
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
                            perm.colors = def.color_identity.to_vec();
                            self.battlefield.push(perm);
                            self.handle_etb(cn, *card_id, player_id);
                        }
                    }
                }
            }

            Action::PlayLandFromTop(card_id) => {
                let player_id = self.priority_player;
                let card_name = self.card_name_for_id(*card_id);
                // The top of the library must be this card
                let is_top = self.players[player_id as usize].library.last() == Some(card_id);
                if !is_top {
                    return;
                }
                if let Some(cn) = card_name {
                    if let Some(def) = find_card(db, cn) {
                        if def.card_types.contains(&CardType::Land) {
                            // Remove from top of library
                            self.players[player_id as usize].library.pop();
                            self.players[player_id as usize].land_plays_remaining -= 1;
                            let mut perm = Permanent::new(
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
                            perm.colors = def.color_identity.to_vec();
                            self.battlefield.push(perm);
                            self.handle_etb(cn, *card_id, player_id);
                        }
                    }
                }
            }

            Action::CastSpell { card_id, targets, x_value, from_graveyard, from_library_top, alt_cost, modes } => {
                let player_id = self.priority_player;
                let card_name = self.card_name_for_id(*card_id);
                if let Some(cn) = card_name {
                    if let Some(def) = find_card(db, cn) {
                        // Determine whether we're paying an alternate cost or normal mana cost.
                        let is_artifact = def.card_types.contains(&CardType::Artifact);
                        let paid = if cn == CardName::HogaakArisenNecropolis {
                            // Hogaak can't spend mana to cast. Pay with convoke + delve.
                            // Simplified: tap up to 7 untapped creatures, exile rest from graveyard.
                            let mut remaining = 7u32;
                            // Convoke: tap untapped creatures
                            let creature_ids: Vec<ObjectId> = self.battlefield.iter()
                                .filter(|p| p.controller == player_id && p.is_creature() && !p.tapped)
                                .map(|p| p.id)
                                .collect();
                            for cid in creature_ids {
                                if remaining == 0 { break; }
                                if let Some(perm) = self.find_permanent_mut(cid) {
                                    perm.tapped = true;
                                    remaining -= 1;
                                }
                            }
                            // Delve: exile from graveyard for the rest
                            let gy = &mut self.players[player_id as usize].graveyard;
                            let to_exile = (remaining as usize).min(gy.len());
                            let mut exiled_cards = Vec::new();
                            for _ in 0..to_exile {
                                if let Some(card) = gy.pop() {
                                    exiled_cards.push(card);
                                }
                            }
                            remaining -= to_exile as u32;
                            for card in exiled_cards {
                                self.exile.push((card, self.card_name_for_id(card).unwrap_or(CardName::Plains), player_id));
                            }
                            remaining == 0
                        } else if let Some(alt) = alt_cost {
                            self.pay_alt_cost(player_id, *card_id, alt)
                        } else if *from_graveyard {
                            // Flashback / Yawgmoth's Will: pay the flashback cost.
                            let base_cost = def.flashback_cost.unwrap_or(def.mana_cost);
                            let taxed = self.effective_cost_with_base(def, player_id, base_cost);
                            if is_artifact {
                                self.players[player_id as usize].mana_pool.pay_for_artifact(&taxed)
                            } else {
                                self.players[player_id as usize].mana_pool.pay(&taxed)
                            }
                        } else if *from_library_top {
                            // Casting from top of library.
                            // Check if the card is actually on top of the library.
                            let is_top = self.players[player_id as usize].library.last() == Some(card_id);
                            if !is_top {
                                false
                            } else {
                                // Check if Bolas's Citadel is on the battlefield (controlled by this player).
                                let citadel_active = self.battlefield.iter().any(|p| {
                                    p.card_name == CardName::BolassCitadel && p.controller == player_id
                                });
                                if citadel_active {
                                    // Pay life equal to the card's mana value instead of mana.
                                    let life_cost = def.mana_cost.cmc() as i32;
                                    if self.players[player_id as usize].life > life_cost {
                                        self.players[player_id as usize].life -= life_cost;
                                        true
                                    } else {
                                        false
                                    }
                                } else {
                                    // Future Sight / Mystic Forge / Experimental Frenzy: pay normal mana cost.
                                    let mut cost = self.effective_cost(def, player_id);
                                    if def.has_x_cost {
                                        let x_cost = (*x_value as u16) * (def.x_multiplier as u16);
                                        cost.generic = cost.generic.saturating_add(x_cost as u8);
                                    }
                                    if is_artifact {
                                        self.players[player_id as usize].mana_pool.pay_for_artifact(&cost)
                                    } else {
                                        self.players[player_id as usize].mana_pool.pay(&cost)
                                    }
                                }
                            }
                        } else {
                            // Normal mana cost (with tax effects applied).
                            let mut cost = self.effective_cost(def, player_id);
                            // For X spells, add X * x_multiplier to the generic cost.
                            if def.has_x_cost {
                                let x_cost = (*x_value as u16) * (def.x_multiplier as u16);
                                cost.generic = cost.generic.saturating_add(x_cost as u8);
                            }
                            if is_artifact {
                                self.players[player_id as usize].mana_pool.pay_for_artifact(&cost)
                            } else {
                                self.players[player_id as usize].mana_pool.pay(&cost)
                            }
                        };

                        if paid {
                            // Remove the card from the appropriate zone.
                            if *from_graveyard {
                                // Remove from graveyard
                                let gyd = &mut self.players[player_id as usize].graveyard;
                                if let Some(pos) = gyd.iter().position(|&id| id == *card_id) {
                                    gyd.swap_remove(pos);
                                } else {
                                    // Card not found in graveyard — bail out
                                    return;
                                }
                            } else if *from_library_top {
                                // Remove from top of library
                                let lib = &mut self.players[player_id as usize].library;
                                if lib.last() == Some(card_id) {
                                    lib.pop();
                                } else {
                                    return;
                                }
                            } else {
                                self.players[player_id as usize].remove_from_hand(*card_id);
                            }
                            // Delve: exile cards from graveyard as part of the cost.
                            // The effective_cost already reduced the generic cost, so we exile
                            // the same number of cards that offset the generic mana.
                            let has_delve = matches!(cn,
                                CardName::DigThroughTime | CardName::TreasureCruise | CardName::HogaakArisenNecropolis
                            );
                            if has_delve {
                                // How many cards were delved? = original generic cost - effective generic cost
                                let original_generic = def.mana_cost.generic as u32;
                                let effective_generic = self.effective_cost(def, player_id).generic as u32;
                                let delved = original_generic.saturating_sub(effective_generic);
                                let gy = &mut self.players[player_id as usize].graveyard;
                                let to_exile = (delved as usize).min(gy.len());
                                let mut exiled_cards = Vec::new();
                                for _ in 0..to_exile {
                                    if let Some(card) = gy.pop() {
                                        exiled_cards.push(card);
                                    }
                                }
                                for card in exiled_cards {
                                    let cn_for_exile = self.card_name_for_id(card).unwrap_or(CardName::Plains);
                                    self.exile.push((card, cn_for_exile, player_id));
                                }
                            }
                            // Check static can't-be-countered (e.g., Abrupt Decay)
                            let mut uncounterable = is_uncounterable(cn);
                            // Cavern of Souls: creature spells of the named type can't be countered.
                            if !uncounterable && def.card_types.contains(&CardType::Creature) {
                                uncounterable = self.cavern_makes_uncounterable(player_id, def, cn);
                            }
                            // Mark evoke-cast spells so resolution can apply the sacrifice trigger.
                            let is_evoke = matches!(alt_cost, Some(AltCost::Evoke { .. }));
                            let spell_id = self.stack.push_with_flags(
                                StackItemKind::Spell {
                                    card_name: cn,
                                    card_id: *card_id,
                                    cast_via_evoke: is_evoke,
                                },
                                player_id,
                                targets.clone(),
                                uncounterable,
                                *x_value,
                                *from_graveyard,
                                modes.clone(),
                            );
                            // Check Lavinia / Boromir: counter free spells cast by opponents.
                            let mana_was_spent = alt_cost.is_none() && !(*from_library_top && self.battlefield.iter().any(|p| p.card_name == CardName::BolassCitadel && p.controller == player_id));
                            self.check_lavinia_trigger(player_id, spell_id, mana_was_spent);
                            // Chalice of the Void: counter spells with MV equal to charge counters.
                            self.check_chalice_trigger(spell_id, def.mana_cost.cmc());
                            // Eidolon of the Great Revel: deal 2 to caster if MV <= 3.
                            self.check_eidolon_trigger(player_id, def.mana_cost.cmc());
                            self.players[player_id as usize].spells_cast_this_turn += 1;
                            if !def.card_types.contains(&CardType::Artifact) {
                                self.players[player_id as usize].nonartifact_spells_cast_this_turn += 1;
                            }
                            let is_noncreature = !def.card_types.contains(&CardType::Creature);
                            if is_noncreature {
                                self.players[player_id as usize].noncreature_spells_cast_this_turn += 1;
                            }
                            self.storm_count += 1;
                            if is_noncreature {
                                self.check_noncreature_cast_triggers(player_id);
                            }
                            // Lurrus of the Dream-Den: track once-per-turn graveyard cast
                            if *from_graveyard
                                && def.mana_cost.cmc() <= 2
                                && (def.card_types.contains(&CardType::Creature)
                                    || def.card_types.contains(&CardType::Artifact)
                                    || def.card_types.contains(&CardType::Enchantment)
                                    || def.card_types.contains(&CardType::Planeswalker))
                                && self.battlefield.iter().any(|p| p.card_name == CardName::LurrusOfTheDreamDen && p.controller == player_id)
                            {
                                self.lurrus_cast_used[player_id as usize] = true;
                            }
                            // Emrakul, the Aeons Torn: when cast, take an extra turn after this one.
                            if cn == CardName::EmrakulTheAeonsTorn {
                                self.stack.push(
                                    crate::stack::StackItemKind::TriggeredAbility {
                                        source_id: *card_id,
                                        source_name: cn,
                                        effect: crate::stack::TriggeredEffect::EmrakulCast,
                                    },
                                    player_id,
                                    vec![],
                                );
                            }
                            // Dack Fayden emblem: "Whenever you cast a spell that targets one or
                            // more permanents, gain control of those permanents."
                            let permanent_targets: Vec<Target> = targets.iter()
                                .filter(|t| matches!(t, Target::Object(_)))
                                .copied()
                                .collect();
                            if !permanent_targets.is_empty()
                                && self.has_emblem(player_id, crate::game::Emblem::DackFayden)
                            {
                                for &tgt in &permanent_targets {
                                    self.stack.push(
                                        crate::stack::StackItemKind::TriggeredAbility {
                                            source_id: 0,
                                            source_name: crate::card::CardName::Plains,
                                            effect: crate::stack::TriggeredEffect::DackEmblemControl,
                                        },
                                        player_id,
                                        vec![tgt],
                                    );
                                }
                            }
                            // Nadu, Winged Wisdom: check targeting triggers for creature targets
                            if !permanent_targets.is_empty() {
                                let creature_target_ids: Vec<ObjectId> = permanent_targets.iter()
                                    .filter_map(|t| if let Target::Object(id) = t {
                                        if self.find_permanent(*id).map(|p| p.is_creature()).unwrap_or(false) {
                                            Some(*id)
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    })
                                    .collect();
                                if !creature_target_ids.is_empty() {
                                    self.check_nadu_targeting_triggers(&creature_target_ids);
                                }
                            }
                            // Tezzeret, Cruel Captain emblem: "Whenever you cast an artifact spell,
                            // search your library for an artifact card, put it onto the battlefield."
                            if def.card_types.contains(&CardType::Artifact)
                                && self.has_emblem(player_id, crate::game::Emblem::TezzeretCruelCaptain)
                            {
                                self.stack.push(
                                    crate::stack::StackItemKind::TriggeredAbility {
                                        source_id: 0,
                                        source_name: crate::card::CardName::Plains,
                                        effect: crate::stack::TriggeredEffect::TezzeretEmblemArtifact,
                                    },
                                    player_id,
                                    vec![],
                                );
                            }
                            // Patchwork Automaton: "Whenever you cast an artifact spell,
                            // put a +1/+1 counter on Patchwork Automaton."
                            if def.card_types.contains(&CardType::Artifact) {
                                let automaton_triggers: Vec<(ObjectId, PlayerId)> = self
                                    .battlefield
                                    .iter()
                                    .filter(|p| p.card_name == crate::card::CardName::PatchworkAutomaton && p.controller == player_id)
                                    .map(|p| (p.id, p.controller))
                                    .collect();
                                for (automaton_id, controller) in automaton_triggers {
                                    self.stack.push(
                                        crate::stack::StackItemKind::TriggeredAbility {
                                            source_id: automaton_id,
                                            source_name: crate::card::CardName::PatchworkAutomaton,
                                            effect: crate::stack::TriggeredEffect::PatchworkAutomatonCast { automaton_id },
                                        },
                                        controller,
                                        vec![],
                                    );
                                }
                                // Kappa Cannoneer: "Whenever you cast an artifact spell,
                                // put a +1/+1 counter on Kappa Cannoneer."
                                let cannoneer_triggers: Vec<(ObjectId, PlayerId)> = self
                                    .battlefield
                                    .iter()
                                    .filter(|p| p.card_name == crate::card::CardName::KappaCannoneer && p.controller == player_id)
                                    .map(|p| (p.id, p.controller))
                                    .collect();
                                for (cannoneer_id, controller) in cannoneer_triggers {
                                    self.stack.push(
                                        crate::stack::StackItemKind::TriggeredAbility {
                                            source_id: cannoneer_id,
                                            source_name: crate::card::CardName::KappaCannoneer,
                                            effect: crate::stack::TriggeredEffect::KappaCannoneerTrigger { cannoneer_id },
                                        },
                                        controller,
                                        vec![],
                                    );
                                }
                            }
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
                let card_name = self.find_permanent(*creature_id).map(|p| p.card_name);
                if let Some(perm) = self.find_permanent_mut(*creature_id) {
                    if !perm.keywords.has(Keyword::Vigilance) {
                        perm.tapped = true;
                    }
                    perm.attacked_this_turn = true;
                }
                // Annihilator N: when this creature attacks, the defending player sacrifices N permanents.
                if let Some(cn) = card_name {
                    let n = crate::card::annihilator_value(cn);
                    if n > 0 {
                        self.trigger_annihilator(defending_player, n);
                    }
                    // Archon of Cruelty: when it attacks, trigger the same effect as its ETB.
                    if cn == CardName::ArchonOfCruelty {
                        self.stack.push(
                            crate::stack::StackItemKind::TriggeredAbility {
                                source_id: *creature_id,
                                source_name: CardName::ArchonOfCruelty,
                                effect: crate::stack::TriggeredEffect::ArchonOfCrueltyTrigger,
                            },
                            self.active_player,
                            vec![crate::types::Target::Player(defending_player)],
                        );
                    }
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
                if let Some(choice) = self.pending_choice.take() {
                    self.resolve_number_choice(choice, *n, db);
                }
            }

            Action::ActivateFromHand { card_id, ability_index, targets, x_value } => {
                self.apply_activate_from_hand(*card_id, *ability_index, targets, *x_value, db);
            }

            Action::CompanionFromSideboard => {
                let player_id = self.priority_player;
                let companion_cost = ManaCost { generic: 3, ..ManaCost::ZERO };
                let player = &self.players[player_id as usize];
                if let Some(companion_id) = player.companion {
                    if player.mana_pool.can_pay(&companion_cost) {
                        // Pay the {3} cost
                        self.players[player_id as usize].mana_pool.pay(&companion_cost);
                        // Move companion from "outside the game" into hand
                        self.players[player_id as usize].companion = None;
                        self.players[player_id as usize].hand.push(companion_id);
                        self.reset_priority_passes();
                    }
                }
            }

            Action::CastAdventure { card_id, targets } => {
                let player_id = self.priority_player;
                let card_name = self.card_name_for_id(*card_id);
                if let Some(cn) = card_name {
                    if let Some(def) = find_card(db, cn) {
                        if let Some(adv) = def.adventure {
                            // Pay the adventure cost
                            let adv_cost = self.effective_cost_with_base(def, player_id, adv.cost);
                            if !self.players[player_id as usize].mana_pool.pay(&adv_cost) {
                                return;
                            }
                            // Remove the card from hand
                            if !self.players[player_id as usize].remove_from_hand(*card_id) {
                                return;
                            }
                            // Push the adventure spell onto the stack, marked as cast_as_adventure
                            self.stack.push_with_all_flags(
                                StackItemKind::Spell {
                                    card_name: cn,
                                    card_id: *card_id,
                                    cast_via_evoke: false,
                                },
                                player_id,
                                targets.clone(),
                                false,  // cant_be_countered
                                0,      // x_value
                                false,  // cast_from_graveyard
                                true,   // cast_as_adventure
                                vec![], // modes
                            );
                            // Chalice of the Void: check adventure's mana cost.
                            // The mana value of an adventure spell on the stack is the adventure's cost.
                            // However, the spell_id wasn't captured from push_with_all_flags above.
                            // We need to get it. Let's find the top of the stack.
                            if let Some(top_item) = self.stack.top() {
                                let adventure_spell_id = top_item.id;
                                self.check_chalice_trigger(adventure_spell_id, adv.cost.cmc());
                            }
                            self.check_eidolon_trigger(player_id, adv.cost.cmc());
                            self.players[player_id as usize].spells_cast_this_turn += 1;
                            self.players[player_id as usize].nonartifact_spells_cast_this_turn += 1;
                            self.players[player_id as usize].noncreature_spells_cast_this_turn += 1;
                            self.storm_count += 1;
                            self.check_noncreature_cast_triggers(player_id);
                            self.reset_priority_passes();
                        }
                    }
                }
            }

            Action::CastCreatureFromAdventureExile { card_id } => {
                let player_id = self.priority_player;
                let card_name = self.card_name_for_id(*card_id);
                if let Some(cn) = card_name {
                    if let Some(def) = find_card(db, cn) {
                        // Pay the creature's normal mana cost
                        let cost = self.effective_cost(def, player_id);
                        if !self.players[player_id as usize].mana_pool.pay(&cost) {
                            return;
                        }
                        // Remove from exile
                        let pos = self.exile.iter().position(|(id, _, _)| *id == *card_id);
                        if let Some(p) = pos {
                            self.exile.swap_remove(p);
                        } else {
                            return; // Not in exile
                        }
                        // Remove from adventure_exiled tracker
                        if let Some(p) = self.adventure_exiled.iter().position(|(id, _)| *id == *card_id) {
                            self.adventure_exiled.swap_remove(p);
                        }
                        // Push the creature spell onto the stack normally
                        let spell_id = self.stack.push(
                            StackItemKind::Spell {
                                card_name: cn,
                                card_id: *card_id,
                                cast_via_evoke: false,
                            },
                            player_id,
                            vec![],
                        );
                        // Chalice of the Void: counter spells with MV equal to charge counters.
                        self.check_chalice_trigger(spell_id, def.mana_cost.cmc());
                        self.check_eidolon_trigger(player_id, def.mana_cost.cmc());
                        self.players[player_id as usize].spells_cast_this_turn += 1;
                        if !def.card_types.contains(&CardType::Artifact) {
                            self.players[player_id as usize].nonartifact_spells_cast_this_turn += 1;
                        }
                        self.storm_count += 1;
                        self.reset_priority_passes();
                    }
                }
            }
        }
    }

    fn apply_activate_from_hand(
        &mut self,
        card_id: ObjectId,
        ability_index: u8,
        targets: &[Target],
        x_value: u8,
        _db: &[CardDef],
    ) {
        let player_id = self.priority_player;
        let card_name = match self.card_name_for_id(card_id) {
            Some(cn) => cn,
            None => return,
        };

        match ability_index {
            // 0 = Cycling
            0 => {
                if let Some((cycling_cost, cycling_kind)) = crate::card::cycling_ability(card_name) {
                    // Street Wraith: pay 2 life instead of mana
                    if card_name == CardName::StreetWraith {
                        self.players[player_id as usize].life -= 2;
                    } else {
                        // Pay X mana for Shark Typhoon, or flat mana for normal cycling
                        let mut cost = cycling_cost;
                        if matches!(cycling_kind, CyclingKind::SharkTyphoon) {
                            cost.generic = cost.generic.saturating_add(x_value);
                        }
                        if !self.players[player_id as usize].mana_pool.pay(&cost) {
                            return;
                        }
                    }
                    // Discard the card (cycling counts as a discard for Hollow One)
                    if !self.players[player_id as usize].remove_from_hand(card_id) {
                        return; // Card not in hand
                    }
                    self.players[player_id as usize].graveyard.push(card_id);
                    self.players[player_id as usize].cards_discarded_this_turn += 1;
                    self.check_emrakul_graveyard_shuffle(player_id);

                    // Push cycling effect to stack
                    let effect = match cycling_kind {
                        CyclingKind::Basic => ActivatedEffect::CyclingDraw,
                        CyclingKind::SharkTyphoon => ActivatedEffect::SharkTyphoonCycling { x_value },
                    };
                    self.stack.push(
                        StackItemKind::ActivatedAbility {
                            source_id: card_id,
                            source_name: card_name,
                            effect,
                        },
                        player_id,
                        vec![],
                    );
                    self.reset_priority_passes();
                }
            }
            // 1 = Channel
            1 => {
                if let Some((channel_cost, channel_kind)) = crate::card::channel_ability(card_name) {
                    if !self.players[player_id as usize].mana_pool.pay(&channel_cost) {
                        return;
                    }
                    // Discard the card
                    if !self.players[player_id as usize].remove_from_hand(card_id) {
                        return;
                    }
                    self.players[player_id as usize].graveyard.push(card_id);
                    self.check_emrakul_graveyard_shuffle(player_id);

                    // Push channel effect to stack
                    let effect = match channel_kind {
                        ChannelKind::Boseiju => ActivatedEffect::BoseijuChannel,
                        ChannelKind::Otawara => ActivatedEffect::OtawaraChannel,
                    };
                    self.stack.push(
                        StackItemKind::ActivatedAbility {
                            source_id: card_id,
                            source_name: card_name,
                            effect,
                        },
                        player_id,
                        targets.to_vec(),
                    );
                    self.reset_priority_passes();
                }
            }
            // 2 = Spirit Guide exile (mana ability, doesn't use stack)
            2 => {
                match card_name {
                    CardName::ElvishSpiritGuide => {
                        // Exile from hand, add {G}
                        if !self.players[player_id as usize].remove_from_hand(card_id) {
                            return;
                        }
                        self.exile.push((card_id, card_name, player_id));
                        self.players[player_id as usize].mana_pool.add(Some(Color::Green), 1);
                        // Mana ability: no stack, no priority reset needed
                    }
                    CardName::SimianSpiritGuide => {
                        // Exile from hand, add {R}
                        if !self.players[player_id as usize].remove_from_hand(card_id) {
                            return;
                        }
                        self.exile.push((card_id, card_name, player_id));
                        self.players[player_id as usize].mana_pool.add(Some(Color::Red), 1);
                        // Mana ability: no stack, no priority reset needed
                    }
                    _ => {}
                }
            }
            _ => {}
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

        // Nadu, Winged Wisdom: check targeting triggers for creature targets of activated abilities.
        {
            let creature_target_ids: Vec<ObjectId> = targets.iter()
                .filter_map(|t| if let Target::Object(id) = t {
                    if self.find_permanent(*id).map(|p| p.is_creature()).unwrap_or(false) {
                        Some(*id)
                    } else {
                        None
                    }
                } else {
                    None
                })
                .collect();
            if !creature_target_ids.is_empty() {
                self.check_nadu_targeting_triggers(&creature_target_ids);
            }
        }

        match card_name {
            CardName::BlackLotus => {
                // Sacrifice + add 3 mana of any color
                // Color is chosen via follow-up ChooseColor action
                if self.destroy_permanent(permanent_id).is_some() {
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseColor {
                            reason: ChoiceReason::BlackLotusColor,
                        },
                    });
                }
            }

            CardName::LotusPetal => {
                if self.destroy_permanent(permanent_id).is_some() {
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
                if self.destroy_permanent(permanent_id).is_some() {
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseColor {
                            reason: ChoiceReason::BlackLotusColor, // Same effect
                        },
                    });
                }
            }

            // Treasure token: Sacrifice to add one mana of any color
            CardName::TreasureToken => {
                if self.remove_permanent(permanent_id).is_some() {
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseColor {
                            reason: ChoiceReason::TreasureSacrificeColor,
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
                self.destroy_permanent(permanent_id);
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
                self.destroy_permanent(permanent_id);
                // Destroy target land
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.destroy_permanent(*target_id);
                }
            }

            // Wasteland: destroy target nonbasic land
            CardName::Wasteland if ability_index == 1 => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.tapped = true;
                }
                self.destroy_permanent(permanent_id);
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.destroy_permanent(*target_id);
                }
            }

            // Isochron Scepter: {2},{T} — copy and cast imprinted instant for free.
            CardName::IsochronScepter if ability_index == 0 => {
                // Pay {2} mana cost
                let cost = crate::mana::ManaCost::generic(2);
                if !self.players[controller as usize].mana_pool.pay(&cost) {
                    return;
                }
                self.stack.push(
                    StackItemKind::ActivatedAbility {
                        source_id: permanent_id,
                        source_name: card_name,
                        effect: ActivatedEffect::IsochronScepterActivated { scepter_id: permanent_id },
                    },
                    controller,
                    vec![],
                );
                self.reset_priority_passes();
            }

            // Shelldock Isle: {T} — cast the hidden card for free (ability_index 1).
            // Condition (library <=20) is already verified in activatable_abilities.
            CardName::ShelldockIsle if ability_index == 1 => {
                if let Some(perm_mut) = self.find_permanent_mut(permanent_id) {
                    perm_mut.tapped = true;
                }
                self.stack.push(
                    StackItemKind::ActivatedAbility {
                        source_id: permanent_id,
                        source_name: card_name,
                        effect: ActivatedEffect::HideawayActivated { land_id: permanent_id },
                    },
                    controller,
                    vec![],
                );
                self.reset_priority_passes();
            }

            // Mosswort Bridge: {T} — cast the hidden card for free (ability_index 1).
            // Condition (control creature with power >= 10) is already verified in activatable_abilities.
            CardName::MosswortBridge if ability_index == 1 => {
                if let Some(perm_mut) = self.find_permanent_mut(permanent_id) {
                    perm_mut.tapped = true;
                }
                self.stack.push(
                    StackItemKind::ActivatedAbility {
                        source_id: permanent_id,
                        source_name: card_name,
                        effect: ActivatedEffect::HideawayActivated { land_id: permanent_id },
                    },
                    controller,
                    vec![],
                );
                self.reset_priority_passes();
            }

            // Griselbrand: pay 7 life, draw 7 cards
            CardName::Griselbrand if ability_index == 0 => {
                self.players[controller as usize].life -= 7;
                self.stack.push(
                    StackItemKind::ActivatedAbility {
                        source_id: permanent_id,
                        source_name: card_name,
                        effect: ActivatedEffect::GriselbrandDraw,
                    },
                    controller,
                    vec![],
                );
                self.reset_priority_passes();
            }

            // Necropotence: pay 1 life, draw a card (simplified)
            CardName::Necropotence if ability_index == 0 => {
                self.players[controller as usize].life -= 1;
                self.stack.push(
                    StackItemKind::ActivatedAbility {
                        source_id: permanent_id,
                        source_name: card_name,
                        effect: ActivatedEffect::NecropotencePayLife,
                    },
                    controller,
                    vec![],
                );
                self.reset_priority_passes();
            }

            // The One Ring: {T}: Put a burden counter, then draw cards equal to burden counters.
            CardName::TheOneRing if ability_index == 0 => {
                self.stack.push(
                    StackItemKind::ActivatedAbility {
                        source_id: permanent_id,
                        source_name: card_name,
                        effect: ActivatedEffect::TheOneRingDraw { ring_id: permanent_id },
                    },
                    controller,
                    vec![],
                );
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

            CardName::DackFayden => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.loyalty_activated_this_turn = true;
                    match ability_index {
                        0 => {
                            perm.loyalty += 1;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::DackDraw,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        1 => {
                            perm.loyalty -= 2;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::DackSteal,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        2 => {
                            perm.loyalty -= 6;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::DackUltimate,
                                },
                                controller,
                                vec![],
                            );
                        }
                        _ => {}
                    }
                }
            }

            CardName::WrennAndSix => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.loyalty_activated_this_turn = true;
                    match ability_index {
                        0 => {
                            perm.loyalty += 1;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::WrennReturn,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        1 => {
                            perm.loyalty -= 1;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::WrennPing,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        2 => {
                            perm.loyalty -= 7;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::WrennUltimate,
                                },
                                controller,
                                vec![],
                            );
                        }
                        _ => {}
                    }
                }
            }

            CardName::TezzeretCruelCaptain => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.loyalty_activated_this_turn = true;
                    match ability_index {
                        0 => {
                            perm.loyalty += 1;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::TezzeretDraw,
                                },
                                controller,
                                vec![],
                            );
                        }
                        1 => {
                            perm.loyalty -= 2;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::TezzeretThopter,
                                },
                                controller,
                                vec![],
                            );
                        }
                        2 => {
                            perm.loyalty -= 7;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::TezzeretUltimate,
                                },
                                controller,
                                vec![],
                            );
                        }
                        _ => {}
                    }
                }
            }

            CardName::GideonOfTheTrials => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.loyalty_activated_this_turn = true;
                    match ability_index {
                        0 => {
                            perm.loyalty += 1;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::GideonPrevent,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        1 => {
                            // 0: Become 4/4 creature (no loyalty change)
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::GideonCreature,
                                },
                                controller,
                                vec![],
                            );
                        }
                        2 => {
                            // +0: Create emblem (no loyalty change)
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::GideonEmblem,
                                },
                                controller,
                                vec![],
                            );
                        }
                        _ => {}
                    }
                }
            }

            CardName::NarsetParterOfVeils => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.loyalty_activated_this_turn = true;
                    match ability_index {
                        0 => {
                            perm.loyalty -= 2;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::NarsetMinus,
                                },
                                controller,
                                vec![],
                            );
                        }
                        _ => {}
                    }
                }
            }

            CardName::OkoThiefOfCrowns => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.loyalty_activated_this_turn = true;
                    match ability_index {
                        0 => {
                            perm.loyalty += 2;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::OkoFood,
                                },
                                controller,
                                vec![],
                            );
                        }
                        1 => {
                            perm.loyalty += 1;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::OkoElkify,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        2 => {
                            // -5: Exchange control
                            perm.loyalty -= 5;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::OkoExchange,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        _ => {}
                    }
                }
            }

            CardName::KarnTheGreatCreator => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.loyalty_activated_this_turn = true;
                    match ability_index {
                        0 => {
                            perm.loyalty += 1;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::KarnAnimate,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        1 => {
                            perm.loyalty -= 2;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::KarnWish,
                                },
                                controller,
                                vec![],
                            );
                        }
                        _ => {}
                    }
                }
            }

            CardName::KayaOrzhovUsurper => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.loyalty_activated_this_turn = true;
                    match ability_index {
                        0 => {
                            perm.loyalty += 1;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::KayaExile,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        1 => {
                            perm.loyalty -= 1;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::KayaMinus,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        2 => {
                            perm.loyalty -= 5;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::KayaUltimate,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        _ => {}
                    }
                }
            }

            CardName::MinscAndBooTimelessHeroes => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.loyalty_activated_this_turn = true;
                    match ability_index {
                        0 => {
                            perm.loyalty += 1;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::MinscCreateBoo,
                                },
                                controller,
                                vec![],
                            );
                        }
                        1 => {
                            perm.loyalty -= 2;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::MinscPump,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        2 => {
                            perm.loyalty -= 6;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::MinscUltimate,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        _ => {}
                    }
                }
            }

            CardName::CometStellarPup => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.loyalty_activated_this_turn = true;
                    // 0 ability: no loyalty change
                    self.stack.push(
                        StackItemKind::ActivatedAbility {
                            source_id: permanent_id,
                            source_name: card_name,
                            effect: ActivatedEffect::CometCreateTokens,
                        },
                        controller,
                        vec![],
                    );
                }
            }

            CardName::DovinHandOfControl => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.loyalty_activated_this_turn = true;
                    match ability_index {
                        0 => {
                            perm.loyalty -= 1;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::DovinPrevent,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        _ => {}
                    }
                }
            }

            // Aphetto Alchemist: {T}: Untap target artifact or creature (ability_index 0)
            CardName::AphettoAlchemist if ability_index == 0 => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.tapped = true;
                }
                self.stack.push(
                    StackItemKind::ActivatedAbility {
                        source_id: permanent_id,
                        source_name: card_name,
                        effect: ActivatedEffect::UntapArtifactOrCreature,
                    },
                    controller,
                    targets.to_vec(),
                );
                self.reset_priority_passes();
            }

            // Emry, Lurker of the Loch: {T}: Choose target artifact in your graveyard.
            // You may cast that card this turn. (ability_index 0)
            CardName::EmryLurkerOfTheLoch if ability_index == 0 => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.tapped = true;
                }
                // Grant the targeted artifact card permission to be cast from graveyard this turn
                if let Some(Target::Object(artifact_id)) = targets.first() {
                    self.emry_castable_artifacts.push(*artifact_id);
                }
                self.reset_priority_passes();
            }

            // Equipment: equip ability (ability_index == 20) — attach to target creature
            _ if ability_index == 20 && crate::card::equip_cost(card_name).is_some() => {
                // Pay equip cost (generic mana)
                let equip_generic = crate::card::equip_cost(card_name).unwrap();
                let paid = self.players[controller as usize].mana_pool.pay_generic(equip_generic as u32);
                if !paid {
                    return; // Can't afford equip
                }
                // Attach to the target creature
                if let Some(Target::Object(creature_id)) = targets.first() {
                    let creature_id = *creature_id;
                    // Push the equip as a triggered-like activated ability to the stack
                    self.stack.push(
                        StackItemKind::ActivatedAbility {
                            source_id: permanent_id,
                            source_name: card_name,
                            effect: ActivatedEffect::EquipCreature { equipment_id: permanent_id },
                        },
                        controller,
                        vec![Target::Object(creature_id)],
                    );
                }
            }

            // Batterskull bounce ability (ability_index == 21): {3}: Return to hand
            CardName::Batterskull if ability_index == 21 => {
                let paid = self.players[controller as usize].mana_pool.pay_generic(3);
                if !paid {
                    return;
                }
                self.stack.push(
                    StackItemKind::ActivatedAbility {
                        source_id: permanent_id,
                        source_name: card_name,
                        effect: ActivatedEffect::BatterskullBounce,
                    },
                    controller,
                    vec![],
                );
            }

            // Walking Ballista: {4}: Put a +1/+1 counter on Walking Ballista (ability_index 0)
            CardName::WalkingBallista if ability_index == 0 => {
                let cost = crate::mana::ManaCost::generic(4);
                if !self.players[controller as usize].mana_pool.pay(&cost) {
                    return;
                }
                self.stack.push(
                    StackItemKind::ActivatedAbility {
                        source_id: permanent_id,
                        source_name: card_name,
                        effect: ActivatedEffect::WalkingBallistaAddCounter { ballista_id: permanent_id },
                    },
                    controller,
                    vec![],
                );
                self.reset_priority_passes();
            }
            // Walking Ballista: Remove a +1/+1 counter: deal 1 damage to any target (ability_index 1)
            CardName::WalkingBallista if ability_index == 1 => {
                // Remove a +1/+1 counter as cost
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    let count = perm.counters.get(CounterType::PlusOnePlusOne);
                    if count == 0 {
                        return;
                    }
                    perm.counters.remove(CounterType::PlusOnePlusOne, 1);
                }
                self.stack.push(
                    StackItemKind::ActivatedAbility {
                        source_id: permanent_id,
                        source_name: card_name,
                        effect: ActivatedEffect::WalkingBallistaPing { ballista_id: permanent_id },
                    },
                    controller,
                    targets.to_vec(),
                );
                self.reset_priority_passes();
            }

            // Time Vault: {T}: Take an extra turn after this one (ability_index 0)
            CardName::TimeVault if ability_index == 0 => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.tapped = true;
                }
                self.stack.push(
                    StackItemKind::ActivatedAbility {
                        source_id: permanent_id,
                        source_name: card_name,
                        effect: ActivatedEffect::TimeVaultExtraTurn,
                    },
                    controller,
                    vec![],
                );
                self.reset_priority_passes();
            }
            // Time Vault: Skip your next turn: Untap Time Vault (ability_index 1)
            CardName::TimeVault if ability_index == 1 => {
                // Cost: skip your next turn (modeled as giving the opponent an extra turn)
                let opponent = self.opponent(controller);
                self.players[opponent as usize].extra_turns += 1;
                self.stack.push(
                    StackItemKind::ActivatedAbility {
                        source_id: permanent_id,
                        source_name: card_name,
                        effect: ActivatedEffect::TimeVaultUntap { vault_id: permanent_id },
                    },
                    controller,
                    vec![],
                );
                self.reset_priority_passes();
            }

            // Krark-Clan Ironworks: Sacrifice an artifact: Add {C}{C} (ability_index 0)
            // This is a mana ability, so it resolves immediately (doesn't use the stack).
            CardName::KrarkClanIronworks if ability_index == 0 => {
                if let Some(Target::Object(artifact_id)) = targets.first() {
                    let artifact_id = *artifact_id;
                    // Verify the artifact exists and is controlled by this player
                    let valid = self.find_permanent(artifact_id)
                        .map(|p| p.is_artifact() && p.controller == controller)
                        .unwrap_or(false);
                    if valid {
                        self.destroy_permanent(artifact_id);
                        self.players[controller as usize].mana_pool.colorless += 2;
                    }
                }
                self.reset_priority_passes();
            }

            // Engineered Explosives: {2}, Sacrifice: Destroy each nonland permanent with MV equal to charge counters (ability_index 0)
            CardName::EngineeredExplosives if ability_index == 0 => {
                let cost = crate::mana::ManaCost::generic(2);
                if !self.players[controller as usize].mana_pool.pay(&cost) {
                    return;
                }
                // Read charge counters before sacrificing
                let charge_counters = self.find_permanent(permanent_id)
                    .map(|p| p.counters.get(CounterType::Charge) as u32)
                    .unwrap_or(0);
                // Sacrifice Engineered Explosives
                self.destroy_permanent(permanent_id);
                self.stack.push(
                    StackItemKind::ActivatedAbility {
                        source_id: permanent_id,
                        source_name: card_name,
                        effect: ActivatedEffect::EngineeredExplosivesDestroy { charge_counters },
                    },
                    controller,
                    vec![],
                );
                self.reset_priority_passes();
            }

            _ => {}
        }
    }

    fn is_fetchable(&self, fetch: CardName, target: CardName) -> bool {
        match fetch {
            // FloodedStrand fetches Plains or Island lands
            // FloodedStrand fetches Plains or Island lands
            CardName::FloodedStrand => matches!(target,
                // Basic lands
                CardName::Plains | CardName::Island
                // Dual lands (Alpha/Beta)
                | CardName::Tundra | CardName::Savannah | CardName::Scrubland | CardName::Plateau
                | CardName::UndergroundSea | CardName::VolcanicIsland | CardName::TropicalIsland
                // Shock lands with Plains subtype
                | CardName::HallowedFountain | CardName::TempleGarden | CardName::GodlessShrine | CardName::SacredFoundry
                // Shock lands with Island subtype
                | CardName::WateryGrave | CardName::SteamVents | CardName::BreedingPool
                // Survey lands with Plains subtype
                | CardName::MeticulousArchive
                // Survey lands with Island subtype (ThunderingFalls=Island+Mountain, HedgeMaze=Forest+Island)
                | CardName::UndercitySewers | CardName::ThunderingFalls | CardName::HedgeMaze
            ),
            // PollutedDelta fetches Island or Swamp lands
            CardName::PollutedDelta => matches!(target,
                // Basic lands
                CardName::Island | CardName::Swamp
                // Dual lands
                | CardName::UndergroundSea | CardName::TropicalIsland | CardName::VolcanicIsland
                | CardName::Badlands | CardName::Bayou | CardName::Tundra | CardName::Scrubland
                // Shock lands with Island subtype
                | CardName::WateryGrave | CardName::SteamVents | CardName::BreedingPool
                | CardName::HallowedFountain
                // Shock lands with Swamp subtype
                | CardName::BloodCrypt | CardName::OvergrownTomb | CardName::GodlessShrine
                // Survey lands with Island subtype
                | CardName::UndercitySewers | CardName::MeticulousArchive
                | CardName::ThunderingFalls | CardName::HedgeMaze
            ),
            // BloodstainedMire fetches Swamp or Mountain lands
            CardName::BloodstainedMire => matches!(target,
                // Basic lands
                CardName::Swamp | CardName::Mountain
                // Dual lands
                | CardName::Badlands | CardName::UndergroundSea | CardName::Bayou | CardName::Scrubland
                | CardName::VolcanicIsland | CardName::Plateau | CardName::Taiga
                // Shock lands with Swamp subtype
                | CardName::BloodCrypt | CardName::OvergrownTomb | CardName::WateryGrave | CardName::GodlessShrine
                // Shock lands with Mountain subtype
                | CardName::StompingGround | CardName::SteamVents | CardName::SacredFoundry
                // Survey lands with Mountain subtype (ThunderingFalls=Island+Mountain)
                | CardName::ThunderingFalls
                // Survey lands with Swamp subtype (UndercitySewers = Island+Swamp)
                | CardName::UndercitySewers
            ),
            // WoodedFoothills fetches Mountain or Forest lands
            CardName::WoodedFoothills => matches!(target,
                // Basic lands
                CardName::Mountain | CardName::Forest
                // Dual lands
                | CardName::Taiga | CardName::VolcanicIsland | CardName::Plateau
                | CardName::Badlands | CardName::Bayou | CardName::Savannah | CardName::TropicalIsland
                // Shock lands with Mountain subtype
                | CardName::StompingGround | CardName::SteamVents | CardName::SacredFoundry | CardName::BloodCrypt
                // Shock lands with Forest subtype
                | CardName::TempleGarden | CardName::OvergrownTomb | CardName::BreedingPool
                // Survey lands with Mountain subtype (ThunderingFalls=Island+Mountain)
                | CardName::ThunderingFalls
                // Survey lands with Forest subtype (HedgeMaze=Forest+Island)
                | CardName::HedgeMaze
            ),
            // WindsweptHeath fetches Forest or Plains lands
            CardName::WindsweptHeath => matches!(target,
                // Basic lands
                CardName::Forest | CardName::Plains
                // Dual lands
                | CardName::Savannah | CardName::TropicalIsland | CardName::Bayou
                | CardName::Taiga | CardName::Tundra | CardName::Plateau | CardName::Scrubland
                // Shock lands with Forest subtype
                | CardName::TempleGarden | CardName::OvergrownTomb | CardName::BreedingPool | CardName::StompingGround
                // Shock lands with Plains subtype
                | CardName::HallowedFountain | CardName::GodlessShrine | CardName::SacredFoundry
                // Survey lands with Forest subtype (HedgeMaze=Forest+Island)
                | CardName::HedgeMaze
                // Survey lands with Plains subtype
                | CardName::MeticulousArchive
            ),
            // MistyRainforest fetches Forest or Island lands
            CardName::MistyRainforest => matches!(target,
                // Basic lands
                CardName::Forest | CardName::Island
                // Dual lands
                | CardName::TropicalIsland | CardName::Bayou | CardName::Savannah
                | CardName::Taiga | CardName::UndergroundSea | CardName::VolcanicIsland | CardName::Tundra
                // Shock lands with Forest subtype
                | CardName::BreedingPool | CardName::OvergrownTomb | CardName::TempleGarden
                | CardName::StompingGround
                // Shock lands with Island subtype
                | CardName::HallowedFountain | CardName::WateryGrave | CardName::SteamVents
                // Survey lands with Forest subtype (HedgeMaze=Forest+Island)
                | CardName::HedgeMaze
                // Survey lands with Island subtype
                | CardName::MeticulousArchive | CardName::UndercitySewers | CardName::ThunderingFalls
            ),
            // ScaldingTarn fetches Island or Mountain lands
            CardName::ScaldingTarn => matches!(target,
                // Basic lands
                CardName::Island | CardName::Mountain
                // Dual lands
                | CardName::VolcanicIsland | CardName::UndergroundSea | CardName::TropicalIsland
                | CardName::Tundra | CardName::Badlands | CardName::Plateau | CardName::Taiga
                // Shock lands with Island subtype
                | CardName::SteamVents | CardName::HallowedFountain | CardName::WateryGrave | CardName::BreedingPool
                // Shock lands with Mountain subtype
                | CardName::StompingGround | CardName::SacredFoundry | CardName::BloodCrypt
                // Survey lands with Island+Mountain subtype (ThunderingFalls=Island+Mountain)
                | CardName::ThunderingFalls
                // Survey lands with Island subtype
                | CardName::MeticulousArchive | CardName::UndercitySewers | CardName::HedgeMaze
            ),
            // VerdantCatacombs fetches Swamp or Forest lands
            CardName::VerdantCatacombs => matches!(target,
                // Basic lands
                CardName::Swamp | CardName::Forest
                // Dual lands
                | CardName::Bayou | CardName::UndergroundSea | CardName::Badlands | CardName::Scrubland
                | CardName::TropicalIsland | CardName::Savannah | CardName::Taiga
                // Shock lands with Swamp subtype
                | CardName::OvergrownTomb | CardName::BloodCrypt | CardName::WateryGrave | CardName::GodlessShrine
                // Shock lands with Forest subtype
                | CardName::TempleGarden | CardName::BreedingPool | CardName::StompingGround
                // Survey lands with Swamp subtype (UndercitySewers = Island+Swamp)
                | CardName::UndercitySewers
                // Survey lands with Forest subtype (HedgeMaze=Forest+Island)
                | CardName::HedgeMaze
            ),
            // AridMesa fetches Mountain or Plains lands
            CardName::AridMesa => matches!(target,
                // Basic lands
                CardName::Mountain | CardName::Plains
                // Dual lands
                | CardName::Plateau | CardName::VolcanicIsland | CardName::Badlands
                | CardName::Taiga | CardName::Tundra | CardName::Savannah | CardName::Scrubland
                // Shock lands with Mountain subtype
                | CardName::SacredFoundry | CardName::StompingGround | CardName::SteamVents | CardName::BloodCrypt
                // Shock lands with Plains subtype
                | CardName::HallowedFountain | CardName::GodlessShrine | CardName::TempleGarden
                // Survey lands with Mountain subtype (ThunderingFalls=Island+Mountain)
                | CardName::ThunderingFalls
                // Survey lands with Plains subtype
                | CardName::MeticulousArchive
            ),
            // MarshFlats fetches Plains or Swamp lands
            CardName::MarshFlats => matches!(target,
                // Basic lands
                CardName::Plains | CardName::Swamp
                // Dual lands
                | CardName::Scrubland | CardName::Tundra | CardName::Savannah
                | CardName::Plateau | CardName::UndergroundSea | CardName::Badlands | CardName::Bayou
                // Shock lands with Plains subtype
                | CardName::GodlessShrine | CardName::HallowedFountain | CardName::TempleGarden | CardName::SacredFoundry
                // Shock lands with Swamp subtype
                | CardName::BloodCrypt | CardName::OvergrownTomb | CardName::WateryGrave
                // Survey lands with Plains subtype
                | CardName::MeticulousArchive
                // Survey lands with Swamp subtype (UndercitySewers = Island+Swamp)
                | CardName::UndercitySewers
            ),
            _ => false,
        }
    }

    /// Pay an alternate cost for a spell being cast.
    /// Returns true if the cost was paid successfully (all required resources were present and consumed).
    /// On success, the exiled card(s) and any life payment are consumed from the player's resources.
    /// On failure, no resources are consumed.
    pub(crate) fn pay_alt_cost(&mut self, player_id: PlayerId, _spell_id: ObjectId, alt: &AltCost) -> bool {
        match alt {
            AltCost::ForceOfWill { exile_id } => {
                // Cost: pay 1 life and exile a blue card from hand.
                let player = &self.players[player_id as usize];
                // Verify the exile card is in hand and is blue.
                let exile_in_hand = player.hand.contains(exile_id);
                if !exile_in_hand || player.life <= 1 {
                    return false;
                }
                // Pay life and exile the card.
                self.players[player_id as usize].life -= 1;
                self.players[player_id as usize].remove_from_hand(*exile_id);
                let exile_name = self.card_name_for_id(*exile_id).unwrap_or(CardName::Plains);
                self.exile.push((*exile_id, exile_name, player_id));
                true
            }
            AltCost::ForceOfNegation { exile_id } => {
                // Cost: exile a blue card from hand (no life payment).
                let player = &self.players[player_id as usize];
                let exile_in_hand = player.hand.contains(exile_id);
                if !exile_in_hand {
                    return false;
                }
                self.players[player_id as usize].remove_from_hand(*exile_id);
                let exile_name = self.card_name_for_id(*exile_id).unwrap_or(CardName::Plains);
                self.exile.push((*exile_id, exile_name, player_id));
                true
            }
            AltCost::Misdirection { exile_id } => {
                // Cost: exile a blue card from hand.
                let player = &self.players[player_id as usize];
                let exile_in_hand = player.hand.contains(exile_id);
                if !exile_in_hand {
                    return false;
                }
                self.players[player_id as usize].remove_from_hand(*exile_id);
                let exile_name = self.card_name_for_id(*exile_id).unwrap_or(CardName::Plains);
                self.exile.push((*exile_id, exile_name, player_id));
                true
            }
            AltCost::Commandeer { exile_id1, exile_id2 } => {
                // Cost: exile two blue cards from hand.
                let player = &self.players[player_id as usize];
                let has_both = player.hand.contains(exile_id1) && player.hand.contains(exile_id2)
                    && exile_id1 != exile_id2;
                if !has_both {
                    return false;
                }
                self.players[player_id as usize].remove_from_hand(*exile_id1);
                self.players[player_id as usize].remove_from_hand(*exile_id2);
                let name1 = self.card_name_for_id(*exile_id1).unwrap_or(CardName::Plains);
                let name2 = self.card_name_for_id(*exile_id2).unwrap_or(CardName::Plains);
                self.exile.push((*exile_id1, name1, player_id));
                self.exile.push((*exile_id2, name2, player_id));
                true
            }
            AltCost::Evoke { exile_id } => {
                // Cost: exile a card of the matching color from hand.
                let player = &self.players[player_id as usize];
                let exile_in_hand = player.hand.contains(exile_id);
                if !exile_in_hand {
                    return false;
                }
                self.players[player_id as usize].remove_from_hand(*exile_id);
                let exile_name = self.card_name_for_id(*exile_id).unwrap_or(CardName::Plains);
                self.exile.push((*exile_id, exile_name, player_id));
                true
            }
            AltCost::PhyrexianMana { life_paid, normal_cost } => {
                // Pay `life_paid` life and `normal_cost` mana from the pool.
                let player = &self.players[player_id as usize];
                // Ensure player has enough life (must survive paying it: life > life_paid)
                if player.life <= *life_paid as i32 {
                    return false;
                }
                // Ensure player can pay the remaining mana cost
                if !player.mana_pool.can_pay(normal_cost) {
                    return false;
                }
                self.players[player_id as usize].life -= *life_paid as i32;
                self.players[player_id as usize].mana_pool.pay(normal_cost);
                true
            }
            AltCost::SnuffOut => {
                // Pay 4 life (must control a Swamp — checked when generating actions).
                let player = &self.players[player_id as usize];
                if player.life <= 4 {
                    return false;
                }
                self.players[player_id as usize].life -= 4;
                true
            }
            AltCost::Daze { island_id } => {
                // Return an Island you control to its owner's hand.
                self.remove_permanent_to_zone(*island_id, DestinationZone::Hand);
                true
            }
            AltCost::Gush { island_id1, island_id2 } => {
                // Return two Islands you control to their owner's hand.
                self.remove_permanent_to_zone(*island_id1, DestinationZone::Hand);
                self.remove_permanent_to_zone(*island_id2, DestinationZone::Hand);
                true
            }
            AltCost::ForceOfVigor { exile_id } => {
                // Exile a green card from hand.
                let player = &self.players[player_id as usize];
                let exile_in_hand = player.hand.contains(exile_id);
                if !exile_in_hand {
                    return false;
                }
                self.players[player_id as usize].remove_from_hand(*exile_id);
                let exile_name = self.card_name_for_id(*exile_id).unwrap_or(CardName::Plains);
                self.exile.push((*exile_id, exile_name, player_id));
                true
            }
        }
    }

}
